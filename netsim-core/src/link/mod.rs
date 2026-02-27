mod id;

use crate::{
    measure::{Bandwidth, CongestionChannel, Latency, PacketLoss},
    network::Round,
};
use rand_core::Rng;
use std::{sync::Arc, time::Duration};

pub use self::id::LinkId;

/// Which direction a packet is travelling across a link.
///
/// `LinkId` is symmetric — `(a, b)` and `(b, a)` produce the same key — so
/// direction must be tracked separately when activating a per-transit
/// [`LinkChannel`]. `Forward` means `smaller_id → larger_id`; `Reverse` is
/// the opposite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkDirection {
    /// Packet is travelling from the node with the smaller [`NodeId`] to the
    /// node with the larger one.
    ///
    /// [`NodeId`]: crate::node::NodeId
    Forward,
    /// Packet is travelling from the node with the larger [`NodeId`] to the
    /// node with the smaller one.
    Reverse,
}

/// Configuration for a connection between two [`Node`]s.
///
/// A `Link` is the canonical record stored in [`Network`]'s link map. It holds
/// the latency, packet-loss policy, and the two independent bandwidth channels
/// (one per direction). It has no per-packet working state — that lives in
/// [`LinkChannel`], which is created via [`Link::channel`] when a packet
/// enters transit.
///
/// [`Node`]: crate::node::Node
/// [`Network`]: crate::network::Network
#[derive(Debug)]
pub struct Link {
    /// Bandwidth channel for the forward direction (smaller_id → larger_id).
    channel_forward: Arc<CongestionChannel>,
    /// Bandwidth channel for the reverse direction (larger_id → smaller_id).
    channel_reverse: Arc<CongestionChannel>,

    latency: Latency,
    packet_loss: PacketLoss,
}

/// Live per-transit state for a single packet travelling across a link.
///
/// Created from a canonical [`Link`] via [`Link::channel`]. Holds one active
/// `Arc<CongestionChannel>` (the direction-specific one), the pending byte
/// count, the latency countdown, and the round guard.
///
/// `Transit` and `Route` own a `LinkChannel`; [`Link`] itself does not.
#[derive(Debug)]
pub(crate) struct LinkChannel {
    /// Bytes waiting to exit the latency delay.
    pending: u64,
    /// Remaining latency before bytes start flowing.
    rem_latency: Duration,
    /// The active directional bandwidth channel.
    channel: Arc<CongestionChannel>,
    round: Round,
}

impl Default for Link {
    fn default() -> Self {
        Self::new(
            Latency::default(),
            Bandwidth::default(),
            PacketLoss::default(),
        )
    }
}

impl Link {
    /// Create a full-duplex link with independent bandwidth channels per direction.
    ///
    /// Both directions are initialised with the same `bandwidth`, but each has
    /// its own [`CongestionChannel`] so traffic in one direction does not
    /// consume capacity in the other.
    pub fn new(latency: Latency, bandwidth: Bandwidth, packet_loss: PacketLoss) -> Self {
        Self::new_with_channels(
            latency,
            Arc::new(CongestionChannel::new(bandwidth.clone())),
            Arc::new(CongestionChannel::new(bandwidth)),
            packet_loss,
        )
    }

    /// Low-level constructor — provide pre-built channels directly.
    ///
    /// Used in tests that need to share or inspect a specific
    /// [`CongestionChannel`] (e.g. to verify half-duplex vs full-duplex behaviour).
    pub(crate) fn new_with_channels(
        latency: Latency,
        channel_forward: Arc<CongestionChannel>,
        channel_reverse: Arc<CongestionChannel>,
        packet_loss: PacketLoss,
    ) -> Self {
        Self {
            channel_forward,
            channel_reverse,
            latency,
            packet_loss,
        }
    }

    /// Returns `true` if this packet should be dropped based on the link's
    /// packet loss model.
    ///
    /// The caller provides `rng` so that all simulation randomness is
    /// controlled from a single, seedable source in [`Network`]. Any type
    /// implementing [`RngCore`] is accepted.
    ///
    /// [`Network`]: crate::network::Network
    pub fn should_drop_packet<R: Rng>(&self, rng: &mut R) -> bool {
        self.packet_loss.should_drop(rng)
    }

