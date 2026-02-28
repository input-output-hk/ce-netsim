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
/// Internally stores **bits per second** (bps) as an [`AtomicU64`], enabling
/// lock-free reads and writes. Supports any rate from 1 bps up to ~18.4 Ebps
/// (effectively unlimited for any realistic simulation).
///
/// ## Constructing
///
/// Pass bits per second directly:
///
/// ```
/// # use netsim_core::measure::Bandwidth;
/// # use std::time::Duration;
/// let bw = Bandwidth::new(8_000_000); // 8 Mbps
/// let capacity = bw.capacity(Duration::from_micros(1));
/// # assert_eq!(capacity, 1);
/// ```
///
/// Or parse from a human-readable string using standard networking units
/// (bits, SI prefixes):
///
/// ```
/// # use netsim_core::measure::Bandwidth;
/// let bw: Bandwidth = "100mbps".parse().unwrap(); // 100 Mbit/s
/// ```
///
/// ## Minimum usable bandwidth and step size
///
/// [`Bandwidth::capacity`] uses integer arithmetic and always returns whole
/// bytes. If the configured bandwidth is very low relative to the `elapsed`
/// duration, the result may floor to **0 bytes** — meaning no data passes in
/// that round and the network appears stalled.
///
/// The minimum bandwidth that yields ≥ 1 byte per step depends on step size:
///
/// | Step size | Min bandwidth for ≥ 1 byte/step |
/// |-----------|----------------------------------|
/// | 200 µs (netsim default) | ~40 Kbps |
/// | 1 ms                    | ~8 Kbps  |
/// | 10 ms                   | ~800 bps |
///
/// This is a fundamental property of integer byte counting with discrete time
/// steps. Use a longer step duration with `Network::advance_with` for very
/// slow links.
///
/// # Default
///
/// The default bandwidth is [`Bandwidth::MAX`] (effectively unlimited).
///
pub struct Bandwidth(AtomicU64);

impl Bandwidth {
    /// The maximum bandwidth: [`u64::MAX`] bits per second, effectively unlimited
    /// for any realistic simulation.
    #[allow(clippy::declare_interior_mutable_const)]
    pub const MAX: Self = Self(AtomicU64::new(u64::MAX));

    /// Creates a new [`Bandwidth`] with the given bits-per-second rate.
    ///
    /// ```
    /// # use netsim_core::measure::Bandwidth;
    /// let bw = Bandwidth::new(100_000_000); // 100 Mbps
    /// ```
    pub const fn new(bits_per_sec: u64) -> Self {
        Self(AtomicU64::new(bits_per_sec))
    }

    /// Returns how many bytes can be transferred during `elapsed`.
    ///
    /// Uses integer arithmetic; may return 0 when `elapsed` is very short
    /// relative to the configured bandwidth — see the [struct-level
    /// documentation][Bandwidth] for the minimum usable bandwidth per step
    /// size.
    ///
    /// ```
    /// # use netsim_core::measure::Bandwidth;
    /// # use std::time::Duration;
    /// // 16 Mbps: 2_000_000 bytes per second
    /// let bw = Bandwidth::new(16_000_000);
    /// let capacity = bw.capacity(Duration::from_secs(1));
    /// # assert_eq!(capacity, 2_000_000);
    /// ```
    pub fn capacity(&self, elapsed: Duration) -> u64 {
        let bps = self.0.load(Ordering::Relaxed) as u128;
        let us = elapsed.as_micros();
        // bytes = bits_per_s × µs / (8 bits/byte × 1_000_000 µs/s)
        let bits = bps.saturating_mul(us);
        (bits / 8_000_000).min(u64::MAX as u128) as u64
    }

    /// Returns the raw bits-per-second value.
    pub fn bits_per_sec(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }

    /// Returns the minimum step [`Duration`] that yields ≥ 1 byte from
    /// [`Bandwidth::capacity`].
    ///
    /// Derived from the capacity formula: `capacity = bps × µs / 8_000_000`.
    /// Solving for the smallest `µs` that gives ≥ 1 byte:
    /// `µs_min = ⌈8_000_000 / bps⌉`.
    ///
    /// Returns [`Duration::ZERO`] when bandwidth is zero (capacity is always 0
    /// regardless of step size — no step helps).
    ///
    /// Use this to choose a suitable `duration` for [`Network::advance_with`],
    /// or check it via [`Network::minimum_step_duration`] across the whole
    /// network.
    ///
    /// ```
    /// # use netsim_core::measure::Bandwidth;
    /// # use std::time::Duration;
    /// // 40 Kbps is the minimum for the 200 µs netsim default step.
    /// assert_eq!(
    ///     Bandwidth::new(40_000).minimum_step_duration(),
    ///     Duration::from_micros(200),
    /// );
    /// ```
    ///
    /// [`Network::advance_with`]: crate::network::Network::advance_with
    /// [`Network::minimum_step_duration`]: crate::network::Network::minimum_step_duration
    pub fn minimum_step_duration(&self) -> Duration {
        let bps = self.bits_per_sec();
        if bps == 0 {
            return Duration::ZERO;
        }
        Duration::from_micros(8_000_000u64.div_ceil(bps))
    }

