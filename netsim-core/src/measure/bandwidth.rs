use anyhow::{bail, ensure};
use logos::{Lexer, Logos};
use std::{
    fmt,
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

/// The [`Bandwidth`] that can be used to determine how much
/// data can be processed during a certain [`Duration`].
///
/// Internally stores bytes per microsecond as an [`AtomicU64`], enabling
/// lock-free reads and writes. The conversion from the `(data, per)` pair
/// happens at construction time and is lossy for bandwidths below 1 byte/µs
/// (i.e. slower than 1 Mbps); such rates are rounded down to 0 bytes/µs.
///
/// # Default
///
/// if using the [`Default`] bandwidth the bandwidth is then at
/// the maximum (or more precisely [`Bandwidth::MAX`]).
///
/// # Example
///
/// ```
/// # use netsim_core::measure::Bandwidth;
/// # use std::time::Duration;
/// // create a bandwidth of `2mbps`
/// let bw = Bandwidth::new(
///     2_000_000,
///     Duration::from_secs(1),
/// );
/// // get the capacity allowed by the bandwidth
/// // i.e. the number of bytes that can be transmitted
/// // during the given duration
/// let capacity = bw.capacity(Duration::from_micros(1));
/// # assert_eq!(capacity, 2);
/// ```
///
pub struct Bandwidth(AtomicU64);

impl Bandwidth {
    /// the maximum bandwidth available
    ///
    /// Stores [`u64::MAX`] bytes per microsecond, which is effectively
    /// unlimited for any realistic simulation.
    ///
    pub const MAX: Self = Self::new(u64::MAX, Duration::from_secs(1));

    /// create a new [`Bandwidth`]
    ///
    /// * `data`: the number of bytes that can be processed for the given duration
    /// * `per`: the duration during which the data can be processed
    ///
    /// The `(data, per)` pair is converted to bytes per microsecond at
    /// construction time. Bandwidths below 1 byte/µs (< ~1 Mbps) are stored
    /// as 0 bytes/µs.
    ///
    /// ```
    /// # use netsim_core::measure::Bandwidth;
    /// # use std::time::Duration;
    /// // create a bandwidth of `200mbps`
    /// let bw = Bandwidth::new(
    ///     200 * 1_024 * 1_024, // (200 MB)
    ///     Duration::from_secs(1),
    /// );
    /// ```
    pub const fn new(data: u64, per: Duration) -> Self {
        let per_us = per.as_micros() as u64;
        let bpu = if per_us == 0 { u64::MAX } else { data / per_us };
        Self(AtomicU64::new(bpu))
    }

    /// Returns how many bytes can be transferred during `elapsed`.
    ///
    /// ```
    /// # use netsim_core::measure::Bandwidth;
    /// # use std::time::Duration;
    /// // 2 bytes/µs → 2_000_000 bytes in 1 second
    /// let bw = Bandwidth::new(2_000_000, Duration::from_secs(1));
    /// let capacity = bw.capacity(Duration::from_secs(1));
    /// # assert_eq!(capacity, 2_000_000);
    /// ```
    pub fn capacity(&self, elapsed: Duration) -> u64 {
        let bpu = self.0.load(Ordering::Relaxed) as u128;
        let us = elapsed.as_micros();
        bpu.saturating_mul(us).min(u64::MAX as u128) as u64
    }

    /// Returns the raw bytes-per-microsecond value.
    pub fn bytes_per_us(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }

    /// Overwrites the bandwidth with a new `(data, per)` pair.
    pub fn set(&self, this: Bandwidth) {
        let bpu = this.0.load(Ordering::Relaxed);
        self.0.store(bpu, Ordering::Relaxed);
    }
}

// --- Manual trait impls needed because AtomicU64 doesn't derive them ---

impl Clone for Bandwidth {
    fn clone(&self) -> Self {
        Self(AtomicU64::new(self.0.load(Ordering::Relaxed)))
    }
}

impl PartialEq for Bandwidth {
    fn eq(&self, other: &Self) -> bool {
        self.0.load(Ordering::Relaxed) == other.0.load(Ordering::Relaxed)
    }
}

impl Eq for Bandwidth {}

impl PartialOrd for Bandwidth {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Bandwidth {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .load(Ordering::Relaxed)
            .cmp(&other.0.load(Ordering::Relaxed))
    }
}

impl std::hash::Hash for Bandwidth {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.load(Ordering::Relaxed).hash(state);
    }
}

