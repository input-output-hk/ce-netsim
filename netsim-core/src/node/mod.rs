mod id;

pub use self::id::NodeId;
use crate::{
    defaults::{DEFAULT_DOWNLOAD_BUFFER, DEFAULT_UPLOAD_BUFFER},
    measure::{Bandwidth, CongestionChannel, Download, Gauge, Upload},
};
use std::sync::Arc;

/// A simulated network endpoint managed by the [`Network`].
///
/// `Node` owns the upload and download congestion channels and byte-level
/// buffers that model a real host's network stack. You never construct a
/// `Node` directly — use [`Network::new_node`] to get a [`NodeBuilder`]
/// which registers the node and returns its [`NodeId`].
///
/// ## Data flow
///
/// ```text
/// Network::send()
///      │
///      ▼
/// [ upload buffer ] ── upload channel (bandwidth limit) ──►
///                                                          link (latency + bandwidth)
/// ◄── download channel (bandwidth limit) ─── [ download buffer ]
///                                                          │
///                                                    Network::advance_with()
///                                                    delivers packet to caller
/// ```
///
/// - The **upload buffer** holds bytes queued for sending. If it is full,
///   [`Network::send`] returns [`SendError::SenderBufferFull`].
/// - The **download buffer** holds bytes that have arrived but not yet been
///   read. If it overflows, the in-transit packet is marked corrupted and
///   silently dropped at delivery.
///
/// [`Network`]: crate::network::Network
/// [`Network::new_node`]: crate::network::Network::new_node
/// [`NodeBuilder`]: crate::network::NodeBuilder
/// [`Network::send`]: crate::network::Network::send
/// [`SendError::SenderBufferFull`]: crate::network::SendError::SenderBufferFull
pub struct Node {
    id: NodeId,

    /// the outbound buffer
    ///
    /// This is the amount of buffer available to submit to the network
    /// if the buffer is full then subsequent call to `send` a new message
    /// will fail
    outbound_buffer: Arc<Gauge>,
    outbound_channel: Arc<CongestionChannel>,

    inbound_channel: Arc<CongestionChannel>,
    inbound_buffer: Arc<Gauge>,
}

impl Node {
    pub(crate) fn new(id: NodeId) -> Self {
        Self {
            id,
            outbound_buffer: Arc::new(Gauge::with_capacity(DEFAULT_UPLOAD_BUFFER)),
            outbound_channel: Arc::new(CongestionChannel::new(Bandwidth::default())),
            inbound_channel: Arc::new(CongestionChannel::new(Bandwidth::default())),
            inbound_buffer: Arc::new(Gauge::with_capacity(DEFAULT_DOWNLOAD_BUFFER)),
        }
    }

    /// Returns the unique identifier of this node.
    #[inline]
    pub fn id(&self) -> NodeId {
        self.id
    }

    /// Set the upload bandwidth limit for this node.
    pub(crate) fn set_upload_bandwidth(&mut self, bandwidth: Bandwidth) {
        self.outbound_channel.set_bandwidth(bandwidth);
    }

    /// Set the maximum upload buffer size in bytes.
    pub(crate) fn set_upload_buffer(&mut self, buffer_size: u64) {
        self.outbound_buffer.set_maximum_capacity(buffer_size);
    }

    /// Set the download bandwidth limit for this node.
    pub(crate) fn set_download_bandwidth(&mut self, bandwidth: Bandwidth) {
        self.inbound_channel.set_bandwidth(bandwidth);
    }

    /// Set the maximum download buffer size in bytes.
    pub(crate) fn set_download_buffer(&mut self, buffer_size: u64) {
        self.inbound_buffer.set_maximum_capacity(buffer_size);
    }

    /// Returns how many bytes are currently occupying the upload buffer.
    pub fn upload_buffer_used(&self) -> u64 {
        self.outbound_buffer.used_capacity()
    }

    /// Returns the maximum capacity of the upload buffer in bytes.
    pub fn upload_buffer_max(&self) -> u64 {
        self.outbound_buffer.maximum_capacity()
    }

    /// Returns how many bytes are currently occupying the download buffer.
    pub fn download_buffer_used(&self) -> u64 {
        self.inbound_buffer.used_capacity()
    }

    /// Returns the maximum capacity of the download buffer in bytes.
    pub fn download_buffer_max(&self) -> u64 {
        self.inbound_buffer.maximum_capacity()
    }

    /// Returns a reference to this node's upload bandwidth setting.
    pub fn upload_bandwidth(&self) -> &Bandwidth {
        self.outbound_channel.bandwidth()
    }

    /// Returns a reference to this node's download bandwidth setting.
    pub fn download_bandwidth(&self) -> &Bandwidth {
        self.inbound_channel.bandwidth()
    }

    /// Returns an [`Upload`] handle for accounting bytes leaving this node.
    pub(crate) fn upload(&self) -> Upload {
        Upload::new(
            Arc::clone(&self.outbound_buffer),
            Arc::clone(&self.outbound_channel),
        )
    }

    /// Returns a [`Download`] handle for accounting bytes arriving at this node.
    pub(crate) fn download(&self) -> Download {
        Download::new(
            Arc::clone(&self.inbound_channel),
            Arc::clone(&self.inbound_buffer),
        )
    }
}
