use std::{
    fmt,
    ops::{Deref, DerefMut},
    str::FromStr,
    time::Duration,
};

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
/// In essence, if you were to send a [`Packet`] with with `0` [`Data`]
/// the [`Latency`] would be the exact amount of time it takes for this
/// empty message to travel.
///
/// [`Packet`]: crate::network::Packet
/// [`Data`]: crate::data::Data
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Latency(Duration);

impl Latency {
    /// The `0` latency. I.e. no latency.
    ///
    pub const ZERO: Self = Self::new(Duration::ZERO);

    /// create a new latency with the given [`Duration`].
    ///
    #[inline(always)]
    pub const fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// get the inner duration
    ///
    #[inline(always)]
    pub fn to_duration(self) -> Duration {
        self.0
    }
}

impl From<Latency> for Duration {
    fn from(value: Latency) -> Self {
        value.0
    }
}
impl From<Duration> for Latency {
    fn from(value: Duration) -> Self {
        Self::new(value)
    }
}
impl Deref for Latency {
    type Target = Duration;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Latency {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl AsRef<Duration> for Latency {
    fn as_ref(&self) -> &Duration {
        &self.0
    }
}

impl Default for Latency {
    fn default() -> Self {
        crate::defaults::DEFAULT_LATENCY
    }
}

impl fmt::Display for Latency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dur = crate::time::Duration::new(self.0);
        dur.fmt(f)
    }
}

impl FromStr for Latency {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let duration = crate::time::Duration::from_str(s)?;

        Ok(Self(duration.into_duration()))
    }
}