impl fmt::Debug for Bandwidth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Bandwidth")
            .field(&self.0.load(Ordering::Relaxed))
            .finish()
    }
}

// --- Display ---

const K: u64 = 1_024;
const M: u64 = 1_024 * 1_024;
const G: u64 = 1_024 * 1_024 * 1_024;

impl fmt::Display for Bandwidth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let capacity = self.capacity(Duration::from_secs(1));

        let v = capacity;
        let k = capacity / K;
        let m = capacity / M;
        let g = capacity / G;

        let v_r = capacity % K;
        let k_r = capacity % M;
        let m_r = capacity % G;

        if v < K || v_r != 0 {
            write!(f, "{v}bps")
        } else if v < M || k_r != 0 {
            write!(f, "{k}kbps")
        } else if v < G || m_r != 0 {
            write!(f, "{m}mbps")
        } else {
            write!(f, "{g}gbps")
        }
    }
}

// --- FromStr ---

#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"[ \t\n\f]+")] // Ignore this regex pattern between tokens
enum BandwidthToken {
    #[regex("bps")]
    Bps,
    #[regex("kbps")]
    Kbps,
    #[regex("mbps")]
    Mbps,
    #[regex("gbps")]
    Gbps,

    #[regex("[0-9]+")]
    Value,
}

impl FromStr for Bandwidth {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut lex = Lexer::<'_, BandwidthToken>::new(s);

        let Some(Ok(BandwidthToken::Value)) = lex.next() else {
            bail!("Expecting to parse a number")
        };
        let number: u64 = lex.slice().parse()?;
        let Some(Ok(token)) = lex.next() else {
            bail!("Expecting to parse a unit")
        };
        let bps = match token {
            BandwidthToken::Bps => number,
            BandwidthToken::Kbps => number * 1_024,
            BandwidthToken::Mbps => number * 1_024 * 1_024,
            BandwidthToken::Gbps => number * 1_024 * 1_024 * 1_024,
            BandwidthToken::Value => bail!("Expecting to parse a unit (bps, kbps, ...)"),
        };

        ensure!(
            lex.next().is_none(),
            "Not expecting any other tokens to parse a bandwidth"
        );

        Ok(Self::new(bps, Duration::from_secs(1)))
    }
}

