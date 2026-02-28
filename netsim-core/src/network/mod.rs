mod linked_list;
mod packet;
mod round;
mod route;
mod transit;

use crate::{
    data::Data,
    link::{Link, LinkDirection, LinkId},
    measure::{Bandwidth, Latency},
    node::{Node, NodeId},
};
use rand_chacha::ChaChaRng;
use rand_core::SeedableRng as _;
use std::{collections::HashMap, time::Duration};
use thiserror::Error;

pub use self::{
    linked_list::{CursorMut, LinkedList},
    packet::{Packet, PacketBuilder, PacketId, PacketIdGenerator},
    round::Round,
    route::{Route, RouteBuilder},
    transit::Transit,
};

/// This is the entry point for all activities with [`netsim_core`].
///
/// The [`Network`] is responsible for maintaining each [`Node`] accountable
/// of their network activities: Upload and Download congestivity as well as
/// buffers for sending and receiving data. It is also responsible for
/// keeping the [`Link`] accountable for the [`Latency`], the per-direction
/// [`Bandwidth`] channels, and the packet-loss policy.
///
/// See the [crate] documentation for more information on how to
/// create a network, update the network and, send and receive messages.
///
/// [`netsim_core`]: crate
/// [`Latency`]: crate::measure::Latency
/// [`Bandwidth`]: crate::measure::Bandwidth
pub struct Network<T> {
    packet_id_generator: PacketIdGenerator,

    nodes: HashMap<NodeId, Node<T>>,

    links: HashMap<LinkId, Link>,

    round: Round,

    /// current active routes (i.e. with messages)
    transit: LinkedList<Transit<T>>,

    /// the last assigned ID
    ///
    /// ID 0 is an error and shouldn't be given
    id: NodeId,

    /// Centralised RNG for all packet-loss decisions on every link.
    ///
    /// A single source guarantees that the simulation is reproducible when
    /// seeded via [`Network::set_seed`].
    rng: ChaChaRng,
}

/// Builder for configuring a new node before registering it with the network.
///
/// Obtained via [`Network::new_node`]. Configure per-node bandwidth and buffer
/// limits with the setter methods, then call [`build`](NodeBuilder::build) to
/// register the node and obtain its [`NodeId`].
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
/// ## Example
///
/// ```
/// use netsim_core::{network::Network, Bandwidth};
///
/// let mut network: Network<()> = Network::new();
///
/// // Default node — unlimited bandwidth, 64 MiB buffers.
/// let n1 = network.new_node().build();
///
/// // Constrained node — 10 Mbps upload, 100 MB upload buffer.
/// let n2 = network
///     .new_node()
///     .set_upload_bandwidth("10mbps".parse().unwrap())
///     .set_upload_buffer(100 * 1_024 * 1_024)
///     .build();
/// ```
pub struct NodeBuilder<'a, T> {
    node: Node<T>,

    network: &'a mut Network<T>,
}

/// Builder for configuring a link between two nodes.
///
/// Obtained via [`Network::configure_link`]. Call [`LinkBuilder::apply`] to
/// commit the configuration.
pub struct LinkBuilder<'a, T> {
    a: NodeId,
    b: NodeId,
    latency: Latency,
    bandwidth: Bandwidth,
    packet_loss: crate::measure::PacketLoss,
    network: &'a mut Network<T>,
}

impl<T> LinkBuilder<'_, T> {
    /// Set the one-way latency of this link.
    pub fn set_latency(mut self, latency: Latency) -> Self {
        self.latency = latency;
        self
    }

    /// Set the bandwidth for this link.
    ///
    /// The same bandwidth applies to both directions independently — each
    /// direction has its own congestion channel, so traffic in one direction
    /// does not consume capacity in the other.
    pub fn set_bandwidth(mut self, bandwidth: Bandwidth) -> Self {
        self.bandwidth = bandwidth;
        self
    }

    /// Set the probabilistic packet loss rate for this link.
    pub fn set_packet_loss(mut self, packet_loss: crate::measure::PacketLoss) -> Self {
        self.packet_loss = packet_loss;
        self
    }

    /// Commit the link configuration to the network.
    pub fn apply(self) {
        let Self {
            a,
            b,
            latency,
            bandwidth,
            packet_loss,
            network,
        } = self;
        let id = LinkId::new((a, b));
        network
            .links
            .insert(id, Link::new(latency, bandwidth, packet_loss));
    }
}

