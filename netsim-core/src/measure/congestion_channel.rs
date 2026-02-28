use super::{Bandwidth, Gauge};
use crate::network::Round;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

/// # Congestion channel
///
/// This object allows to account for the [`Bandwidth`] and the associated
/// byte congestion effect that may occurs when we transmit messages.
///
/// Indeed the [`Bandwidth`] tells us how many bytes can travel during
/// a certain amount of time. This means that once the channel has reached
/// saturation, no more bytes should be travelling through it. I.e. when
/// we have reached the allocated [`Bandwidth::capacity`] it is then no longer
/// possible to transport anymore bytes.
///
/// # Default
///
/// It is possible to build a [`Default`] [`CongestionChannel`]. This
/// will set the [`Bandwidth`] to [`Bandwidth::MAX`] i.e. it will allocate
/// as much bandwidth as possible and no data congestion should be perceived.
///
#[derive(Debug, Default)]
pub struct CongestionChannel {
    bandwidth: Bandwidth,

    // atomic
    round: AtomicU64,

    // atomic operation
    gauge: Gauge,
}

impl CongestionChannel {
    /// create a new [`CongestionChannel`] with the given [`Bandwidth`]
    ///
    pub fn new(bandwidth: Bandwidth) -> Self {
        Self {
            bandwidth,
            round: AtomicU64::new(0),
            gauge: Gauge::with_capacity(0),
        }
    }

    #[inline]
    pub fn set_bandwidth(&self, bandwidth: Bandwidth) {
        self.bandwidth.set(bandwidth);
    }

    #[inline]
    pub fn bandwidth(&self) -> &Bandwidth {
        &self.bandwidth
    }

    #[inline]
    pub fn capacity(&self) -> u64 {
        self.gauge.remaining_capacity()
    }

    pub fn update_capacity(&self, round: Round, duration: Duration) -> bool {
        let new_round = round.into_u64();
        let mut old_round = self.round.load(Ordering::SeqCst);

        let update;

        loop {
            if old_round >= new_round {
                return false;
            }

            match self.round.compare_exchange_weak(
                old_round,
                new_round,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(old_round) => {
                    update = old_round != new_round;
                    debug_assert!(update, "at this point we should only update to newer round");
                    debug_assert!(
                        old_round < new_round,
                        "Expecting old round ({old_round}) to be smaller than new round ({new_round})"
                    );
                    break;
                }
                Err(next_old_round) => {
                    old_round = next_old_round;
                }
            }
        }

        if update {
            let capacity = self.bandwidth().capacity(duration);

            debug_assert!(
                capacity > 0 || self.bandwidth().bits_per_sec() == 0 || duration.is_zero(),
                "Bandwidth {}bps yields 0 bytes in a {:?} step — packets on this channel \
                 will stall silently. Minimum step for this bandwidth: {:?}.",
                self.bandwidth().bits_per_sec(),
                duration,
                self.bandwidth().minimum_step_duration(),
            );

            self.gauge.set_maximum_capacity(capacity);
            // Reset used capacity to zero for the new round. `free(u64::MAX)`
            // is clamped internally to the actual used amount, so this is
            // equivalent to "free everything" — giving the full `capacity`
            // budget to the upcoming time step.
            self.gauge.free(u64::MAX);
        }

        update
    }

    /// attempt to reserve up to `size` data and return how much data has been actually
    /// consummed
    pub fn reserve(&self, size: u64) -> u64 {
        self.gauge.reserve(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 8 Mbps
    #[allow(clippy::declare_interior_mutable_const)]
    const BD_8MBPS: Bandwidth = Bandwidth::new(8_000_000);

    /// test that the initial capacity is always 0
    #[test]
    fn initial_capacity() {
        let cc = CongestionChannel::new(BD_8MBPS);

        assert_eq!(cc.capacity(), 0);
    }

    /// we should not have round 0 when calling update_capacity
    ///
    /// However, just to be sure test that we don't have an update
    /// happening as expected.
    ///
    #[test]
    fn update_capacity_round_zero() {
        let cc = CongestionChannel::new(BD_8MBPS);
        let round = Round::new();

        let updated = cc.update_capacity(round, Duration::from_secs(1));

        assert!(!updated);
        assert_eq!(cc.capacity(), 0);
    }

    #[test]
    fn update_capacity_same_round() {
        let cc = CongestionChannel::new(BD_8MBPS);
        let round = Round::new().next();

        let updated = cc.update_capacity(round, Duration::from_secs(1));
        assert!(updated);
        assert_eq!(cc.capacity(), 1_000_000);

        let updated = cc.update_capacity(round, Duration::from_secs(1));
        assert!(!updated);
    }

    #[test]
    fn update_capacity_always_latest() {
        let cc = CongestionChannel::new(BD_8MBPS);
        let round = Round::new().next();

        let updated = cc.update_capacity(round, Duration::from_secs(100));
        assert!(updated);
        assert_eq!(cc.capacity(), 100_000_000);

        let updated = cc.update_capacity(round.next(), Duration::from_secs(1));
        assert!(updated);
        assert_eq!(cc.capacity(), 1_000_000);
    }

    #[test]
    fn update_capacity_zero_duration_gives_zero_capacity() {
        let cc = CongestionChannel::new(BD_8MBPS);
        let round = Round::new().next();

        let updated = cc.update_capacity(round, Duration::ZERO);
        assert!(updated);
        assert_eq!(cc.capacity(), 0);
    }

    #[test]
    fn set_bandwidth_takes_effect_on_next_round() {
        let cc = CongestionChannel::new(BD_8MBPS);
        let round = Round::new().next();

        // First round: 8 Mbps × 1 s = 1_000_000 bytes
        cc.update_capacity(round, Duration::from_secs(1));
        assert_eq!(cc.capacity(), 1_000_000);

        // Change to 16 Mbps
        cc.set_bandwidth(Bandwidth::new(16_000_000));

        // Next round: new bandwidth applies
        let updated = cc.update_capacity(round.next(), Duration::from_secs(1));
        assert!(updated);
        assert_eq!(cc.capacity(), 2_000_000);
    }

    #[test]
    fn reserve_reduces_capacity() {
        let cc = CongestionChannel::new(BD_8MBPS);
        let round = Round::new().next();

        cc.update_capacity(round, Duration::from_secs(1));
        assert_eq!(cc.capacity(), 1_000_000);

        let reserved = cc.reserve(400_000);
        assert_eq!(reserved, 400_000);
        assert_eq!(cc.capacity(), 600_000);
    }

    #[test]
    fn round_regression_does_not_update() {
        let cc = CongestionChannel::new(BD_8MBPS);
        let round = Round::new().next().next(); // round 2

        cc.update_capacity(round, Duration::from_secs(1));
        assert_eq!(cc.capacity(), 1_000_000);

        // Consume some capacity
        cc.reserve(200_000);

        // Attempt with same round again — should not restore capacity
        let updated = cc.update_capacity(round, Duration::from_secs(100));
        assert!(!updated);
        assert_eq!(cc.capacity(), 800_000); // still reduced
    }
}