    /// Overwrites this bandwidth with a new value.
    pub fn set(&self, this: Bandwidth) {
        self.0
            .store(this.0.load(Ordering::Relaxed), Ordering::Relaxed);
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
//
// Uses SI (1000-based) prefixes, matching standard networking convention.
// Shows the largest unit that divides the value evenly.

const K: u64 = 1_000;
const M: u64 = 1_000_000;
const G: u64 = 1_000_000_000;

impl fmt::Display for Bandwidth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bps = self.bits_per_sec();
        let (divisor, unit) = if bps < K {
            return write!(f, "{bps}bps");
        } else if bps < M {
            (K, "kbps")
        } else if bps < G {
            (M, "mbps")
        } else {
            (G, "gbps")
        };

        if bps.is_multiple_of(divisor) {
            write!(f, "{}{unit}", bps / divisor)
        } else {
            let val = bps as f64 / divisor as f64;
            let s = format!("{val:.2}");
            let s = s.trim_end_matches('0');
            write!(f, "{s}{unit}")
        }
    }
}

// --- FromStr ---
//
// Parses standard networking units (bits, SI prefixes):
//   "1bps"  = 1 bit/s
//   "1kbps" = 1_000 bits/s
//   "1mbps" = 1_000_000 bits/s
//   "1gbps" = 1_000_000_000 bits/s

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
        let (multiplier, unit) = match token {
            BandwidthToken::Bps => (1, "bps"),
            BandwidthToken::Kbps => (K, "kbps"),
            BandwidthToken::Mbps => (M, "mbps"),
            BandwidthToken::Gbps => (G, "gbps"),
            BandwidthToken::Value => bail!("Expecting to parse a unit (bps, kbps, ...)"),
        };
        let Some(bps) = number.checked_mul(multiplier) else {
            bail!(
                "{number}{unit} overflows maximum bandwidth ({max})",
                max = Bandwidth::MAX
            )
        };

        ensure!(
            lex.next().is_none(),
            "Not expecting any other tokens to parse a bandwidth"
        );

        Ok(Self::new(bps))
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

    #[allow(clippy::declare_interior_mutable_const)]
    const ZERO: Bandwidth = Bandwidth(AtomicU64::new(0));

    /// After redesign to bits/s storage: the minimum representable non-zero
    /// bandwidth is 1 bit/s, parseable as "1bps".
    #[test]
    fn test_minimum_bandwidth() {
        let min = Bandwidth(AtomicU64::new(1));
        assert_eq!(min, "1bps".parse().unwrap());
    }

    /// All sub-megabit values are now representable and non-zero.
    #[test]
    fn test_bandwidth_isnt_floored_to_zero() {
        assert_ne!(ZERO, "1bps".parse().unwrap());
        assert_ne!(ZERO, "10bps".parse().unwrap());
        assert_ne!(ZERO, "100bps".parse().unwrap());
        assert_ne!(ZERO, "1kbps".parse().unwrap());
        assert_ne!(ZERO, "10kbps".parse().unwrap());
        assert_ne!(ZERO, "100kbps".parse().unwrap());
        assert_ne!(ZERO, "999kbps".parse().unwrap());
    }

    #[test]
    fn parse_bandwidth() {
        assert_eq!("0bps".parse::<Bandwidth>().unwrap().bits_per_sec(), 0);
        assert_eq!("42bps".parse::<Bandwidth>().unwrap().bits_per_sec(), 42);
        assert_eq!(
            "42kbps".parse::<Bandwidth>().unwrap().bits_per_sec(),
            42_000
        );
        assert_eq!(
            "42mbps".parse::<Bandwidth>().unwrap().bits_per_sec(),
            42_000_000
        );
        assert_eq!(
            "42gbps".parse::<Bandwidth>().unwrap().bits_per_sec(),
            42_000_000_000
        );
    }

    #[test]
    fn print_bandwidth() {
        // Exact SI multiples round-trip cleanly.
        assert_eq!(Bandwidth(AtomicU64::new(0)).to_string(), "0bps");
        assert_eq!(Bandwidth(AtomicU64::new(1)).to_string(), "1bps");
        assert_eq!(Bandwidth(AtomicU64::new(999)).to_string(), "999bps");
        assert_eq!(Bandwidth(AtomicU64::new(1_000)).to_string(), "1kbps");
        // Not an exact kbps multiple → fractional with trailing zeros trimmed
        assert_eq!(Bandwidth(AtomicU64::new(1_500)).to_string(), "1.5kbps");
        assert_eq!(Bandwidth(AtomicU64::new(42_000)).to_string(), "42kbps");
        assert_eq!(Bandwidth(AtomicU64::new(1_000_000)).to_string(), "1mbps");
        assert_eq!(Bandwidth(AtomicU64::new(42_000_000)).to_string(), "42mbps");
        assert_eq!(
            Bandwidth(AtomicU64::new(1_000_000_000)).to_string(),
            "1gbps"
        );
        assert_eq!(
            Bandwidth(AtomicU64::new(42_000_000_000)).to_string(),
            "42gbps"
        );
    }

    #[test]
    fn bandwidth_capacity_8mbps() {
        // 8 Mbps = 1 byte/µs
        let bandwidth = Bandwidth::new(8_000_000);

        assert_eq!(bandwidth.capacity(Duration::from_micros(1)), 1);
        assert_eq!(bandwidth.capacity(Duration::from_millis(1)), 1_000);
        assert_eq!(bandwidth.capacity(Duration::from_secs(1)), 1_000_000);
        assert_eq!(bandwidth.capacity(Duration::from_secs(100)), 100_000_000);
    }

    #[test]
    fn bandwidth_capacity_80mbps() {
        // 80 Mbps = 10 bytes/µs
        let bandwidth = Bandwidth::new(80_000_000);

        assert_eq!(bandwidth.capacity(Duration::from_micros(1)), 10);
        assert_eq!(bandwidth.capacity(Duration::from_millis(1)), 10_000);
        assert_eq!(bandwidth.capacity(Duration::from_secs(1)), 10_000_000);
    }

    #[test]
    fn bandwidth_capacity_sub_megabit() {
        // 100 Kbps link on a 1 ms step: 100_000 * 1000 / 8_000_000 = 12 bytes
        let bw = Bandwidth::new(100_000);
        assert_eq!(bw.capacity(Duration::from_millis(1)), 12);

        // 1 Kbps link on a 10 ms step: 1_000 * 10_000 / 8_000_000 = 1 byte
        let bw = Bandwidth::new(1_000);
        assert_eq!(bw.capacity(Duration::from_millis(10)), 1);
    }

    #[test]
    fn zero_bandwidth_capacity_always_zero() {
        let bw = Bandwidth::new(0);
        assert_eq!(bw.capacity(Duration::from_micros(1)), 0);
        assert_eq!(bw.capacity(Duration::from_secs(1)), 0);
    }

    #[test]
    fn max_bandwidth_saturates_to_u64_max() {
        let max = Bandwidth::MAX;
        // Bandwidth::MAX stores u64::MAX bits/s.
        assert_eq!(max.bits_per_sec(), u64::MAX);
        // capacity() over a long duration saturates to u64::MAX bytes.
        assert_eq!(max.capacity(Duration::from_secs(1_000_000)), u64::MAX);
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
        let original = Bandwidth::new(40_000_000); // 40 Mbps
        let clone = original.clone();
        original.set(Bandwidth::new(80_000_000)); // 80 Mbps
        assert_eq!(clone.bits_per_sec(), 40_000_000);
        assert_eq!(original.bits_per_sec(), 80_000_000);
    }

    #[test]
    fn minimum_step_duration() {
        // 8 Mbps: ceil(8_000_000 / 8_000_000) = 1 µs
        assert_eq!(
            Bandwidth::new(8_000_000).minimum_step_duration(),
            Duration::from_micros(1)
        );
        // 40 Kbps: ceil(8_000_000 / 40_000) = 200 µs (netsim multiplexer default step)
        assert_eq!(
            Bandwidth::new(40_000).minimum_step_duration(),
            Duration::from_micros(200)
        );
        // 1 Kbps: ceil(8_000_000 / 1_000) = 8_000 µs = 8 ms
        assert_eq!(
            Bandwidth::new(1_000).minimum_step_duration(),
            Duration::from_micros(8_000)
        );
        // 1 bps: ceil(8_000_000 / 1) = 8_000_000 µs = 8 s
        assert_eq!(
            Bandwidth::new(1).minimum_step_duration(),
            Duration::from_micros(8_000_000)
        );
        // Zero bandwidth: no step size helps
        assert_eq!(Bandwidth::new(0).minimum_step_duration(), Duration::ZERO);
    }

    /// Issue #13: parsing a bandwidth value that overflows u64 silently
    /// saturates to u64::MAX instead of returning a parse error.
    ///
    /// `99999999999gbps` = 99_999_999_999 × 1_000_000_000 which exceeds
    /// u64::MAX (18_446_744_073_709_551_615). The current implementation
    /// uses `saturating_mul`, so this silently becomes Bandwidth::MAX —
    /// indistinguishable from an intentional u64::MAX.
    #[test]
    #[should_panic]
    fn parse_overflow_silently_saturates() {
        let parsed: Bandwidth = "99999999999gbps".parse().unwrap();
        // BUG: this should be an error, but instead it silently becomes MAX
        assert_ne!(
            parsed.bits_per_sec(),
            u64::MAX,
            "overflowing parse silently saturated to u64::MAX"
        );
    }

    #[test]
    fn ordering_and_eq() {
        let low = Bandwidth::new(8_000_000);
        let high = Bandwidth::new(40_000_000);
        let low2 = Bandwidth::new(8_000_000);

        assert!(low < high);
        assert!(high > low);
        assert_eq!(low, low2);
        assert_ne!(low, high);
    }
}
