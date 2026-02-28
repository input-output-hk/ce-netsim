use rand_core::Rng;
use std::{fmt, str::FromStr};

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
/// // 5% packet loss (programmatic)
/// let lossy = PacketLoss::rate(0.05).unwrap();
/// assert_eq!(lossy.to_string(), "5%");
///
/// // 5% packet loss (parsed)
/// let parsed: PacketLoss = "5%".parse().unwrap();
/// assert_eq!(parsed, lossy);
/// ```
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum PacketLoss {
    /// No packet loss. All packets are forwarded (default).
    #[default]
    None,
    /// Random packet loss at the given rate (`0.0..=1.0`).
    ///
    /// Use [`PacketLoss::rate`] to construct this variant — it validates
    /// the value at creation time.
    Rate(PacketLossRate),
}

/// A validated packet loss rate in the range `[0.0, 1.0]`.
///
/// `0.0` means no loss; `1.0` means all packets are dropped.
/// Constructed via [`PacketLoss::rate`] which rejects NaN, negative,
/// and out-of-range values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PacketLossRate(f64);

impl PacketLoss {
    /// Create a `PacketLoss::Rate` with a validated loss probability.
    ///
    /// # Errors
    ///
    /// Returns an error if `rate` is not in `[0.0, 1.0]` (including NaN).
    pub fn rate(rate: f64) -> Result<Self, PacketLossRateError> {
        Ok(PacketLoss::Rate(PacketLossRate::new(rate)?))
    }

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
                let bits = rng.next_u64();
                let sample = (bits as f64) * (1.0 / (u64::MAX as f64 + 1.0));
                sample < rate.0
            }
        }
    }
}

impl fmt::Display for PacketLoss {
    /// Formats as a percentage with up to 2 decimal places.
    ///
    /// - `PacketLoss::None` → `"0%"`
    /// - `PacketLoss::Rate(0.05)` → `"5%"`
    /// - `PacketLoss::Rate(0.123)` → `"12.30%"`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PacketLoss::None => write!(f, "0%"),
            PacketLoss::Rate(rate) => write!(f, "{rate}"),
        }
    }
}

impl FromStr for PacketLoss {
    type Err = PacketLossParseError;

    /// Parses a percentage string like `"0%"`, `"5%"`, `"12.30%"`, `"100%"`.
    ///
    /// The `%` suffix is required.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let Some(num) = s.strip_suffix('%') else {
            return Err(PacketLossParseError::MissingSuffix);
        };
        let pct: f64 = num
            .trim()
            .parse()
            .map_err(|_| PacketLossParseError::InvalidNumber)?;
        let rate = pct / 100.0;
        if rate == 0.0 {
            return Ok(PacketLoss::None);
        }
        PacketLoss::rate(rate).map_err(PacketLossParseError::OutOfRange)
    }
}

impl fmt::Display for PacketLossRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pct = self.0 * 100.0;
        // If the percentage is a whole number, skip decimal places.
        if pct.fract() == 0.0 {
            write!(f, "{}%", pct as u64)
        } else {
            write!(f, "{:.2}%", pct)
        }
    }
}

impl PacketLossRate {
    /// Create a new validated rate.
    ///
    /// # Errors
    ///
    /// Returns [`PacketLossRateError`] if `rate` is NaN, negative, or
    /// greater than `1.0`.
    pub fn new(rate: f64) -> Result<Self, PacketLossRateError> {
        if !(0.0..=1.0).contains(&rate) {
            return Err(PacketLossRateError(rate));
        }
        Ok(Self(rate))
    }

    /// Returns the inner `f64` value.
    pub fn value(self) -> f64 {
        self.0
    }
}

/// Error returned when constructing a [`PacketLossRate`] with a value
/// outside `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("packet loss rate must be in [0.0, 1.0], got {0}")]
pub struct PacketLossRateError(f64);