    /// Returns the packet loss configuration for this link.
    pub fn packet_loss(&self) -> PacketLoss {
        self.packet_loss
    }

    /// Create a live [`LinkChannel`] for a packet travelling in `direction`.
    ///
    /// The channel shares the same `Arc<CongestionChannel>` as this canonical
    /// link for the requested direction, so bandwidth consumed by the transit
    /// is correctly deducted from the shared pool.
    pub(crate) fn channel(&self, direction: LinkDirection) -> LinkChannel {
        let channel = match direction {
            LinkDirection::Forward => Arc::clone(&self.channel_forward),
            LinkDirection::Reverse => Arc::clone(&self.channel_reverse),
        };
        LinkChannel {
            pending: 0,
            rem_latency: self.latency.into_duration(),
            channel,
            round: Round::ZERO,
        }
    }

    /// Returns the configured latency for this link.
    pub fn latency(&self) -> Latency {
        self.latency
    }

    /// Returns the configured bandwidth for this link.
    pub fn bandwidth(&self) -> &Bandwidth {
        self.channel_forward.bandwidth()
    }
}

impl LinkChannel {
    /// Update the bandwidth capacity of this channel for the current `round`.
    pub fn update_capacity(&mut self, round: Round, duration: Duration) {
        if self.round != round {
            self.round = round;
            let min = std::cmp::min(self.rem_latency, duration);

            self.rem_latency = self.rem_latency.saturating_sub(min);
            let rem = duration.saturating_sub(min);

            self.channel.update_capacity(round, rem);
        }
    }

    /// Attempt to move `inbound` bytes through the channel.
    ///
    /// Bytes are held in `pending` until the latency countdown reaches zero,
    /// at which point as many bytes as the channel allows are consumed and
    /// returned as the number of bytes that transited.
    pub fn process(&mut self, inbound: u64) -> u64 {
        self.pending = self.pending.saturating_add(inbound);

        if self.rem_latency.is_zero() {
            let transited = self.channel.reserve(self.pending);
            self.pending = self.pending.saturating_sub(transited);
            transited
        } else {
            0
        }
    }

    /// Returns `true` when all pending bytes have transited and the latency
    /// countdown has reached zero.
    pub fn completed(&self) -> bool {
        self.pending == 0 && self.rem_latency.is_zero()
    }