/// Error returned when a route between two nodes cannot be established.
///
/// Nodes are not automatically connected when created — a link must be
/// explicitly configured via [`Network::configure_link`] before packets
/// can be sent between them.
#[derive(Debug, Error)]
pub enum RouteError {
    /// The sending node ID was not found in the network.
    #[error("Sender ({sender}) Not Found")]
    SenderNotFound { sender: NodeId },
    /// The receiving node ID was not found in the network.
    #[error("Recipient ({recipient}) Not Found")]
    RecipientNotFound { recipient: NodeId },
    /// No link has been configured between the two nodes.
    ///
    /// Use [`Network::configure_link`] to set up a direct connection
    /// between them before sending packets.
    #[error(
        "Link ({link:?}) Not Found: nodes are not directly connected, call configure_link first"
    )]
    LinkNotFound { link: LinkId },
}

/// Error returned when [`Network::send`] fails.
#[derive(Debug, Error)]
pub enum SendError {
    /// The route between the two nodes could not be established.
    #[error("{0}")]
    Route(#[from] RouteError),
    /// The sending node's upload buffer is full; the packet was dropped.
    #[error("Sender's ({sender}) buffer is full.")]
    SenderBufferFull {
        sender: NodeId,
        buffer_max_size: u64,
        buffer_current_size: u64,
        packet_size: u64,
    },
}

impl<T> Default for Network<T>
where
    T: Data,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> NodeBuilder<'_, T> {
    /// Set the maximum size of the node's upload buffer in bytes.
    ///
    /// Packets queued for sending are held in this buffer until bandwidth
    /// allows them to be transmitted. If the buffer is full, [`Network::send`]
    /// returns [`SendError::SenderBufferFull`].
    pub fn set_upload_buffer(mut self, buffer_size: u64) -> Self {
        self.node.set_upload_buffer(buffer_size);
        self
    }

    /// Set the upload bandwidth limit for this node.
    ///
    /// Controls how many bytes per second this node can transmit. Defaults
    /// to [`Bandwidth::MAX`] (unlimited).
    pub fn set_upload_bandwidth(mut self, bandwidth: Bandwidth) -> Self {
        self.node.set_upload_bandwidth(bandwidth);
        self
    }

    /// Set the maximum size of the node's download buffer in bytes.
    ///
    /// Incoming bytes are held in this buffer until the application reads
    /// them. If the buffer is full, arriving bytes are silently dropped and
    /// the transit is marked corrupted.
    pub fn set_download_buffer(mut self, buffer_size: u64) -> Self {
        self.node.set_download_buffer(buffer_size);
        self
    }

    /// Set the download bandwidth limit for this node.
    ///
    /// Controls how many bytes per second this node can receive. Defaults
    /// to [`Bandwidth::MAX`] (unlimited).
    pub fn set_download_bandwidth(mut self, bandwidth: Bandwidth) -> Self {
        self.node.set_download_bandwidth(bandwidth);
        self
    }

    /// Finalise the node configuration and register it with the network.
    ///
    /// Returns the [`NodeId`] assigned to this node.
    pub fn build(self) -> NodeId {
        let Self { node, network } = self;

        let id = node.id();

        network.nodes.insert(id, node);

        id
    }
}

