//! Network statistics and observability types.
//!
//! [`NetworkStats`] provides a point-in-time snapshot of the network state.
//! Obtain one via [`Network::stats`](crate::network::Network::stats).

use crate::{
    link::LinkId,
    measure::{Bandwidth, Latency, PacketLoss},
    node::NodeId,
};

/// Snapshot of statistics for a single node.
#[derive(Debug, Clone)]
pub struct NodeStats {
    /// The node's identifier.
    pub id: NodeId,
    /// Bytes currently occupying the upload (outbound) buffer.
    pub upload_buffer_used: u64,
    /// Maximum capacity of the upload buffer.
    pub upload_buffer_max: u64,
    /// Bytes currently occupying the download (inbound) buffer.
    pub download_buffer_used: u64,
    /// Maximum capacity of the download buffer.
    pub download_buffer_max: u64,
    /// Configured upload bandwidth for this node.
    pub upload_bandwidth: Bandwidth,
    /// Configured download bandwidth for this node.
    pub download_bandwidth: Bandwidth,
}

/// Snapshot of statistics for a single link.
#[derive(Debug, Clone)]
pub struct LinkStats {
    /// The link identifier (ordered pair of node IDs).
    pub id: LinkId,
    /// Configured latency of this link.
    pub latency: Latency,
    /// Configured bandwidth of this link (applies to both directions independently).
    pub bandwidth: Bandwidth,
    /// Configured packet loss model for this link.
    pub packet_loss: PacketLoss,
    /// Bytes currently pending in the link (after latency, before delivery).
    pub bytes_in_transit: u64,
}

/// Point-in-time snapshot of the entire network state.
#[derive(Debug, Clone)]
pub struct NetworkStats {
    /// Per-node statistics.
    pub nodes: Vec<NodeStats>,
    /// Per-link statistics.
    pub links: Vec<LinkStats>,
}