    /// Returns the number of bytes currently waiting in this channel's pipeline.
    pub fn bytes_in_transit(&self) -> u64 {
        self.pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::Bandwidth;

    // 8 Mbps
    #[allow(clippy::declare_interior_mutable_const)]
    const BW: Bandwidth = Bandwidth::new(8_000_000);

    #[test]
    fn default() {
        let link = Link::default();
        let max = Bandwidth::MAX;

        assert_eq!(link.latency, Latency::default());
        assert_eq!(link.channel_forward.bandwidth(), &max);
        assert_eq!(link.channel_reverse.bandwidth(), &max);
    }

    #[test]
    fn create_link() {
        let link = Link::new(Latency::ZERO, BW, PacketLoss::default());
        let ch = link.channel(LinkDirection::Forward);
        assert_eq!(ch.bytes_in_transit(), 0);
    }

    #[test]
    fn channel_resets_state() {
        let latency = Latency::new(Duration::from_secs(1));
        let link = Link::new(latency, BW, PacketLoss::default());
        let mut ch = link.channel(LinkDirection::Forward);
        let round = Round::ZERO.next();

        ch.process(24);
        ch.update_capacity(round, Duration::from_millis(500));

        // A second channel from the same link starts fresh.
        let ch2 = link.channel(LinkDirection::Forward);

        assert_eq!(ch.bytes_in_transit(), 24);
        assert_eq!(ch2.bytes_in_transit(), 0);
        assert_eq!(ch2.rem_latency, Duration::from_secs(1));
        assert_eq!(ch2.round, Round::ZERO);

        // Both share the same underlying Arc.
        assert!(Arc::ptr_eq(&ch.channel, &ch2.channel));
    }

    #[test]
    fn process() {
        let mut ch = link_channel(Latency::ZERO, BW);
        let round = Round::ZERO.next();
        assert_eq!(ch.channel.capacity(), 0);

        let processed = ch.process(24);
        assert_eq!(processed, 0);
        assert_eq!(ch.bytes_in_transit(), 24);

        ch.update_capacity(round, Duration::from_micros(100));

        let processed = ch.process(200);
        assert_eq!(processed, 100);
        assert_eq!(ch.bytes_in_transit(), 124);

        let processed = ch.process(0);
        assert_eq!(processed, 0);
        assert_eq!(ch.bytes_in_transit(), 124);
    }

    #[test]
    fn process_latency() {
        let mut ch = link_channel(Latency::new(Duration::from_secs(1)), BW);
        let round = Round::ZERO.next();

        assert!(!ch.rem_latency.is_zero());
        assert!(!ch.completed());
        assert_eq!(ch.bytes_in_transit(), 0);

        ch.update_capacity(round, Duration::from_millis(500));
        assert!(!ch.rem_latency.is_zero());
        assert!(!ch.completed());

        // 1_500ms advance: 500ms latency consumed, 1_000ms left for bandwidth
        // capacity = 8 Mbps × 1_000ms = 1_000_000 bytes
        ch.update_capacity(round.next(), Duration::from_millis(1500));
        assert!(ch.rem_latency.is_zero());
        assert!(ch.completed());
        assert_eq!(ch.channel.capacity(), 1_000_000);
    }

    /// Verify that forward and reverse channels are fully independent: saturating
    /// one direction does not consume any capacity from the other.
    #[test]
    fn full_duplex_independence() {
        #[allow(clippy::declare_interior_mutable_const)]
        const BW100: Bandwidth = Bandwidth::new(800_000_000);

        let link = Link::new(Latency::ZERO, BW100, PacketLoss::default());
        let round = Round::ZERO.next();

        let mut fwd = link.channel(LinkDirection::Forward);
        let mut rev = link.channel(LinkDirection::Reverse);

        fwd.update_capacity(round, Duration::from_micros(1));
        rev.update_capacity(round, Duration::from_micros(1));

        let transited_fwd = fwd.process(100);
        assert_eq!(
            transited_fwd, 100,
            "forward should get its full 100-byte quota"
        );
        assert_eq!(fwd.bytes_in_transit(), 0);

        let transited_rev = rev.process(100);
        assert_eq!(
            transited_rev, 100,
            "reverse should be unaffected by forward saturation"
        );
        assert_eq!(rev.bytes_in_transit(), 0);
    }

    /// Verify that a shared-channel link demonstrates half-duplex behaviour:
    /// saturating one direction starves the other.
    #[test]
    fn shared_channel_is_half_duplex() {
        #[allow(clippy::declare_interior_mutable_const)]
        const BW100: Bandwidth = Bandwidth::new(800_000_000);

        let channel = Arc::new(CongestionChannel::new(BW100));
        let link = Link::new_with_channels(
            Latency::ZERO,
            Arc::clone(&channel),
            Arc::clone(&channel),
            PacketLoss::default(),
        );
        let round = Round::ZERO.next();

        let mut fwd = link.channel(LinkDirection::Forward);
        let mut rev = link.channel(LinkDirection::Reverse);

        fwd.update_capacity(round, Duration::from_micros(1));
        rev.update_capacity(round, Duration::from_micros(1));

        let transited_fwd = fwd.process(100);
        assert_eq!(transited_fwd, 100);

        let transited_rev = rev.process(100);
        assert_eq!(
            transited_rev, 0,
            "shared channel: reverse is starved after forward saturates"
        );
    }

    /// Helper: create a LinkChannel directly from latency + bandwidth.
    fn link_channel(latency: Latency, bandwidth: Bandwidth) -> LinkChannel {
        Link::new(latency, bandwidth, PacketLoss::default()).channel(LinkDirection::Forward)
    }
}
