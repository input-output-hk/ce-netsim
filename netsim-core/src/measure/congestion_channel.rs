use super::{Bandwidth, Gauge};
use crate::network::Round;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        RwLock,
    },
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
/// # Thread Safety
///
/// The [`CongestionChannel`] is meant to be [`Send`] and [`Sync`]. it should
/// be possible to have concurrent threads utilise the same congestion channel
/// because we are using a [`RwLock`] for the configured [`Bandwidth`] and
/// the remaining [`Gauge`] is already containing atomic fields.
///
#[derive(Debug, Default)]
pub struct CongestionChannel {
    bandwidth: RwLock<Bandwidth>,

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
            bandwidth: RwLock::new(bandwidth),
            round: AtomicU64::new(0),
            gauge: Gauge::with_capacity(0),
        }
    }

    #[inline]
    pub fn set_bandwidth(&self, bandwidth: Bandwidth) {
        match self.bandwidth.try_write() {
            Ok(mut bw) => *bw = bandwidth,
            Err(error) => {
                panic!("failed to set bandwidth: {error}")
            }
        }
    }

    pub fn bandwidth(&self) -> Bandwidth {
        match self.bandwidth.try_read() {
            Ok(bw) => *bw,
            Err(error) => {
                panic!("failed to read bandwidth: {error}")
            }
        }
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

            self.gauge.set_maximum_capacity(capacity);
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

    const BD_1KBPS: Bandwidth = Bandwidth::new(1_024, Duration::from_secs(1));

    /// test that the initial capacity is always 0
    #[test]
    fn initial_capacity() {
        let cc = CongestionChannel::new(BD_1KBPS);

        assert_eq!(cc.capacity(), 0);
    }

    /// we should not have round 0 when calling update_capacity
    ///
    /// However, just to be sure test that we don't have an update
    /// happening as expected.
    ///
    #[test]
    fn update_capacity_round_zero() {
        let cc = CongestionChannel::new(BD_1KBPS);
        let round = Round::new();

        let updated = cc.update_capacity(round, Duration::from_secs(1));

        assert!(!updated);
        assert_eq!(cc.capacity(), 0);
    }

    #[test]
    fn update_capacity_same_round() {
        let cc = CongestionChannel::new(BD_1KBPS);
        let round = Round::new().next();

        let updated = cc.update_capacity(round, Duration::from_secs(1));
        assert!(updated);
        assert_eq!(cc.capacity(), 1_024);

        let updated = cc.update_capacity(round, Duration::from_secs(1));
        assert!(!updated);
    }

    #[test]
    fn update_capacity_always_latest() {
        let cc = CongestionChannel::new(BD_1KBPS);
        let round = Round::new().next();

        let updated = cc.update_capacity(round, Duration::from_secs(100));
        assert!(updated);
        assert_eq!(cc.capacity(), 102_400);

        let updated = cc.update_capacity(round.next(), Duration::from_secs(1));
        assert!(updated);
        assert_eq!(cc.capacity(), 1_024);
    }
}
