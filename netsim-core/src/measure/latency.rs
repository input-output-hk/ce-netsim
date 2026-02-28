use std::{fmt, str::FromStr, time::Duration};

/// The latency is a measure of how much a signal takes to
/// travel between two points.
///
/// # Default [`Latency`]
///
/// ```
/// # use netsim_core::measure::Latency;
/// assert_eq!(
///     Latency::default().to_string(),
///     "5ms"
/// )
/// ```
///
/// # about packets of `0` bytes size
///
/// In essence, if you were to send a [`Packet`] with `0` [`Data`]
/// the [`Latency`] would be the exact amount of time it takes for this
/// empty message to travel.
///
/// [`Packet`]: crate::network::Packet
/// [`Data`]: crate::data::Data
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Latency(u64);

impl Latency {
    /// The `0` latency. I.e. no latency.
    ///
    pub const ZERO: Self = Self::new(Duration::ZERO);

    /// create a new latency with the given [`Duration`].
    ///
    /// # truncation
    ///
    /// The latency is precise up to the micro seconds. Constructing a
    /// [`Latency`] from a [`Duration`] that contains nano seconds
    /// precision value will truncate the nano seconds part.
    ///
    /// ```
    /// # use netsim_core::measure::Latency;
    /// # use std::time::Duration;
    /// let latency = Latency::new(Duration::from_nanos(987_654_321));
    /// assert_eq!(
    ///     latency.into_duration(),
    ///     Duration::from_micros(987_654),
    /// );
    /// ```
    ///
    #[inline(always)]
    pub const fn new(duration: Duration) -> Self {
        Self(duration.as_micros() as u64)
    }

    /// get the inner duration
    ///
    #[inline(always)]
    pub fn into_duration(self) -> Duration {
        Duration::from_micros(self.0)
    }
}

impl From<Latency> for Duration {
    fn from(value: Latency) -> Self {
        value.into_duration()
    }
}
impl From<Duration> for Latency {
    fn from(value: Duration) -> Self {
        Self::new(value)
    }
}

impl Default for Latency {
    fn default() -> Self {
        crate::defaults::DEFAULT_LATENCY
    }
}

impl fmt::Display for Latency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dur = crate::time::Duration::new(self.into_duration());
        dur.fmt(f)
    }
}

impl FromStr for Latency {
    type Err = crate::time::DurationParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let duration = crate::time::Duration::from_str(s)?;

        Ok(Self::new(duration.into_duration()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default() {
        assert_eq!(Latency::default(), crate::defaults::DEFAULT_LATENCY,);
    }

    #[test]
    fn truncate() {
        assert_eq!(
            Latency::new(Duration::from_nanos(9_876_543_210)).into_duration(),
            Duration::from_micros(9_876_543),
        )
    }

    #[test]
    fn display() {
        assert_eq!(
            Latency::new(Duration::from_millis(150)).to_string(),
            "150ms"
        );

        assert_eq!(
            Latency::new(Duration::from_millis(1_542)).to_string(),
            "1s542ms"
        );

        assert_eq!(Latency::new(Duration::from_nanos(1_542)).to_string(), "1µs");
    }

    #[test]
    fn parse() {
        assert_eq!(
            Latency::new(Duration::from_millis(150)),
            "150ms".parse().unwrap(),
        );

        assert_eq!(
            Latency::new(Duration::from_millis(1_542)),
            "1s542ms".parse().unwrap(),
        );

        assert_eq!(
            Latency::new(Duration::from_nanos(1_542)),
            "1µs".parse().unwrap()
        );
    }

    #[test]
    fn zero_latency() {
        assert_eq!(Latency::ZERO.into_duration(), Duration::ZERO);
        assert_eq!(Latency::new(Duration::ZERO).into_duration(), Duration::ZERO);
    }

    #[test]
    fn sub_microsecond_truncates_to_zero() {
        // 999ns < 1µs → truncated to 0
        assert_eq!(
            Latency::new(Duration::from_nanos(999)).into_duration(),
            Duration::ZERO
        );
        // 1000ns = 1µs exactly
        assert_eq!(
            Latency::new(Duration::from_nanos(1_000)).into_duration(),
            Duration::from_micros(1)
        );
    }

    #[test]
    fn from_trait_impls() {
        let dur = Duration::from_millis(42);
        let lat = Latency::new(dur);

        // From<Latency> for Duration
        let back: Duration = lat.into();
        assert_eq!(back, dur);

        // From<Duration> for Latency
        let lat2: Latency = dur.into();
        assert_eq!(lat2, lat);
    }

    #[test]
    fn parse_invalid_strings() {
        assert!("150".parse::<Latency>().is_err());
        assert!("abc".parse::<Latency>().is_err());
        assert!("".parse::<Latency>().is_err());
    }

    #[test]
    fn display_round_trip() {
        let original = Latency::new(Duration::from_millis(150));
        let s = original.to_string();
        let parsed: Latency = s.parse().unwrap();
        assert_eq!(original, parsed);
    }
}
