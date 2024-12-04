use anyhow::{bail, ensure};
use logos::{Lexer, Logos};
use std::{fmt, str::FromStr, time::Duration};

/// The [`Bandwidth`] that can be used to determine how much
/// data can be processed during a certain [`Duration`].
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
/// // create a bandwidth of `1mbps`
/// let bw = Bandwidth::new(
///     2_000,
///     Duration::from_millis(1),
/// );
/// // get the capacity allowed by the bandwidth
/// // i.e. the number of bytes that can be transmitted
/// // during the given duration
/// let capacity = bw.capacity(Duration::from_micros(1));
/// # assert_eq!(capacity, 2);
/// ```
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bandwidth {
    /// bytes that can be processed per _duration_
    data: u64,
    /// the duration during which we can process _data_
    per: Duration,
}

impl Bandwidth {
    /// the maximum bandwidth available
    ///
    /// # note
    ///
    /// technically this isn't really the maximum bandwidth. This is still
    /// way too unrealistic bandwidth and will allow to pretend the network
    /// has an infinite bandwidth as it is.
    ///
    /// The actual maximum [Bandwidth] is:
    ///
    /// ```
    /// # use netsim_core::measure::Bandwidth;
    /// # use std::time::Duration;
    /// let real_max = Bandwidth::new(
    ///     u64::MAX,
    ///     Duration::from_nanos(1),
    /// );
    /// ```
    ///
    pub const MAX: Self = Self::new(u64::MAX, Duration::from_secs(1));

    /// create a new [`Bandwidth`]
    ///
    /// * data: the number of bytes that can be processed for the given duration
    /// * duration: the duration during which the data can be processed
    ///
    /// This allows to create different kind of [`Bandwidth`] depending on needs
    /// and requirements for accuracy:
    ///
    /// ```
    /// # use netsim_core::measure::Bandwidth;
    /// # use std::time::Duration;
    /// // create a bandwidth of `200mbps`
    /// let bw1 = Bandwidth::new(
    ///     200 * 1_024 * 1_024, // (200 MB)
    ///     Duration::from_secs(1),
    /// );
    /// // create a bandwidth of `300mb per 1.5s` or `200mbps`
    /// let bw2 = Bandwidth::new(
    ///     300 * 1_024 * 1_024, // (300 MB)
    ///     Duration::from_secs(1) + Duration::from_millis(500),
    /// );
    /// # assert_eq!(
    /// #   bw1.capacity(Duration::from_secs(1)),
    /// #   bw2.capacity(Duration::from_secs(1)),
    /// # )
    /// ```
    pub const fn new(data: u64, per: Duration) -> Self {
        Self { data, per }
    }

    /// the base time of the bandwidth
    ///
    /// Currently the default is to be in bits per seconds so that
    /// the time_base is always 1s. However for increased granularity
    /// we will want to allow things like `bytes per minutes` or
    /// even (1024MiB per 1.2 seconds).
    ///
    pub fn time_base(&self) -> Duration {
        self.per
    }

    /// return the data base used for the bandwith
    ///
    /// i.e. this is how many bytes per [`Self::time_base`]
    pub fn data_base(&self) -> u64 {
        self.data
    }

    /// returns how many bytes can be transfered during the
    /// elapsed time
    ///
    /// this function has a micro seconds precision to compute
    /// the data capacity for a given duration
    ///
    /// ```
    /// # use netsim_core::measure::Bandwidth;
    /// # use std::time::Duration;
    /// // create a bandwidth of `1mbps`
    /// let bw = Bandwidth::new(
    ///     1,
    ///     Duration::from_micros(1),
    /// );
    /// let capacity = bw.capacity(Duration::from_secs(1));
    /// # assert_eq!(capacity, 1_000_000);
    /// ```
    pub fn capacity(&self, elapsed: Duration) -> u64 {
        let elapsed = elapsed.as_micros();
        let time_base = self.time_base().as_micros();
        let data_base = self.data_base() as u128;

        data_base.saturating_mul(elapsed).saturating_div(time_base) as u64
    }
}

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
        macro_rules! assert_bandwidth {
            (($bandwidth:expr) == $string:literal) => {
                assert_eq!(
                    Bandwidth::new($bandwidth, Duration::from_secs(1)).to_string(),
                    $string
                );
            };
        }

        assert_bandwidth!((0) == "0bps");
        assert_bandwidth!((42) == "42bps");
        assert_bandwidth!((42 * K) == "42kbps");
        assert_bandwidth!((42 * M) == "42mbps");
        assert_bandwidth!((42 * G) == "42gbps");

        assert_bandwidth!((12_345) == "12345bps");
        assert_bandwidth!((12_345 * K) == "12345kbps");
        assert_bandwidth!((12_345 * M) == "12345mbps");
    }

    #[test]
    fn bandwidth_capacity_1bps() {
        let bandwidth = Bandwidth::new(1, Duration::from_secs(1));

        assert_eq!(bandwidth.capacity(Duration::from_micros(100)), 0);
        assert_eq!(bandwidth.capacity(Duration::from_millis(1)), 0);
        assert_eq!(bandwidth.capacity(Duration::from_secs(1)), 1);
        assert_eq!(bandwidth.capacity(Duration::from_secs(100)), 100);
    }

    // 12_000 bytes every  2.1 s
    //     12 bytes every  2.1 ms (2_100 μs)
    #[test]
    fn bandwidth_capacity_12kbp2s100ms() {
        let bandwidth = Bandwidth::new(12_000, Duration::from_secs(2) + Duration::from_millis(100));

        assert_eq!(bandwidth.capacity(Duration::from_micros(100)), 0);
        assert_eq!(bandwidth.capacity(Duration::from_millis(1)), 5);
        assert_eq!(bandwidth.capacity(Duration::from_secs(1)), 5714);
        assert_eq!(bandwidth.capacity(Duration::from_secs(100)), 571428);
    }
}
