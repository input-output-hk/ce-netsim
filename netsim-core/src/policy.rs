use crate::geo;
use crate::{
    defaults::{
        DEFAULT_DOWNLOAD_BANDWIDTH, DEFAULT_LATENCY, DEFAULT_PACKET_LOSS, DEFAULT_UPLOAD_BANDWIDTH,
    },
    HasBytesSize, Msg, SimId,
};
use anyhow::{bail, ensure};
use logos::{Lexer, Logos};
use std::{collections::HashMap, fmt::Display, str::FromStr, time::Duration};

pub enum PolicyOutcome {
    //TODO(nicolasdp): implement the drop strategy
    #[allow(unused)]
    Drop,
    Delay {
        delay: Duration,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bandwidth(
    /// bits per seconds
    u64,
);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Latency(Duration);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PacketLoss {
    n: u64,
    every: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Edge {
    smaller_id: SimId,
    larger_id: SimId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodePolicy {
    pub bandwidth_down: Bandwidth,
    pub bandwidth_up: Bandwidth,
    pub location: Option<(i64, u64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgePolicy {
    pub latency: Latency,
    pub bandwidth_down: Bandwidth,
    pub bandwidth_up: Bandwidth,
    pub packet_loss: PacketLoss,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Policy {
    default_node_policy: NodePolicy,
    default_edge_policy: EdgePolicy,

    node_policies: HashMap<SimId, NodePolicy>,
    edge_policies: HashMap<Edge, EdgePolicy>,
}

impl Bandwidth {
    pub const fn bits_per(bits: u64, duration: Duration) -> Self {
        Self(bits * duration.as_millis() as u64)
    }
}

impl Latency {
    #[inline(always)]
    pub const fn new(duration: Duration) -> Self {
        Self(duration)
    }

    pub(crate) fn to_duration(self) -> Duration {
        self.0
    }
}

impl PacketLoss {
    // No Packet loss
    pub const NONE: Self = Self::new(0, 1);

    pub const fn new(n: u64, every: u64) -> Self {
        Self { n, every }
    }
}

impl Default for Latency {
    fn default() -> Self {
        DEFAULT_LATENCY
    }
}

impl Edge {
    pub fn new((a, b): (SimId, SimId)) -> Self {
        if a < b {
            Self {
                smaller_id: a,
                larger_id: b,
            }
        } else {
            Self {
                smaller_id: b,
                larger_id: a,
            }
        }
    }
}

impl Default for NodePolicy {
    fn default() -> Self {
        Self {
            bandwidth_down: DEFAULT_DOWNLOAD_BANDWIDTH,
            bandwidth_up: DEFAULT_UPLOAD_BANDWIDTH,
            location: None,
        }
    }
}

impl Default for EdgePolicy {
    fn default() -> Self {
        Self {
            latency: DEFAULT_LATENCY,
            bandwidth_down: DEFAULT_DOWNLOAD_BANDWIDTH,
            bandwidth_up: DEFAULT_UPLOAD_BANDWIDTH,
            packet_loss: DEFAULT_PACKET_LOSS,
        }
    }
}

impl EdgePolicy {
    pub fn between_nodes(
        node_policies: &HashMap<SimId, NodePolicy>,
        a: SimId,
        b: SimId,
    ) -> EdgePolicy {
        let loc_a = node_policies.get(&a).and_then(|pol_a| pol_a.location);
        let loc_b = node_policies.get(&b).and_then(|pol_b| pol_b.location);

        let latency = if let Some(loc_a) = loc_a {
            if let Some(loc_b) = loc_b {
                geo::latency_between_locations(loc_a, loc_b, 1.0).unwrap_or(DEFAULT_LATENCY)
            } else {
                DEFAULT_LATENCY
            }
        } else {
            DEFAULT_LATENCY
        };

        Self {
            latency,
            ..Self::default()
        }
    }
}

impl Policy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default_node_policy(&self) -> NodePolicy {
        self.default_node_policy
    }

    pub fn default_edge_policy(&self) -> EdgePolicy {
        self.default_edge_policy
    }

    pub fn set_default_node_policy(&mut self, default_node_policy: NodePolicy) {
        self.default_node_policy = default_node_policy;
    }

    pub fn set_default_edge_policy(&mut self, default_edge_policy: EdgePolicy) {
        self.default_edge_policy = default_edge_policy;
    }

    pub fn get_node_policy(&self, node: SimId) -> Option<NodePolicy> {
        self.node_policies.get(&node).copied()
    }

    pub fn set_node_policy(&mut self, node: SimId, policy: NodePolicy) {
        self.node_policies.insert(node, policy);
    }

    pub fn reset_node_policy(&mut self, node: SimId) {
        self.node_policies.remove(&node);
    }

    pub fn get_edge_policy(&self, edge: Edge) -> Option<EdgePolicy> {
        self.edge_policies.get(&edge).copied()
    }

    pub fn set_edge_policy(&mut self, edge: Edge, policy: EdgePolicy) {
        self.edge_policies.insert(edge, policy);
    }

    pub fn reset_edge_policy(&mut self, edge: Edge) {
        self.edge_policies.remove(&edge);
    }

    fn message_delay<T>(&self, msg: &Msg<T>) -> Duration
    where
        T: HasBytesSize,
    {
        let from = msg.from();
        let to = msg.to();

        let edge = Edge::new((from, to));
        let edge_policy = self
            .get_edge_policy(edge)
            .unwrap_or_else(|| self.default_edge_policy());

        edge_policy.latency.to_duration()
    }

    pub(crate) fn process<T>(&mut self, msg: &Msg<T>) -> PolicyOutcome
    where
        T: HasBytesSize,
    {
        PolicyOutcome::Delay {
            delay: self.message_delay(msg),
        }
    }
}

impl Bandwidth {
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

const K: u64 = 1_024;
const M: u64 = 1_024 * 1_024;
const G: u64 = 1_024 * 1_024 * 1_024;

impl Display for Bandwidth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = self.0;
        let k = self.0 / K;
        let m = self.0 / M;
        let g = self.0 / G;

        let v_r = self.0 % K;
        let k_r = self.0 % M;
        let m_r = self.0 % G;

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

        Ok(Self(bps))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bandwidth() {
        macro_rules! assert_bandwidth {
            ($string:literal == $value:expr) => {
                assert_eq!($string.parse::<Bandwidth>().unwrap(), Bandwidth($value));
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
                assert_eq!(Bandwidth($bandwidth).to_string(), $string);
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
}
