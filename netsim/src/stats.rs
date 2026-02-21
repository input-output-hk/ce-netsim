//! Network observability types for [`SimContext`].
//!
//! Obtain a snapshot via [`SimContext::stats`](crate::SimContext::stats).

pub use netsim_core::stats::LinkStats;

/// Statistics for a single node in a [`SimContext`].
#[derive(Debug, Clone)]
pub struct NodeStats {
    /// Core node statistics (buffer usage, bandwidth).
    pub inner: netsim_core::NodeStats,
    /// Number of packets dropped for this node due to a full sender buffer.
    pub packets_dropped: u64,
}

/// Point-in-time snapshot of the entire simulated network.
#[derive(Debug, Clone)]
pub struct SimStats {
    /// Per-node statistics (includes drop counters).
    pub nodes: Vec<NodeStats>,
    /// Per-link statistics (latency, bandwidth, packet loss, bytes in transit).
    pub links: Vec<LinkStats>,
}
