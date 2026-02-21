mod id;

use crate::{
    defaults::{DEFAULT_DOWNLOAD_BUFFER, DEFAULT_UPLOAD_BUFFER},
    measure::{Bandwidth, CongestionChannel, Download, Gauge, Upload},
    network::Packet,
};
use std::{collections::LinkedList, sync::Arc};

pub use self::id::NodeId;

pub struct Node<T> {
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

    inbounds: LinkedList<Packet<T>>,
}

impl<T> Node<T> {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            outbound_buffer: Arc::new(Gauge::with_capacity(DEFAULT_UPLOAD_BUFFER)),
            outbound_channel: Arc::new(CongestionChannel::new(Bandwidth::default())),
            inbound_channel: Arc::new(CongestionChannel::new(Bandwidth::default())),
            inbound_buffer: Arc::new(Gauge::with_capacity(DEFAULT_DOWNLOAD_BUFFER)),
            inbounds: LinkedList::new(),
        }
    }

    #[inline]
    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn set_upload_bandwidth(&mut self, bandwidth: Bandwidth) {
        self.outbound_channel.set_bandwidth(bandwidth);
    }

    pub fn set_upload_buffer(&mut self, buffer_size: u64) {
        self.outbound_buffer.set_maximum_capacity(buffer_size);
    }

    pub fn set_download_bandwidth(&mut self, bandwidth: Bandwidth) {
        self.inbound_channel.set_bandwidth(bandwidth);
    }

    pub fn set_download_buffer(&mut self, buffer_size: u64) {
        self.inbound_buffer.set_maximum_capacity(buffer_size);
    }

    pub fn upload_buffer_used(&self) -> u64 {
        self.outbound_buffer.used_capacity()
    }

    pub fn upload_buffer_max(&self) -> u64 {
        self.outbound_buffer.maximum_capacity()
    }

    pub fn download_buffer_used(&self) -> u64 {
        self.inbound_buffer.used_capacity()
    }

    pub fn download_buffer_max(&self) -> u64 {
        self.inbound_buffer.maximum_capacity()
    }

    pub fn upload_bandwidth(&self) -> Bandwidth {
        self.outbound_channel.bandwidth()
    }

    pub fn download_bandwidth(&self) -> Bandwidth {
        self.inbound_channel.bandwidth()
    }

    pub fn upload(&self) -> Upload {
        Upload::new(
            Arc::clone(&self.outbound_buffer),
            Arc::clone(&self.outbound_channel),
        )
    }

    pub fn download(&self) -> Download {
        Download::new(
            Arc::clone(&self.inbound_channel),
            Arc::clone(&self.inbound_buffer),
        )
    }

    pub fn push_pending(&mut self, packet: Packet<T>) {
        self.inbounds.push_back(packet);
    }

    pub fn pop_pending(&mut self) -> Option<Packet<T>> {
        self.inbounds.pop_front()
    }
}
