use crate::multiplexer::command::{CommandSender, NewNodeCommand};
use anyhow::Result;
use netsim_core::{
    data::Data,
    defaults::{
        DEFAULT_DOWNLOAD_BANDWIDTH, DEFAULT_DOWNLOAD_BUFFER, DEFAULT_UPLOAD_BANDWIDTH,
        DEFAULT_UPLOAD_BUFFER,
    },
    network::{PacketId, PacketIdGenerator},
    Bandwidth, NodeId, Packet,
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc::{sync_channel, Receiver},
    Arc,
};
use thiserror::Error;

pub struct SimSocket<T> {
    id: NodeId,
    packet_id_generator: PacketIdGenerator,
    download: Receiver<Packet<T>>,
    command: CommandSender<T>,
    dropped: Arc<AtomicU64>,
}

pub struct SimSocketBuilder<'a, T> {
    commands: CommandSender<T>,

    packet_id_generator: PacketIdGenerator,

    // initial upload bandwidth
    upload_bandwidth: Bandwidth,
    upload_buffer: u64,

    download_bandwidth: Bandwidth,
    download_buffer: u64,

    _marker: std::marker::PhantomData<&'a ()>,
}

#[derive(Debug, Error)]
pub enum SendError<T> {
    #[error("Failed to send packet: disconnected.")]
    Disconnected(Packet<T>),
    /// This error should only happen if the queue is overloading with
    /// backlog data.
    ///
    #[error("Failed to send packet: queue is full.")]
    Full(Packet<T>),
}

#[derive(Debug, Error)]
pub enum SendToError<T> {
    #[error("Failed to build message")]
    FailedToBuildMessage(#[source] anyhow::Error),
    #[error("Failed to send packet: disconnected")]
    Disconnected(Packet<T>),
    /// This error should only happen if the queue is overloading with
    /// backlog data.
    ///
    #[error("Failed to send packet: queue is full.")]
    Full(Packet<T>),
}

#[derive(Debug, Error)]
pub enum TryRecvError {
    #[error("Failed to receive packet: disconnected.")]
    Disconnected,
    #[error("No message to receive yet.")]
    Empty,
}

#[derive(Debug)]
pub struct RecvError;

impl<T> SimSocketBuilder<'_, T> {
    pub(crate) fn new(commands: CommandSender<T>, packet_id_generator: PacketIdGenerator) -> Self {
        Self {
            packet_id_generator,
            commands,
            upload_bandwidth: DEFAULT_UPLOAD_BANDWIDTH,
            upload_buffer: DEFAULT_UPLOAD_BUFFER,
            download_bandwidth: DEFAULT_DOWNLOAD_BANDWIDTH,
            download_buffer: DEFAULT_DOWNLOAD_BUFFER,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn set_upload_bandwidth(mut self, bandwidth: Bandwidth) -> Self {
        self.upload_bandwidth = bandwidth;
        self
    }

    pub fn set_upload_buffer(mut self, buffer: u64) -> Self {
        self.upload_buffer = buffer;
        self
    }

    pub fn set_download_bandwidth(mut self, bandwidth: Bandwidth) -> Self {
        self.download_bandwidth = bandwidth;
        self
    }

    pub fn set_download_buffer(mut self, buffer: u64) -> Self {
        self.download_buffer = buffer;
        self
    }

    pub fn build(self) -> anyhow::Result<SimSocket<T>> {
        let Self {
            mut commands,
            packet_id_generator,
            upload_bandwidth,
            upload_buffer,
            download_bandwidth,
            download_buffer,
            _marker,
        } = self;
        let (sender, receiver) = sync_channel(10 * 1_024);
        let dropped = Arc::new(AtomicU64::new(0));

        let new_node = NewNodeCommand {
            sender,
            dropped: Arc::clone(&dropped),
            upload_bandwidth,
            upload_buffer,
            download_bandwidth,
            download_buffer,
        };

        let id = commands.send_new_node(new_node)?;

        Ok(SimSocket::new(id, commands, receiver, packet_id_generator, dropped))
    }
}

impl<T> SimSocket<T> {
    pub(crate) fn new(
        id: NodeId,
        command: CommandSender<T>,
        download: Receiver<Packet<T>>,
        packet_id_generator: PacketIdGenerator,
        dropped: Arc<AtomicU64>,
    ) -> Self {
        Self {
            id,
            command,
            download,
            packet_id_generator,
            dropped,
        }
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn packet_id_generator(&self) -> &PacketIdGenerator {
        &self.packet_id_generator
    }

    /// Returns the number of packets dropped for this node due to a full sender buffer.
    ///
    /// Packets are dropped silently (UDP semantics). Use this counter to observe loss.
    pub fn packets_dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn send_packet(&mut self, packet: Packet<T>) -> Result<(), SendError<T>> {
        Ok(self.command.send_packet(packet)?)
    }

    pub fn recv_packet(&mut self) -> Result<Packet<T>, RecvError> {
        Ok(self.download.recv()?)
    }

    pub fn try_recv_packet(&mut self) -> Result<Packet<T>, TryRecvError> {
        Ok(self.download.try_recv()?)
    }
}

impl<T> SimSocket<T>
where
    T: Data,
{
    pub fn send_to(&mut self, to: NodeId, data: T) -> Result<PacketId, SendToError<T>> {
        let packet = Packet::builder(self.packet_id_generator())
            .from(self.id)
            .to(to)
            .data(data)
            .build()
            .map_err(SendToError::FailedToBuildMessage)?;

        let id = packet.id();
        self.send_packet(packet).map(|()| id)?;

        Ok(id)
    }
}

impl From<std::sync::mpsc::RecvError> for RecvError {
    fn from(_value: std::sync::mpsc::RecvError) -> Self {
        RecvError
    }
}

impl<T> From<std::sync::mpsc::TrySendError<Packet<T>>> for SendError<T> {
    fn from(value: std::sync::mpsc::TrySendError<Packet<T>>) -> Self {
        match value {
            std::sync::mpsc::TrySendError::Disconnected(packet) => Self::Disconnected(packet),
            std::sync::mpsc::TrySendError::Full(packet) => Self::Full(packet),
        }
    }
}

impl<T> From<SendError<T>> for SendToError<T> {
    fn from(value: SendError<T>) -> Self {
        match value {
            SendError::Disconnected(packet) => Self::Disconnected(packet),
            SendError::Full(packet) => Self::Full(packet),
        }
    }
}

impl From<std::sync::mpsc::TryRecvError> for TryRecvError {
    fn from(value: std::sync::mpsc::TryRecvError) -> Self {
        match value {
            std::sync::mpsc::TryRecvError::Disconnected => Self::Disconnected,
            std::sync::mpsc::TryRecvError::Empty => Self::Empty,
        }
    }
}