impl<T> Network<T>
where
    T: Data,
{
    /// Create a new, empty simulated network.
    ///
    /// The network has no nodes or links. Add nodes with
    /// [`new_node`](Network::new_node) and connect them with
    /// [`configure_link`](Network::configure_link).
    ///
    /// # Example
    ///
    /// ```
    /// use netsim_core::network::Network;
    ///
    /// let mut network: Network<()> = Network::new();
    /// let n1 = network.new_node().build();
    /// let n2 = network.new_node().build();
    /// network.configure_link(n1, n2).apply();
    /// ```
    pub fn new() -> Self {
        Self {
            packet_id_generator: PacketIdGenerator::new(),
            nodes: HashMap::new(),
            links: HashMap::new(),
            round: Round::ZERO,
            transit: LinkedList::new(),
            id: NodeId::ZERO,
            rng: ChaChaRng::seed_from_u64(0),
        }
    }

    /// Re-seed the network's random-number generator.
    ///
    /// All packet-loss decisions for every link are drawn from a single,
    /// centralised [`ChaChaRng`]. Calling `set_seed` before running a
    /// simulation produces a fully deterministic, reproducible sequence of
    /// drops — useful for regression tests and benchmarks.
    ///
    /// The default seed is `0`.
    ///
    /// # Example
    ///
    /// ```
    /// use netsim_core::{network::Network, measure::PacketLoss};
    ///
    /// let mut network: Network<()> = Network::new();
    /// network.set_seed(42); // deterministic packet-loss sequence
    /// ```
    pub fn set_seed(&mut self, seed: u64) {
        self.rng = ChaChaRng::seed_from_u64(seed);
    }

    /// Returns the shared [`PacketIdGenerator`] for this network.
    ///
    /// Pass this to [`Packet::builder`] when constructing packets manually.
    /// The generator is shared between the network and all sockets; every call
    /// to [`PacketIdGenerator::generate`] produces a unique [`PacketId`].
    pub fn packet_id_generator(&self) -> &PacketIdGenerator {
        &self.packet_id_generator
    }

    /// Create a new node and return a builder to configure it.
    ///
    /// Node IDs are assigned sequentially starting at `1`. [`NodeId::ZERO`] is
    /// reserved as a sentinel and is never returned by this method.
    pub fn new_node(&mut self) -> NodeBuilder<'_, T> {
        self.id = self.id.next();
        NodeBuilder {
            node: Node::new(self.id),
            network: self,
        }
    }

    /// Configure the link between two nodes.
    ///
    /// Returns a [`LinkBuilder`] that allows setting latency, bandwidth, and
    /// packet loss. The same bandwidth applies to both directions independently:
    /// each direction has its own congestion channel so traffic in one direction
    /// does not consume capacity in the other (full-duplex).
    /// Call [`.apply()`](LinkBuilder::apply) to commit.
    ///
    /// If a link already exists between these nodes it will be replaced.
    /// In-flight packets on the old link will complete with the old settings.
    ///
    /// # Example
    ///
    /// ```
    /// # use netsim_core::{network::Network, Bandwidth, Latency};
    /// # use std::time::Duration;
    /// let mut network: Network<()> = Network::new();
    /// let n1 = network.new_node().build();
    /// let n2 = network.new_node().build();
    ///
    /// network
    ///     .configure_link(n1, n2)
    ///     .set_latency(Latency::new(Duration::from_millis(10)))
    ///     .set_bandwidth("100mbps".parse().unwrap())
    ///     .apply();
    /// ```
    pub fn configure_link(&mut self, a: NodeId, b: NodeId) -> LinkBuilder<'_, T> {
        LinkBuilder {
            a,
            b,
            latency: Latency::default(),
            bandwidth: Bandwidth::default(),
            packet_loss: crate::measure::PacketLoss::default(),
            network: self,
        }
    }

    /// Returns the route between two nodes, if one exists.
    ///
    /// A route requires both nodes to be present in the network **and** a
    /// link to have been configured between them via [`Network::configure_link`].
    /// Nodes that have never had a link configured are not directly reachable
    /// from each other, and this method will return [`RouteError::LinkNotFound`]
    /// in that case.
    ///
    /// # Errors
    ///
    /// - [`RouteError::SenderNotFound`] — `from` node does not exist in the network.
    /// - [`RouteError::RecipientNotFound`] — `to` node does not exist in the network.
    /// - [`RouteError::LinkNotFound`] — no link has been configured between the two
    ///   nodes. Call [`Network::configure_link`] to establish a direct connection.
    pub fn route(&self, from: NodeId, to: NodeId) -> Result<Route, RouteError> {
        let edge = LinkId::new((from, to));
        let direction = if from < to {
            LinkDirection::Forward
        } else {
            LinkDirection::Reverse
        };

        let Some(from) = self.nodes.get(&from) else {
            return Err(RouteError::SenderNotFound { sender: from });
        };
        let Some(to) = self.nodes.get(&to) else {
            return Err(RouteError::RecipientNotFound { recipient: to });
        };
        let Some(link) = self.links.get(&edge) else {
            return Err(RouteError::LinkNotFound { link: edge });
        };

        let route = RouteBuilder::new()
            .upload(from)
            .link(link, direction)
            .download(to)
            .build()
            .expect("Failed to build a Route");

        Ok(route)
    }

    /// Send a packet through the network.
    ///
    /// # Errors
    ///
    /// - [`SendError::Route`] wrapping [`RouteError::SenderNotFound`] or
    ///   [`RouteError::RecipientNotFound`] if either node does not exist.
    /// - [`SendError::Route`] wrapping [`RouteError::LinkNotFound`] if no link
    ///   has been configured between the sender and recipient. Call
    ///   [`Network::configure_link`] to establish a direct connection first.
    /// - [`SendError::SenderBufferFull`] if the sender's upload buffer is at
    ///   capacity.
    ///
    pub fn send(&mut self, packet: Packet<T>) -> Result<(), SendError> {
        let from = packet.from();
        let to = packet.to();

        // Check packet loss before routing (avoids building the full route for dropped packets)
        let edge = LinkId::new((from, to));
        if let Some(link) = self.links.get(&edge)
            && link.should_drop_packet(&mut self.rng)
        {
            return Ok(());
        }

        let route = self.route(from, to)?;

        let transit = route.transit(packet)?;

        self.transit.push(transit);

        Ok(())
    }

    /// Returns a point-in-time snapshot of the network state.
    ///
    /// Includes per-node buffer and bandwidth stats and per-link latency,
    /// bandwidth, packet loss, and bytes in transit.
    pub fn stats(&self) -> crate::stats::NetworkStats {
        use crate::stats::{LinkStats, NetworkStats, NodeStats};

        let nodes = self
            .nodes
            .values()
            .map(|node| NodeStats {
                id: node.id(),
                upload_buffer_used: node.upload_buffer_used(),
                upload_buffer_max: node.upload_buffer_max(),
                download_buffer_used: node.download_buffer_used(),
                download_buffer_max: node.download_buffer_max(),
                upload_bandwidth: node.upload_bandwidth().clone(),
                download_bandwidth: node.download_bandwidth().clone(),
            })
            .collect();

        let links = self
            .links
            .iter()
            .map(|(id, link)| LinkStats {
                id: *id,
                latency: link.latency(),
                bandwidth: link.bandwidth().clone(),
                packet_loss: link.packet_loss(),
                bytes_in_transit: 0,
            })
            .collect();

        NetworkStats { nodes, links }
    }

    /// Returns the minimum step [`Duration`] needed so that every configured
    /// bandwidth channel can transfer at least 1 byte per call to
    /// [`advance_with`](Network::advance_with).
    ///
    /// Computed as the maximum of [`Bandwidth::minimum_step_duration`] across
    /// every node's upload/download channel and every link's bandwidth channel.
    /// Returns [`Duration::ZERO`] for an empty network or when all configured
    /// bandwidths are zero.
    ///
    /// If you pass a `duration` smaller than this value to `advance_with`, the
    /// most constrained channel will yield 0 bytes per step and packets on that
    /// route will stall silently. Check this after configuring your network:
    ///
    /// ```
    /// # use netsim_core::{network::Network, Bandwidth};
    /// let mut network: Network<()> = Network::new();
    /// let a = network.new_node().build();
    /// let b = network.new_node().build();
    /// network
    ///     .configure_link(a, b)
    ///     .set_bandwidth(Bandwidth::new(10_000)) // 10 Kbps
    ///     .apply();
    ///
    /// // ceil(8_000_000 / 10_000) = 800 µs minimum step
    /// assert_eq!(
    ///     network.minimum_step_duration(),
    ///     std::time::Duration::from_micros(800),
    /// );
    /// ```
    ///
    /// [`Bandwidth::minimum_step_duration`]: crate::measure::Bandwidth::minimum_step_duration
    pub fn minimum_step_duration(&self) -> Duration {
        let node_mins = self.nodes.values().flat_map(|node| {
            [
                node.upload_bandwidth().minimum_step_duration(),
                node.download_bandwidth().minimum_step_duration(),
            ]
        });
        let link_mins = self
            .links
            .values()
            .map(|link| link.bandwidth().minimum_step_duration());
        node_mins.chain(link_mins).max().unwrap_or(Duration::ZERO)
    }

    /// Advance the network state and deliver packets that have completed transit.
    ///
    /// `duration` is the simulated time elapsed since the last call. The
    /// provided `handle` closure is called once for each packet that has
    /// fully traversed the network during this step.
    ///
    /// ## Bandwidth floor
    ///
    /// If `duration` is shorter than [`Network::minimum_step_duration`], the
    /// most constrained bandwidth channel will yield 0 bytes per step and
    /// packets on that route will never be delivered. See
    /// [`Bandwidth`](crate::measure::Bandwidth) for the minimum bandwidth table
    /// by step size.
    pub fn advance_with<H>(&mut self, duration: Duration, mut handle: H)
    where
        H: FnMut(Packet<T>),
    {
        self.round = self.round.next();

        let mut cursor = self.transit.cursor_mut();
        loop {
            let Some(transit) = cursor.as_mut() else {
                break;
            };

            transit.advance(self.round, duration);

            let remove = transit.completed() || transit.corrupted();

            if remove {
                if let Some(transit) = cursor.remove_entry()
                    && let Ok(packet) = transit.complete()
                {
                    handle(packet)
                }
            } else {
                cursor.move_next();
            }
        }
    }
}
