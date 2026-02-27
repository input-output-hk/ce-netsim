use crate::multiplexer::command::{CommandSender, NewNodeCommand};
use anyhow::Result;
use netsim_core::{
    Bandwidth, NodeId, Packet,
    data::Data,
    defaults::{
        DEFAULT_DOWNLOAD_BANDWIDTH, DEFAULT_DOWNLOAD_BUFFER, DEFAULT_UPLOAD_BANDWIDTH,
        DEFAULT_UPLOAD_BUFFER,
    },
    network::{PacketId, PacketIdGenerator},
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc::{Receiver, sync_channel},
};
use thiserror::Error;

/// A socket handle for a node in a [`SimContext`] simulation.
///
/// `SimSocket` is the primary send/receive interface. Each socket represents
/// one simulated node. Obtain a socket by calling [`SimContext::open`](crate::SimContext::open) and
/// building it:
///
/// ```rust,no_run
/// # use netsim::{SimContext, Data};
/// # struct MyMsg;
/// # impl Data for MyMsg { fn bytes_size(&self) -> u64 { 0 } }
/// # fn example() -> anyhow::Result<()> {
/// let mut sim = SimContext::<MyMsg>::new()?;
/// let mut socket = sim.open().build()?;
/// println!("node id: {}", socket.id());
/// # Ok(()) }
/// ```
///
/// ## Sending
///
/// The simplest way to send is [`send_to`](SimSocket::send_to), which builds
/// the packet for you:
///
/// ```rust,no_run
/// # use netsim::{SimContext, Latency, Data};
/// # use std::time::Duration;
/// # #[derive(Debug)] struct MyMsg(u64);
/// # impl Data for MyMsg { fn bytes_size(&self) -> u64 { 8 } }
/// # fn example() -> anyhow::Result<()> {
/// # let mut sim = SimContext::<MyMsg>::new()?;
/// # let mut a = sim.open().build()?;
/// # let mut b = sim.open().build()?;
/// # sim.configure_link(a.id(), b.id()).apply()?;
/// let packet_id = a.send_to(b.id(), MyMsg(42))?;
/// # Ok(()) }
/// ```
///
/// ## Receiving
///
/// | Method | Behaviour |
/// |--------|-----------|
/// | [`recv_packet`](SimSocket::recv_packet) | Blocks until a packet arrives or the sim shuts down |
/// | [`try_recv_packet`](SimSocket::try_recv_packet) | Returns immediately; [`TryRecvError::Empty`] if nothing waiting |
///
/// [`SimContext`]: crate::SimContext
pub struct SimSocket<T> {
    id: NodeId,
    packet_id_generator: PacketIdGenerator,
    download: Receiver<Packet<T>>,
    command: CommandSender<T>,
    dropped: Arc<AtomicU64>,
}

/// Builder for configuring a [`SimSocket`] before registering it with the
/// simulation.
///
/// Obtained via [`SimContext::open`]. Configure per-node bandwidth and buffer
/// limits, then call [`build`](SimSocketBuilder::build) to register the node
/// and receive the [`SimSocket`].
///
/// ## Defaults
///
/// | Setting | Default |
/// |---------|---------|
/// | Upload bandwidth | Unlimited ([`Bandwidth::MAX`]) |
/// | Upload buffer | 64 MiB |
/// | Download bandwidth | Unlimited ([`Bandwidth::MAX`]) |
/// | Download buffer | 64 MiB |
///
/// [`SimContext::open`]: crate::SimContext::open
/// [`Bandwidth::MAX`]: netsim_core::Bandwidth
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

/// Error returned by [`SimSocket::send_packet`].
#[derive(Debug, Error)]
pub enum SendError<T> {
    /// The multiplexer has shut down; the packet could not be delivered.
    #[error("Failed to send packet: disconnected.")]
    Disconnected(Packet<T>),
    /// The internal command queue is full. This typically indicates that the
    /// multiplexer thread cannot keep up with the rate of sends.
    #[error("Failed to send packet: queue is full.")]
    Full(Packet<T>),
}

/// Error returned by [`SimSocket::send_to`].
#[derive(Debug, Error)]
pub enum SendToError<T> {
    /// The [`Packet`] could not be built (e.g. missing `from`/`to` fields).
    #[error("Failed to build message")]
    FailedToBuildMessage(#[source] anyhow::Error),
    /// The multiplexer has shut down; the packet could not be delivered.
    #[error("Failed to send packet: disconnected")]
    Disconnected(Packet<T>),
    /// The internal command queue is full.
    #[error("Failed to send packet: queue is full.")]
    Full(Packet<T>),
}

/// Error returned by [`SimSocket::try_recv_packet`].
#[derive(Debug, Error)]
pub enum TryRecvError {
    /// The simulation has shut down and no more packets will arrive.
    #[error("Failed to receive packet: disconnected.")]
    Disconnected,
    /// No packet is currently waiting in the receive buffer.
    #[error("No message to receive yet.")]
    Empty,
}

/// Error returned by [`SimSocket::recv_packet`].
///
/// The only way to get this error is if the simulation shuts down before a
/// packet arrives (i.e. the multiplexer thread exited).
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

    /// Set the upload bandwidth limit for this node in bytes per second.
    ///
    /// Controls how fast this node can transmit. Defaults to unlimited.
    pub fn set_upload_bandwidth(mut self, bandwidth: Bandwidth) -> Self {
        self.upload_bandwidth = bandwidth;
        self
    }

