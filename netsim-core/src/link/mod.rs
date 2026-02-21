mod id;

use crate::{
    measure::{Bandwidth, CongestionChannel, Latency, PacketLoss},
    network::Round,
};
use std::{sync::Arc, time::Duration};

pub use self::id::LinkId;

/// A link state between two [`Node`]s.
///
/// The [`Link`] is responsible for maintaining the congestion [`Bandwidth`]
/// between two nodes and for maintaining the [`Latency`] and [`PacketLoss`].
///
/// [`Node`]: crate::node::Node
/// [`Bandwidth`]: crate::measure::Bandwidth
#[derive(Debug, Default)]
pub struct Link {
    pending: u64,
    rem_latency: Duration,

    channel: Arc<CongestionChannel>,
    latency: Latency,
    packet_loss: PacketLoss,
    round: Round,
}

impl Link {
    pub fn new(latency: Latency, channel: Arc<CongestionChannel>) -> Self {
        Self {
            pending: 0,
            rem_latency: latency.into_duration(),
            channel,
            latency,
            packet_loss: PacketLoss::default(),
            round: Round::default(),
        }
    }

    pub fn new_with_loss(
        latency: Latency,
        channel: Arc<CongestionChannel>,
        packet_loss: PacketLoss,
    ) -> Self {
        Self {
            pending: 0,
            rem_latency: latency.into_duration(),
            channel,
            latency,
            packet_loss,
            round: Round::default(),
        }
    }

    /// Returns `true` if this packet should be dropped based on the link's packet loss model.
    pub fn should_drop_packet(&self) -> bool {
        self.packet_loss.should_drop()
    }

    /// Returns the packet loss configuration for this link.
    pub fn packet_loss(&self) -> PacketLoss {
        self.packet_loss
    }

    /// create a new [`Link`] off this link. However the pending
    /// data and the current round and the consummed latency are
    /// reset
    pub(crate) fn duplicate(&self) -> Self {
        Self::new_with_loss(self.latency, self.channel.clone(), self.packet_loss)
    }

    pub fn update_capacity(&mut self, round: Round, duration: Duration) {
        if self.round != round {
            self.round = round;
            let min = std::cmp::min(self.rem_latency, duration);
            self.rem_latency = self.rem_latency.saturating_sub(duration);
            let rem = duration.saturating_sub(min);

            self.channel.update_capacity(round, rem);
        }
    }

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

    pub fn completed(&self) -> bool {
        self.pending == 0 && self.rem_latency.is_zero()
    }

    pub fn bytes_in_transit(&self) -> u64 {
        self.pending
    }

    /// Returns the configured latency for this link.
    pub fn latency(&self) -> Latency {
        self.latency
    }

    /// Returns the configured bandwidth for this link.
    pub fn bandwidth(&self) -> Bandwidth {
        self.channel.bandwidth()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::Bandwidth;

    const BW: Bandwidth = Bandwidth::new(1_024, Duration::from_secs(1));

    #[test]
    fn default() {
        let link = Link::default();

        assert_eq!(link.pending, 0);
        assert_eq!(link.round, Round::ZERO);
        assert_eq!(link.latency, Latency::default());
        assert_eq!(link.channel.bandwidth(), Bandwidth::MAX);
    }

    #[test]
    fn create_link() {
        let channel = Arc::new(CongestionChannel::new(BW));
        let link = Link::new(Latency::ZERO, channel);

        assert_eq!(link.bytes_in_transit(), 0);
    }

    #[test]
    fn duplicate() {
        let channel = Arc::new(CongestionChannel::new(BW));
        let latency = Latency::new(Duration::from_secs(1));
        let mut link1 = Link::new(latency, channel);
        let round = Round::ZERO.next();

        link1.process(24);
        link1.update_capacity(round, Duration::from_millis(500));

        let link2 = link1.duplicate();

        // pending bytes is reset on duplicate
        assert_eq!(link1.pending, 24);
        assert_eq!(link2.pending, 0);

        // round is reset on duplicate
        assert_eq!(link1.round, Round::ZERO.next());
        assert_eq!(link2.round, Round::ZERO);

        // latency is reset on duplicate
        assert_eq!(link1.rem_latency, Duration::from_millis(500));
        assert_eq!(link2.rem_latency, Duration::from_secs(1));

        assert_eq!(link1.latency, latency);
        assert_eq!(link2.latency, latency);
        assert_eq!(link1.channel.capacity(), link2.channel.capacity());
        assert!(Arc::ptr_eq(&link1.channel, &link2.channel));
    }

    #[test]
    fn process() {
        let channel = Arc::new(CongestionChannel::new(BW));
        let mut link = Link::new(Latency::ZERO, channel);

        let round = Round::ZERO.next();
        assert_eq!(link.channel.capacity(), 0);

        // we expect 0 bytes since we haven't called the function to update the capacity
        // of the channel
        let processed = link.process(24);
        assert_eq!(processed, 0);

        // the bytes remain in transit until the capacity is updated and the process function
        // is called again
        assert_eq!(link.bytes_in_transit(), 24);

        link.update_capacity(round, Duration::from_secs(1));

        // we ask the link to process 100 additional bytes to the 24 already
        // in transit.
        let processed = link.process(100);
        assert_eq!(processed, 124);
        assert_eq!(link.bytes_in_transit(), 0);

        // if we ask for more than remaining then the left over
        // will remain in the `in_transit` buffer.
        let processed = link.process(1_000);
        assert_eq!(processed, 900);
        assert_eq!(link.bytes_in_transit(), 100);
    }

    #[test]
    fn process_latency() {
        let channel = Arc::new(CongestionChannel::new(BW));
        let mut link = Link::new(Latency::new(Duration::from_secs(1)), channel);

        let round = Round::ZERO.next();

        assert!(!link.rem_latency.is_zero());
        assert!(!link.completed());
        assert_eq!(link.bytes_in_transit(), 0);

        link.update_capacity(round, Duration::from_millis(500));
        assert!(!link.rem_latency.is_zero());
        assert!(!link.completed());
        assert_eq!(link.bytes_in_transit(), 0);

        link.update_capacity(round.next(), Duration::from_millis(1500));
        assert!(link.rem_latency.is_zero());
        assert!(link.completed());
        assert_eq!(link.channel.capacity(), 1_024);
    }
}