/// Error returned when parsing a [`PacketLoss`] from a string.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PacketLossParseError {
    /// The string does not end with `%`.
    #[error("expected '%' suffix")]
    MissingSuffix,
    /// The numeric part could not be parsed as a float.
    #[error("invalid number before '%'")]
    InvalidNumber,
    /// The parsed percentage is outside `[0, 100]`.
    #[error("{0}")]
    OutOfRange(#[from] PacketLossRateError),
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
        let loss = PacketLoss::rate(0.0).unwrap();
        for _ in 0..1000 {
            assert!(!loss.should_drop(&mut rng));
        }
    }

    #[test]
    fn rate_one_always_drops() {
        let mut rng = rng();
        let loss = PacketLoss::rate(1.0).unwrap();
        for _ in 0..1000 {
            assert!(loss.should_drop(&mut rng));
        }
    }

    #[test]
    fn rate_half_approximately() {
        let loss = PacketLoss::rate(0.5).unwrap();
        let mut rng = rng();
        let drops: usize = (0..10_000).filter(|_| loss.should_drop(&mut rng)).count();
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
    fn rate_nan_rejected() {
        assert!(PacketLoss::rate(f64::NAN).is_err());
    }

    #[test]
    fn rate_negative_rejected() {
        assert!(PacketLoss::rate(-0.1).is_err());
    }

    #[test]
    fn rate_above_one_rejected() {
        assert!(PacketLoss::rate(1.5).is_err());
    }

    #[test]
    fn rate_tenth_approximately() {
        let loss = PacketLoss::rate(0.1).unwrap();
        let mut rng = rng();
        let drops: usize = (0..10_000).filter(|_| loss.should_drop(&mut rng)).count();
        assert!(drops > 800 && drops < 1200, "drop rate was {}/10000", drops);
    }

    #[test]
    fn reproducible_with_same_seed() {
        let loss = PacketLoss::rate(0.3).unwrap();
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

    #[test]
    fn error_display() {
        let err = PacketLoss::rate(2.0).unwrap_err();
        assert_eq!(
            err.to_string(),
            "packet loss rate must be in [0.0, 1.0], got 2"
        );
    }

    #[test]
    fn display_none() {
        assert_eq!(PacketLoss::None.to_string(), "0%");
    }

    #[test]
    fn display_whole_percent() {
        assert_eq!(PacketLoss::rate(0.05).unwrap().to_string(), "5%");
        assert_eq!(PacketLoss::rate(1.0).unwrap().to_string(), "100%");
    }

    #[test]
    fn display_fractional_percent() {
        assert_eq!(PacketLoss::rate(0.123).unwrap().to_string(), "12.30%");
        assert_eq!(PacketLoss::rate(0.015).unwrap().to_string(), "1.50%");
    }

    #[test]
    fn parse_none() {
        assert_eq!("0%".parse::<PacketLoss>().unwrap(), PacketLoss::None);
    }

    #[test]
    fn parse_whole_percent() {
        assert_eq!(
            "5%".parse::<PacketLoss>().unwrap(),
            PacketLoss::rate(0.05).unwrap()
        );
        assert_eq!(
            "100%".parse::<PacketLoss>().unwrap(),
            PacketLoss::rate(1.0).unwrap()
        );
    }

    #[test]
    fn parse_fractional_percent() {
        let parsed = "12.30%".parse::<PacketLoss>().unwrap();
        // Compare via display to avoid floating-point precision issues
        assert_eq!(parsed.to_string(), "12.30%");
    }

    #[test]
    fn parse_round_trip() {
        for rate in [0.0, 0.01, 0.05, 0.1, 0.5, 1.0] {
            let loss = if rate == 0.0 {
                PacketLoss::None
            } else {
                PacketLoss::rate(rate).unwrap()
            };
            let s = loss.to_string();
            let parsed: PacketLoss = s.parse().unwrap();
            assert_eq!(loss, parsed, "round-trip failed for {s}");
        }
    }

    #[test]
    fn parse_missing_suffix() {
        assert!("5".parse::<PacketLoss>().is_err());
    }

    #[test]
    fn parse_invalid_number() {
        assert!("abc%".parse::<PacketLoss>().is_err());
    }

    #[test]
    fn parse_out_of_range() {
        assert!("150%".parse::<PacketLoss>().is_err());
        assert!("-1%".parse::<PacketLoss>().is_err());
    }
}
