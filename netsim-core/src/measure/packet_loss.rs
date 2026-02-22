use rand_chacha::ChaChaRng;
use rand_core::SeedableRng as _;

/// Probabilistic packet loss model for a network link.
///
/// Configures what fraction of packets are silently dropped on a link
/// before they enter transit.
///
/// # Example
///
/// ```
/// use netsim_core::PacketLoss;
///
/// // No packet loss
/// let none = PacketLoss::None;
///
/// // 5% packet loss
/// let lossy = PacketLoss::Rate(0.05);
/// ```
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum PacketLoss {
    /// No packet loss. All packets are forwarded (default).
    #[default]
    None,
    /// Random packet loss at the given rate.
    ///
    /// `rate` must be in the range `[0.0, 1.0]`. A value of `0.0` means no
    /// loss; `1.0` means all packets are dropped.
    Rate(f64),
}

pub struct PacketLossController {
    rng: ChaChaRng,
    cfg: PacketLoss,
}

impl std::fmt::Debug for PacketLossController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PacketLossController")
            .field("cfg", &self.cfg)
            .finish_non_exhaustive()
    }
}

impl PacketLossController {
    pub fn new(cfg: PacketLoss, rng: ChaChaRng) -> Self {
        Self { rng, cfg }
    }

    /// Creates a controller seeded from a `u64`.
    pub fn from_seed(cfg: PacketLoss, seed: u64) -> Self {
        Self::new(cfg, ChaChaRng::seed_from_u64(seed))
    }

    /// Returns the configured [`PacketLoss`] policy.
    pub fn cfg(&self) -> PacketLoss {
        self.cfg
    }

    /// Returns `true` if this packet should be dropped.
    pub fn should_drop(&mut self) -> bool {
        match self.cfg {
            PacketLoss::None => false,
            PacketLoss::Rate(rate) => {
                use rand_core::Rng as _;

                // Map a u64 to [0, 1) and compare against the rate.
                let bits = self.rng.next_u64();
                let sample = (bits as f64) * (1.0 / (u64::MAX as f64 + 1.0));
                sample < rate
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller(cfg: PacketLoss) -> PacketLossController {
        PacketLossController::from_seed(cfg, 42)
    }

    #[test]
    fn none_never_drops() {
        let mut c = controller(PacketLoss::None);
        for _ in 0..1000 {
            assert!(!c.should_drop());
        }
    }

    #[test]
    fn rate_zero_never_drops() {
        let mut c = controller(PacketLoss::Rate(0.0));
        for _ in 0..1000 {
            assert!(!c.should_drop());
        }
    }

    #[test]
    fn rate_one_always_drops() {
        let mut c = controller(PacketLoss::Rate(1.0));
        for _ in 0..1000 {
            assert!(c.should_drop());
        }
    }

    #[test]
    fn rate_half_approximately() {
        let mut c = controller(PacketLoss::Rate(0.5));
        let drops: usize = (0..10_000).filter(|_| c.should_drop()).count();
        // With 10k samples at 50% rate, expect between 45% and 55%
        assert!(
            drops > 4500 && drops < 5500,
            "drop rate was {}/10000",
            drops
        );
    }

    #[test]
    fn default_never_drops() {
        let mut c = controller(PacketLoss::default());
        for _ in 0..1000 {
            assert!(!c.should_drop());
        }
    }

    #[test]
    fn rate_nan_never_drops() {
        // NaN comparisons are always false: sample < NaN is always false
        let mut c = controller(PacketLoss::Rate(f64::NAN));
        for _ in 0..1000 {
            assert!(!c.should_drop());
        }
    }

    #[test]
    fn rate_above_one_always_drops() {
        // sample is in [0, 1) so always < 1.5
        let mut c = controller(PacketLoss::Rate(1.5));
        for _ in 0..1000 {
            assert!(c.should_drop());
        }
    }

    #[test]
    fn rate_tenth_approximately() {
        let mut c = controller(PacketLoss::Rate(0.1));
        let drops: usize = (0..10_000).filter(|_| c.should_drop()).count();
        // With 10k samples at 10% rate, expect between 8% and 12%
        assert!(drops > 800 && drops < 1200, "drop rate was {}/10000", drops);
    }

    #[test]
    fn reproducible_with_same_seed() {
        let results_a: Vec<bool> = {
            let mut c = PacketLossController::from_seed(PacketLoss::Rate(0.3), 99);
            (0..100).map(|_| c.should_drop()).collect()
        };
        let results_b: Vec<bool> = {
            let mut c = PacketLossController::from_seed(PacketLoss::Rate(0.3), 99);
            (0..100).map(|_| c.should_drop()).collect()
        };
        assert_eq!(results_a, results_b);
    }
}