    /// Set the maximum upload buffer size in bytes.
    ///
    /// Packets accumulate here until bandwidth allows them to enter the link.
    /// If the buffer fills, [`SimSocket::send_to`] / [`SimSocket::send_packet`]
    /// will fail with a dropped-packet error.
    pub fn set_upload_buffer(mut self, buffer: u64) -> Self {
        self.upload_buffer = buffer;
        self
    }

    /// Set the download bandwidth limit for this node in bytes per second.
    ///
    /// Controls how fast this node can receive. Defaults to unlimited.
    pub fn set_download_bandwidth(mut self, bandwidth: Bandwidth) -> Self {
        self.download_bandwidth = bandwidth;
        self
    }

    /// Set the maximum download buffer size in bytes.
    ///
    /// Incoming bytes wait here until the application reads them. If the buffer
    /// overflows, arriving bytes are dropped (corrupted transit).
    pub fn set_download_buffer(mut self, buffer: u64) -> Self {
        self.download_buffer = buffer;
        self
    }

    /// Register this node with the simulation and return the [`SimSocket`].
    ///
    /// Communicates with the multiplexer synchronously and blocks briefly
    /// until the node is registered. Returns the socket ready for use.
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

        Ok(SimSocket::new(
            id,
            commands,
            receiver,
            packet_id_generator,
            dropped,
        ))
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

    /// Returns the [`NodeId`] of this socket's simulated node.
    ///
    /// Use this ID when addressing packets to this socket from another node,
    /// or when configuring links via [`SimContext::configure_link`].
    ///
    /// [`SimContext::configure_link`]: crate::SimContext::configure_link
    pub fn id(&self) -> NodeId {
        self.id
    }

    /// Returns the shared [`PacketIdGenerator`] for this socket.
    ///
    /// Use this when manually constructing a [`Packet`] with
    /// [`Packet::builder`]. Each call to [`PacketIdGenerator::generate`]
    /// produces a globally unique [`PacketId`] for the lifetime of the
    /// simulation.
    pub fn packet_id_generator(&self) -> &PacketIdGenerator {
        &self.packet_id_generator
    }

    /// Returns the cumulative number of packets dropped at this node's
    /// upload buffer due to congestion.
    ///
    /// Packets are dropped silently (UDP semantics). Use this counter to
    /// observe loss in your simulation without halting on errors.
    ///
    /// The counter is reset to zero when the socket is created and only
    /// increments — it never wraps.
    pub fn packets_dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Send a pre-built [`Packet`] into the simulation.
    ///
    /// The packet must have its `from` field set to this socket's [`NodeId`].
    /// For most use cases [`send_to`](SimSocket::send_to) is more ergonomic.
    ///
    /// # Errors
    ///
    /// - [`SendError::Disconnected`] — the multiplexer has shut down.
    /// - [`SendError::Full`] — the internal command queue is full.
    pub fn send_packet(&mut self, packet: Packet<T>) -> Result<(), SendError<T>> {
        Ok(self.command.send_packet(packet)?)
    }

    /// Block until the next packet arrives at this node.
    ///
    /// This call parks the calling thread until either a packet is delivered
    /// by the multiplexer or the simulation shuts down. For non-blocking
    /// receive, use [`try_recv_packet`](SimSocket::try_recv_packet).
    ///
    /// # Errors
    ///
    /// Returns [`RecvError`] only if the multiplexer thread has exited and no
    /// more packets will ever arrive. Typically this means
    /// [`SimContext::shutdown`] was called before this socket could receive.
    ///
    /// [`SimContext::shutdown`]: crate::SimContext::shutdown
    pub fn recv_packet(&mut self) -> Result<Packet<T>, RecvError> {
        Ok(self.download.recv()?)
    }

    /// Attempt to receive a packet without blocking.
    ///
    /// Returns immediately with either the next waiting packet, or an error
    /// indicating why nothing was available.
    ///
    /// # Errors
    ///
    /// - [`TryRecvError::Empty`] — no packet is currently queued; try again later.
    /// - [`TryRecvError::Disconnected`] — the simulation has shut down.
    pub fn try_recv_packet(&mut self) -> Result<Packet<T>, TryRecvError> {
        Ok(self.download.try_recv()?)
    }
}

impl<T> SimSocket<T>
where
    T: Data,
{
    /// Build and send a packet to another node in one step.
    ///
    /// This is the most ergonomic way to send. It constructs a [`Packet`]
    /// addressed from this socket's node to `to`, then submits it to the
    /// multiplexer. Returns the [`PacketId`] so you can match it on the
    /// receiving end.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use netsim::{SimContext, Latency, Data};
    /// # use std::time::Duration;
    /// # #[derive(Debug)] struct Msg(String);
    /// # impl Data for Msg { fn bytes_size(&self) -> u64 { self.0.len() as u64 } }
    /// # fn example() -> anyhow::Result<()> {
    /// # let mut sim = SimContext::<Msg>::new()?;
    /// # let mut client = sim.open().build()?;
    /// # let mut server = sim.open().build()?;
    /// # sim.configure_link(client.id(), server.id()).apply()?;
    /// let id = client.send_to(server.id(), Msg("hello".to_string()))?;
    ///
    /// let packet = server.recv_packet().unwrap();
    /// assert_eq!(packet.id(), id);
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// - [`SendToError::FailedToBuildMessage`] — an internal packet-building
    ///   error (should not occur under normal use).
    /// - [`SendToError::Disconnected`] — the multiplexer has shut down.
    /// - [`SendToError::Full`] — the internal command queue is full.
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
