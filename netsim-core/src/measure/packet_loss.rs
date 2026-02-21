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
    /// Uses the thread-local RNG from the `rand` crate.
    pub fn should_drop(&self) -> bool {
        match self {
            Self::None => false,
            Self::Rate(rate) => {
                use rand::Rng as _;
                rand::thread_rng().gen::<f64>() < *rate
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_never_drops() {
        for _ in 0..1000 {
            assert!(!PacketLoss::None.should_drop());
        }
    }

    #[test]
    fn rate_zero_never_drops() {
        for _ in 0..1000 {
            assert!(!PacketLoss::Rate(0.0).should_drop());
        }
    }

    #[test]
    fn rate_one_always_drops() {
        for _ in 0..1000 {
            assert!(PacketLoss::Rate(1.0).should_drop());
        }
    }

    #[test]
    fn rate_half_approximately() {
        let drops: usize = (0..10_000)
            .filter(|_| PacketLoss::Rate(0.5).should_drop())
            .count();
        // With 10k samples at 50% rate, expect between 45% and 55%
        assert!(
            drops > 4500 && drops < 5500,
            "drop rate was {}/10000",
            drops
        );
    }
}