impl Default for Bandwidth {
    fn default() -> Self {
        Bandwidth::MAX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bandwidth() {
        macro_rules! assert_bandwidth {
            ($string:literal == $value:expr) => {
                assert_eq!(
                    $string.parse::<Bandwidth>().unwrap(),
                    Bandwidth::new($value, Duration::from_secs(1))
                );
            };
        }

        assert_bandwidth!("0bps" == 0);
        assert_bandwidth!("42bps" == 42);
        assert_bandwidth!("42kbps" == 42 * 1_024);
        assert_bandwidth!("42mbps" == 42 * 1_024 * 1_024);
    }

    #[test]
    fn print_bandwidth() {
        // Values must be exact multiples of 1_000_000 (bytes/µs granularity).
        // Bandwidth::new(v, 1s) stores v / 1_000_000 bytes/µs; Display shows
        // that back as (v / 1_000_000) * 1_000_000 bytes/s.
        macro_rules! assert_bandwidth {
            (($bpu:expr) == $string:literal) => {
                // bpu = bytes per microsecond; construct via 1µs duration
                assert_eq!(
                    Bandwidth::new($bpu, Duration::from_micros(1)).to_string(),
                    $string
                );
            };
        }

        assert_bandwidth!((0) == "0bps");
        // 1 byte/µs = 1_000_000 bytes/s; 1_000_000 / K = 976 kbps (non-zero remainder), so bps
        assert_bandwidth!((1) == "1000000bps");
        // M bytes/µs = M * 1_000_000 bytes/s; divided by M = 1_000_000 mbps
        assert_bandwidth!((1 * M) == "1000000mbps");
        // G bytes/µs = G * 1_000_000 bytes/s; divided by G = 1_000_000 gbps
        assert_bandwidth!((1 * G) == "1000000gbps");
    }

    #[test]
    fn bandwidth_capacity_1_byte_per_us() {
        // 1 byte/µs = 1_000_000 bytes/s
        let bandwidth = Bandwidth::new(1, Duration::from_micros(1));

        assert_eq!(bandwidth.capacity(Duration::from_micros(1)), 1);
        assert_eq!(bandwidth.capacity(Duration::from_millis(1)), 1_000);
        assert_eq!(bandwidth.capacity(Duration::from_secs(1)), 1_000_000);
        assert_eq!(bandwidth.capacity(Duration::from_secs(100)), 100_000_000);
    }

    #[test]
    fn bandwidth_capacity_10_bytes_per_us() {
        // 10 bytes/µs = 10_000_000 bytes/s
        let bandwidth = Bandwidth::new(10, Duration::from_micros(1));

        assert_eq!(bandwidth.capacity(Duration::from_micros(1)), 10);
        assert_eq!(bandwidth.capacity(Duration::from_millis(1)), 10_000);
        assert_eq!(bandwidth.capacity(Duration::from_secs(1)), 10_000_000);
    }

    #[test]
    fn bandwidth_capacity_non_standard_duration() {
        // 2_100 bytes every 2_100 µs = 1 byte/µs
        let bandwidth = Bandwidth::new(2_100, Duration::from_micros(2_100));

        assert_eq!(bandwidth.capacity(Duration::from_micros(1)), 1);
        assert_eq!(bandwidth.capacity(Duration::from_millis(1)), 1_000);
        assert_eq!(bandwidth.capacity(Duration::from_secs(1)), 1_000_000);
    }

    #[test]
    fn zero_duration_gives_max_bandwidth() {
        // per_us == 0 → constructor stores u64::MAX bytes/µs
        let bw = Bandwidth::new(100, Duration::ZERO);
        assert_eq!(bw.bytes_per_us(), u64::MAX);
    }

    #[test]
    fn zero_bandwidth_capacity_always_zero() {
        let bw = Bandwidth::new(0, Duration::from_secs(1));
        assert_eq!(bw.capacity(Duration::from_micros(1)), 0);
        assert_eq!(bw.capacity(Duration::from_secs(1)), 0);
    }

    #[test]
    fn max_bandwidth_saturates_to_u64_max() {
        // Bandwidth::MAX = new(u64::MAX, 1s) = u64::MAX / 1_000_000 bytes/µs.
        // With a large enough duration the product overflows u128 → saturates to u64::MAX.
        // 1 second = 1_000_000 µs; bpu * 1_000_000 = (u64::MAX / 1_000_000) * 1_000_000
        // which is close to but not over u64::MAX, so we need a longer duration.
        assert_eq!(
            Bandwidth::MAX.capacity(Duration::from_secs(1_000_000)),
            u64::MAX
        );
    }

    #[test]
    fn parse_invalid_strings() {
        assert!("42".parse::<Bandwidth>().is_err()); // no unit
        assert!("mbps".parse::<Bandwidth>().is_err()); // no number
        assert!("".parse::<Bandwidth>().is_err()); // empty
        assert!("42mbps extra".parse::<Bandwidth>().is_err()); // trailing token
    }

    #[test]
    fn clone_is_independent() {
        let original = Bandwidth::new(5, Duration::from_micros(1));
        let clone = original.clone();
        // Mutate original via set()
        original.set(Bandwidth::new(10, Duration::from_micros(1)));
        // Clone must be unchanged
        assert_eq!(clone.bytes_per_us(), 5);
        assert_eq!(original.bytes_per_us(), 10);
    }

    #[test]
    fn ordering_and_eq() {
        let low = Bandwidth::new(1, Duration::from_micros(1));
        let high = Bandwidth::new(5, Duration::from_micros(1));
        let low2 = Bandwidth::new(1, Duration::from_micros(1));

        assert!(low < high);
        assert!(high > low);
        assert_eq!(low, low2);
        assert_ne!(low, high);
    }
}
