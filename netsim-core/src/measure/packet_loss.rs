use rand_core::Rng;

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

impl PacketLoss {
    /// Returns `true` if this packet should be dropped.
    ///
    /// The caller provides `rng` so that all simulation randomness is
    /// controlled from a single, seedable source in [`Network`]. Any type
    /// that implements [`RngCore`] can be used, keeping this method
    /// independent of the concrete generator used by the network.
    ///
    /// [`Network`]: crate::network::Network
    pub fn should_drop<R: Rng>(&self, rng: &mut R) -> bool {
        match self {
            PacketLoss::None => false,
            PacketLoss::Rate(rate) => {
                // Map a u64 to [0, 1) and compare against the rate.
                let bits = rng.next_u64();
                let sample = (bits as f64) * (1.0 / (u64::MAX as f64 + 1.0));
                sample < *rate
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rand_chacha::ChaChaRng;
    use rand_core::SeedableRng as _;

    use super::*;

    fn rng() -> ChaChaRng {
        ChaChaRng::seed_from_u64(42)
    }

    #[test]
    fn none_never_drops() {
        let mut rng = rng();
        for _ in 0..1000 {
            assert!(!PacketLoss::None.should_drop(&mut rng));
        }
    }

    #[test]
    fn rate_zero_never_drops() {
        let mut rng = rng();
        for _ in 0..1000 {
            assert!(!PacketLoss::Rate(0.0).should_drop(&mut rng));
        }
    }

    #[test]
    fn rate_one_always_drops() {
        let mut rng = rng();
        for _ in 0..1000 {
            assert!(PacketLoss::Rate(1.0).should_drop(&mut rng));
        }
    }

    #[test]
    fn rate_half_approximately() {
        let loss = PacketLoss::Rate(0.5);
        let mut rng = rng();
        let drops: usize = (0..10_000).filter(|_| loss.should_drop(&mut rng)).count();
        // With 10k samples at 50% rate, expect between 45% and 55%
        assert!(
            drops > 4500 && drops < 5500,
            "drop rate was {}/10000",
            drops
        );
    }

    #[test]
    fn default_never_drops() {
        let mut rng = rng();
        for _ in 0..1000 {
            assert!(!PacketLoss::default().should_drop(&mut rng));
        }
    }

    #[test]
    fn rate_nan_never_drops() {
        // NaN comparisons are always false: sample < NaN is always false
        let mut rng = rng();
        for _ in 0..1000 {
            assert!(!PacketLoss::Rate(f64::NAN).should_drop(&mut rng));
        }
    }

    #[test]
    fn rate_above_one_always_drops() {
        // sample is in [0, 1) so always < 1.5
        let mut rng = rng();
        for _ in 0..1000 {
            assert!(PacketLoss::Rate(1.5).should_drop(&mut rng));
        }
    }

    #[test]
    fn rate_tenth_approximately() {
        let loss = PacketLoss::Rate(0.1);
        let mut rng = rng();
        let drops: usize = (0..10_000).filter(|_| loss.should_drop(&mut rng)).count();
        // With 10k samples at 10% rate, expect between 8% and 12%
        assert!(drops > 800 && drops < 1200, "drop rate was {}/10000", drops);
    }

    #[test]
    fn reproducible_with_same_seed() {
        let loss = PacketLoss::Rate(0.3);
        let results_a: Vec<bool> = {
            let mut rng = ChaChaRng::seed_from_u64(99);
            (0..100).map(|_| loss.should_drop(&mut rng)).collect()
        };
        let results_b: Vec<bool> = {
            let mut rng = ChaChaRng::seed_from_u64(99);
            (0..100).map(|_| loss.should_drop(&mut rng)).collect()
        };
        assert_eq!(results_a, results_b);
    }
}
