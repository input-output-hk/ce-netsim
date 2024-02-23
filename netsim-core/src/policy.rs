use crate::{
    defaults::{
        DEFAULT_DOWNLOAD_BANDWIDTH, DEFAULT_LATENCY, DEFAULT_PACKET_LOSS, DEFAULT_UPLOAD_BANDWIDTH,
    },
    HasBytesSize, Msg, SimId,
};
use std::{
    cmp::min,
    collections::HashMap,
    time::{Duration, SystemTime},
};

pub enum PolicyOutcome {
    //TODO(nicolasdp): implement the drop strategy
    #[allow(unused)]
    Drop,
    Delay {
        until: SystemTime,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bandwidth(
    /// bits per seconds
    u128,
);

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgePolicy {
    pub latency: Latency,
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
        Self(bits as u128 * duration.as_millis())
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
        }
    }
}

impl Default for EdgePolicy {
    fn default() -> Self {
        Self {
            latency: DEFAULT_LATENCY,
            packet_loss: DEFAULT_PACKET_LOSS,
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

    fn message_due_time<T>(&self, msg: &Msg<T>) -> SystemTime
    where
        T: HasBytesSize,
    {
        let from = msg.from();
        let to = msg.to();
        let sent_time = msg.time();
        let msg_bits = msg.content().bytes_size() * 8;

        let upload_bandwidth = self
            .get_node_policy(from)
            .unwrap_or_else(|| self.default_node_policy())
            .bandwidth_up;
        let download_bandwidth = self
            .get_node_policy(to)
            .unwrap_or_else(|| self.default_node_policy())
            .bandwidth_down;
        let bandwidth = min(upload_bandwidth, download_bandwidth);

        let edge = Edge::new((from, to));
        let edge_policy = self
            .get_edge_policy(edge)
            .unwrap_or_else(|| self.default_edge_policy());

        let transfer_duration = Duration::from_millis((msg_bits as u128 / bandwidth.0) as u64);

        sent_time + edge_policy.latency.to_duration() + transfer_duration
    }

    pub(crate) fn process<T>(&mut self, msg: &Msg<T>) -> PolicyOutcome
    where
        T: HasBytesSize,
    {
        PolicyOutcome::Delay {
            until: self.message_due_time(msg),
        }
    }
}
