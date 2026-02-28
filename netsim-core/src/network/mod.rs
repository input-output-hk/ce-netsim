mod linked_list;
mod packet;
mod round;
mod route;
mod transit;

use crate::{
    data::Data,
    link::{Link, LinkId},
    measure::{Bandwidth, Latency},
    node::{Node, NodeId},
};
use rand_chacha::ChaChaRng;
use rand_core::SeedableRng as _;
use std::{collections::HashMap, time::Duration};
use thiserror::Error;

pub use self::{
    linked_list::{CursorMut, LinkedList},
    packet::{Packet, PacketBuildError, PacketBuilder, PacketId, PacketIdGenerator},
    round::Round,
    route::Route,
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

    nodes: HashMap<NodeId, Node>,

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
/// | Upload buffer | Unlimited (`u64::MAX` bytes) |
/// | Download bandwidth | Unlimited ([`Bandwidth::MAX`]) |
/// | Download buffer | Unlimited (`u64::MAX` bytes) |
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
    node: Node,

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

/// Builder for reconfiguring an existing node's bandwidth and buffer settings.
///
/// Obtained via [`Network::configure_node`]. Mutations are applied eagerly
/// through each setter; [`apply`](NodeConfigBuilder::apply) is a no-op
/// finaliser kept for API symmetry with [`LinkBuilder`].
///
/// # Example
///
/// ```
/// use netsim_core::{network::Network, Bandwidth};
///
/// let mut network: Network<()> = Network::new();
/// let n1 = network.new_node().build();
///
/// network
///     .configure_node(n1)
///     .set_upload_bandwidth(Bandwidth::new(10_000_000))
///     .set_upload_buffer(1_024 * 1_024)
///     .apply();
/// ```
pub struct NodeConfigBuilder<'a, T> {
    id: NodeId,
    network: &'a mut Network<T>,
}

impl<T> NodeConfigBuilder<'_, T> {
    /// Set the upload bandwidth limit for this node.
    pub fn set_upload_bandwidth(self, bandwidth: Bandwidth) -> Self {
        if let Some(node) = self.network.nodes.get_mut(&self.id) {
            node.set_upload_bandwidth(bandwidth);
        }
        self
    }

    /// Set the download bandwidth limit for this node.
    pub fn set_download_bandwidth(self, bandwidth: Bandwidth) -> Self {
        if let Some(node) = self.network.nodes.get_mut(&self.id) {
            node.set_download_bandwidth(bandwidth);
        }
        self
    }

    /// Set the maximum upload buffer size in bytes.
    pub fn set_upload_buffer(self, buffer_size: u64) -> Self {
        if let Some(node) = self.network.nodes.get_mut(&self.id) {
            node.set_upload_buffer(buffer_size);
        }
        self
    }

    /// Set the maximum download buffer size in bytes.
    pub fn set_download_buffer(self, buffer_size: u64) -> Self {
        if let Some(node) = self.network.nodes.get_mut(&self.id) {
            node.set_download_buffer(buffer_size);
        }
        self
    }

    /// Finalise the configuration.
    ///
    /// Mutations have already been applied by each setter; this method
    /// exists for API symmetry with [`LinkBuilder::apply`].
    pub fn apply(self) {}
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

    /// Reconfigure an existing node's bandwidth and buffer settings.
    ///
    /// Returns a [`NodeConfigBuilder`] that allows updating upload/download
    /// bandwidth and buffer sizes. Call [`.apply()`](NodeConfigBuilder::apply)
    /// to finalise.
    ///
    /// If `id` does not exist in the network, setter calls are silently
    /// ignored (the builder is still returned for ergonomics).
    ///
    /// # Example
    ///
    /// ```
    /// # use netsim_core::{network::Network, Bandwidth};
    /// let mut network: Network<()> = Network::new();
    /// let n1 = network.new_node().build();
    ///
    /// network
    ///     .configure_node(n1)
    ///     .set_upload_bandwidth(Bandwidth::new(10_000_000))
    ///     .set_download_buffer(1_024 * 1_024)
    ///     .apply();
    /// ```
    pub fn configure_node(&mut self, id: NodeId) -> NodeConfigBuilder<'_, T> {
        NodeConfigBuilder { id, network: self }
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

        let Some(from) = self.nodes.get(&from) else {
            return Err(RouteError::SenderNotFound { sender: from });
        };
        let Some(to) = self.nodes.get(&to) else {
            return Err(RouteError::RecipientNotFound { recipient: to });
        };
        let Some(link) = self.links.get(&edge) else {
            return Err(RouteError::LinkNotFound { link: edge });
        };

        Ok(Route::new(from, link, to))
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

    /// Access a node by its [`NodeId`].
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    /// Iterate over all nodes in the network.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    /// Access a link by its [`LinkId`].
    pub fn link(&self, id: LinkId) -> Option<&Link> {
        self.links.get(&id)
    }

    /// Iterate over all links in the network.
    pub fn links(&self) -> impl Iterator<Item = (&LinkId, &Link)> {
        self.links.iter()
    }

    /// Number of packets currently in transit.
    pub fn transits(&self) -> impl Iterator<Item = &Transit<T>> {
        self.transit.iter()
    }

    /// Number of packets currently in transit.
    pub fn packets_in_transit(&self) -> usize {
        self.transit.len()
    }

    /// Current simulation round.
    pub fn round(&self) -> Round {
        self.round
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
    //
    // note: this function is O(n + l) (number of nodes + links). Would could be
    // faster by storing the maximum every time [self::configure_link] is used
    // (though we may not be affected by updates of the bandwidth on the nodes or link)
    pub fn minimum_step_duration(&self) -> Duration {
        let node_mins = self.nodes.values().flat_map(|node| {
            [
                node.upload_bandwidth().minimum_step_duration(),
                node.download_bandwidth().minimum_step_duration(),
            ]
        });
        let link_mins = self.links.values().flat_map(|link| {
            [
                link.forward_bandwidth().minimum_step_duration(),
                link.reverse_bandwidth().minimum_step_duration(),
            ]
        });
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
    pub fn advance_with<H>(&mut self, duration: Duration, handle: H)
    where
        H: FnMut(Packet<T>),
    {
        self.advance_with_report(duration, handle, |_| {});
    }

    /// Like [`advance_with`](Self::advance_with), but also reports corrupted
    /// (dropped) transits via a second callback.
    ///
    /// `on_deliver` is called for each packet that successfully completed
    /// transit.  `on_corrupt` is called for each transit that was removed
    /// because it became corrupted (e.g. the receiver's download buffer
    /// overflowed).  The `&Transit<T>` reference is valid only for the
    /// duration of the callback — the transit is freed immediately after.
    pub fn advance_with_report<H, D>(
        &mut self,
        duration: Duration,
        mut on_deliver: H,
        mut on_corrupt: D,
    ) where
        H: FnMut(Packet<T>),
        D: FnMut(Transit<T>),
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
                if let Some(transit) = cursor.remove_entry() {
                    match transit.complete() {
                        Ok(packet) => on_deliver(packet),
                        Err(transit) => on_corrupt(transit),
                    }
                }
            } else {
                cursor.move_next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::{Latency, PacketLoss};

    /// Helper: create a 2-node network with a zero-latency link and return (network, n1, n2).
    fn two_node_network() -> (Network<&'static str>, NodeId, NodeId) {
        let mut net = Network::new();
        let n1 = net.new_node().build();
        let n2 = net.new_node().build();
        net.configure_link(n1, n2)
            .set_latency(Latency::ZERO)
            .apply();
        (net, n1, n2)
    }

    fn send_msg(
        net: &mut Network<&'static str>,
        from: NodeId,
        to: NodeId,
        msg: &'static str,
    ) -> PacketId {
        let pkt = Packet::builder(net.packet_id_generator())
            .from(from)
            .to(to)
            .data(msg)
            .build()
            .unwrap();
        let id = pkt.id();
        net.send(pkt).unwrap();
        id
    }

    // ------------------------------------------------------------------
    // 1. Basic send + deliver
    // ------------------------------------------------------------------

    #[test]
    fn basic_send_and_deliver() {
        let (mut net, n1, n2) = two_node_network();
        let id = send_msg(&mut net, n1, n2, "hello");

        let mut delivered = Vec::new();
        // Default bandwidth is MAX, so a small packet arrives in one step.
        net.advance_with(Duration::from_millis(1), |pkt| delivered.push(pkt));

        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].id(), id);
        assert_eq!(delivered[0].from(), n1);
        assert_eq!(delivered[0].to(), n2);
    }

    #[test]
    fn delivered_payload_matches() {
        let (mut net, n1, n2) = two_node_network();
        send_msg(&mut net, n1, n2, "payload");

        let mut msg = None;
        net.advance_with(Duration::from_millis(1), |pkt| msg = Some(pkt.into_inner()));
        assert_eq!(msg, Some("payload"));
    }

    // ------------------------------------------------------------------
    // 2. Latency
    // ------------------------------------------------------------------

    #[test]
    fn packet_respects_latency() {
        let mut net: Network<&str> = Network::new();
        let n1 = net.new_node().build();
        let n2 = net.new_node().build();
        net.configure_link(n1, n2)
            .set_latency(Latency::new(Duration::from_millis(100)))
            .apply();

        send_msg(&mut net, n1, n2, "hi");

        // Advance 50ms — packet should NOT arrive yet.
        let mut arrived = false;
        net.advance_with(Duration::from_millis(50), |_| arrived = true);
        assert!(!arrived, "packet arrived before latency elapsed");

        // Advance another 60ms (total 110ms > 100ms latency) — should arrive.
        net.advance_with(Duration::from_millis(60), |_| arrived = true);
        assert!(arrived, "packet did not arrive after latency elapsed");
    }

    // ------------------------------------------------------------------
    // 3. Bandwidth saturation
    // ------------------------------------------------------------------

    #[test]
    fn bandwidth_limits_delivery_time() {
        let mut net: Network<[u8; 1000]> = Network::new();
        let n1 = net
            .new_node()
            .set_upload_bandwidth(Bandwidth::new(8_000_000)) // 8 Mbps = 1 byte/µs
            .build();
        let n2 = net.new_node().build();
        net.configure_link(n1, n2)
            .set_latency(Latency::ZERO)
            .set_bandwidth(Bandwidth::new(8_000_000))
            .apply();

        let pkt = Packet::builder(net.packet_id_generator())
            .from(n1)
            .to(n2)
            .data([0u8; 1000])
            .build()
            .unwrap();
        net.send(pkt).unwrap();

        // At 1 byte/µs, 1000 bytes needs 1000µs through upload and again
        // through the link. With 500µs steps, the first step can only push
        // 500 bytes through upload — packet should NOT be done yet.
        let mut count = 0u32;
        net.advance_with(Duration::from_micros(500), |_| count += 1);
        assert_eq!(count, 0, "packet arrived too early");

        // Keep advancing in 500µs steps until delivered.
        for _ in 0..1 {
            net.advance_with(Duration::from_micros(500), |_| count += 1);
            if count > 0 {
                break;
            }
        }
        assert_eq!(count, 1, "packet should eventually be delivered");
    }

    // ------------------------------------------------------------------
    // 4. Packet loss
    // ------------------------------------------------------------------

    #[test]
    fn full_packet_loss_drops_silently() {
        let mut net: Network<&str> = Network::new();
        let n1 = net.new_node().build();
        let n2 = net.new_node().build();
        net.configure_link(n1, n2)
            .set_packet_loss(PacketLoss::rate(1.0).unwrap())
            .apply();

        // send() returns Ok — the packet is silently dropped.
        let pkt = Packet::builder(net.packet_id_generator())
            .from(n1)
            .to(n2)
            .data("dropped")
            .build()
            .unwrap();
        assert!(net.send(pkt).is_ok());

        // Advance generously — nothing should arrive.
        let mut delivered = 0u32;
        for _ in 0..10 {
            net.advance_with(Duration::from_millis(10), |_| delivered += 1);
        }
        assert_eq!(delivered, 0);
    }

    #[test]
    fn no_packet_loss_delivers() {
        let mut net: Network<&str> = Network::new();
        let n1 = net.new_node().build();
        let n2 = net.new_node().build();
        net.configure_link(n1, n2)
            .set_latency(Latency::ZERO)
            .set_packet_loss(PacketLoss::None)
            .apply();

        send_msg(&mut net, n1, n2, "safe");

        let mut delivered = 0u32;
        net.advance_with(Duration::from_millis(1), |_| delivered += 1);
        assert_eq!(delivered, 1);
    }

    // ------------------------------------------------------------------
    // 5. Send errors
    // ------------------------------------------------------------------

    #[test]
    fn send_to_unknown_sender() {
        let mut net: Network<&str> = Network::new();
        let n1 = net.new_node().build();
        let n2 = net.new_node().build();
        net.configure_link(n1, n2).apply();

        let fake_sender = NodeId::new(999);
        let pkt = Packet::builder(net.packet_id_generator())
            .from(fake_sender)
            .to(n2)
            .data("x")
            .build()
            .unwrap();
        let err = net.send(pkt).unwrap_err();
        assert!(
            matches!(err, SendError::Route(RouteError::SenderNotFound { .. })),
            "expected SenderNotFound, got {err:?}"
        );
    }

    #[test]
    fn send_to_unknown_recipient() {
        let mut net: Network<&str> = Network::new();
        let n1 = net.new_node().build();
        let _n2 = net.new_node().build();

        let fake_recipient = NodeId::new(999);
        let pkt = Packet::builder(net.packet_id_generator())
            .from(n1)
            .to(fake_recipient)
            .data("x")
            .build()
            .unwrap();
        let err = net.send(pkt).unwrap_err();
        assert!(
            matches!(err, SendError::Route(RouteError::RecipientNotFound { .. })),
            "expected RecipientNotFound, got {err:?}"
        );
    }

    #[test]
    fn send_without_link() {
        let mut net: Network<&str> = Network::new();
        let n1 = net.new_node().build();
        let n2 = net.new_node().build();
        // No configure_link call.

        let pkt = Packet::builder(net.packet_id_generator())
            .from(n1)
            .to(n2)
            .data("x")
            .build()
            .unwrap();
        let err = net.send(pkt).unwrap_err();
        assert!(
            matches!(err, SendError::Route(RouteError::LinkNotFound { .. })),
            "expected LinkNotFound, got {err:?}"
        );
    }

    #[test]
    fn send_buffer_full() {
        let mut net: Network<[u8; 100]> = Network::new();
        let n1 = net
            .new_node()
            .set_upload_buffer(50) // smaller than the packet
            .build();
        let n2 = net.new_node().build();
        net.configure_link(n1, n2).apply();

        let pkt = Packet::builder(net.packet_id_generator())
            .from(n1)
            .to(n2)
            .data([0u8; 100]) // 100 bytes > 50 byte buffer
            .build()
            .unwrap();
        let err = net.send(pkt).unwrap_err();
        assert!(
            matches!(err, SendError::SenderBufferFull { .. }),
            "expected SenderBufferFull, got {err:?}"
        );
    }

    // ------------------------------------------------------------------
    // 6. Corruption detection (download buffer too small)
    // ------------------------------------------------------------------

    #[test]
    fn corruption_when_download_buffer_too_small() {
        let mut net: Network<[u8; 200]> = Network::new();
        let n1 = net.new_node().build();
        let n2 = net
            .new_node()
            .set_download_buffer(50) // smaller than the packet
            .build();
        net.configure_link(n1, n2).apply();

        let pkt = Packet::builder(net.packet_id_generator())
            .from(n1)
            .to(n2)
            .data([0u8; 200])
            .build()
            .unwrap();
        net.send(pkt).unwrap();

        // The download buffer (50 bytes) is smaller than the packet (200 bytes).
        // Flow control holds excess bytes in the link, so the transit stalls
        // rather than corrupting. The packet is never fully delivered.
        let mut delivered = 0u32;
        for _ in 0..100 {
            net.advance_with(Duration::from_millis(1), |_| delivered += 1);
        }
        assert_eq!(
            delivered, 0,
            "packet cannot complete when buffer is too small"
        );
    }

    // ------------------------------------------------------------------
    // 7. Multiple packets in flight
    // ------------------------------------------------------------------

    #[test]
    fn multiple_packets_in_flight() {
        let (mut net, n1, n2) = two_node_network();

        let id1 = send_msg(&mut net, n1, n2, "first");
        let id2 = send_msg(&mut net, n1, n2, "second");
        let id3 = send_msg(&mut net, n1, n2, "third");

        let mut ids = Vec::new();
        for _ in 0..10 {
            net.advance_with(Duration::from_millis(1), |pkt| ids.push(pkt.id()));
        }

        assert!(ids.contains(&id1), "first packet not delivered");
        assert!(ids.contains(&id2), "second packet not delivered");
        assert!(ids.contains(&id3), "third packet not delivered");
        assert_eq!(ids.len(), 3);
    }

    // ------------------------------------------------------------------
    // 8. Bidirectional traffic
    // ------------------------------------------------------------------

    #[test]
    fn bidirectional_traffic() {
        let (mut net, n1, n2) = two_node_network();

        let id_fwd = send_msg(&mut net, n1, n2, "forward");
        let id_rev = send_msg(&mut net, n2, n1, "reverse");

        let mut delivered = Vec::new();
        for _ in 0..10 {
            net.advance_with(Duration::from_millis(1), |pkt| {
                delivered.push((pkt.id(), pkt.from(), pkt.to()));
            });
        }

        assert_eq!(delivered.len(), 2);
        assert!(
            delivered
                .iter()
                .any(|(id, from, to)| *id == id_fwd && *from == n1 && *to == n2),
            "forward packet not delivered"
        );
        assert!(
            delivered
                .iter()
                .any(|(id, from, to)| *id == id_rev && *from == n2 && *to == n1),
            "reverse packet not delivered"
        );
    }

    // ------------------------------------------------------------------
    // 9. Network accessors
    // ------------------------------------------------------------------

    #[test]
    fn network_default() {
        let net = Network::<()>::default();
        assert_eq!(net.round(), Round::ZERO);
        assert_eq!(net.packets_in_transit(), 0);
    }

    #[test]
    fn node_accessor() {
        let mut net = Network::<()>::new();
        let n1 = net.new_node().build();
        let n2 = net.new_node().build();

        assert_eq!(net.node(n1).unwrap().id(), n1);
        assert_eq!(net.node(n2).unwrap().id(), n2);
        assert!(net.node(NodeId::new(999)).is_none());
    }

    #[test]
    fn nodes_iterator() {
        let mut net = Network::<()>::new();
        let n1 = net.new_node().build();
        let n2 = net.new_node().build();

        let ids: Vec<NodeId> = net.nodes().map(|n| n.id()).collect();
        assert!(ids.contains(&n1));
        assert!(ids.contains(&n2));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn link_accessor() {
        let mut net = Network::<()>::new();
        let n1 = net.new_node().build();
        let n2 = net.new_node().build();

        net.configure_link(n1, n2)
            .set_latency(Latency::new(Duration::from_millis(42)))
            .apply();

        let link_id = LinkId::new((n1, n2));
        let link = net.link(link_id).unwrap();
        assert_eq!(link.latency(), Latency::new(Duration::from_millis(42)));
    }

    #[test]
    fn links_iterator() {
        let mut net = Network::<()>::new();
        let n1 = net.new_node().build();
        let n2 = net.new_node().build();
        let n3 = net.new_node().build();

        net.configure_link(n1, n2).apply();
        net.configure_link(n2, n3).apply();

        let links: Vec<_> = net.links().collect();
        assert_eq!(links.len(), 2);
    }

    #[test]
    fn packets_in_transit() {
        let (mut net, n1, n2) = two_node_network();

        assert_eq!(net.packets_in_transit(), 0);

        send_msg(&mut net, n1, n2, "a");
        send_msg(&mut net, n1, n2, "b");

        assert_eq!(net.packets_in_transit(), 2);

        net.advance_with(Duration::from_millis(1), |_| {});

        assert_eq!(net.packets_in_transit(), 0);
    }

    #[test]
    fn round_advances() {
        let (mut net, _n1, _n2) = two_node_network();

        assert_eq!(net.round(), Round::ZERO);

        net.advance_with(Duration::from_millis(1), |_| {});
        assert_eq!(net.round(), Round::ZERO.next());

        net.advance_with(Duration::from_millis(1), |_| {});
        assert_eq!(net.round(), Round::ZERO.next().next());
    }

    #[test]
    fn set_seed_produces_deterministic_packet_loss() {
        // Run the same scenario with the same seed twice and verify identical outcomes.
        let run = |seed| {
            let mut net: Network<&str> = Network::new();
            net.set_seed(seed);
            let n1 = net.new_node().build();
            let n2 = net.new_node().build();
            net.configure_link(n1, n2)
                .set_latency(Latency::ZERO)
                .set_packet_loss(PacketLoss::rate(0.5).unwrap())
                .apply();

            let mut delivered = 0u32;
            for i in 0..100 {
                let msg: &str = if i % 2 == 0 { "even" } else { "odd" };
                send_msg(&mut net, n1, n2, msg);
                net.advance_with(Duration::from_millis(1), |_| delivered += 1);
            }
            delivered
        };

        assert_eq!(run(42), run(42));
    }

    #[test]
    fn set_download_bandwidth_via_builder() {
        let mut net = Network::<()>::new();
        let bw: Bandwidth = Bandwidth::new(1_000_000);
        let n1 = net.new_node().set_download_bandwidth(bw.clone()).build();

        assert_eq!(net.node(n1).unwrap().download_bandwidth(), &bw);
    }

    // ------------------------------------------------------------------
    // 11. configure_node
    // ------------------------------------------------------------------

    #[test]
    fn configure_node_upload_bandwidth() {
        let mut net = Network::<()>::new();
        let n1 = net.new_node().build();
        let bw = Bandwidth::new(5_000_000);

        net.configure_node(n1)
            .set_upload_bandwidth(bw.clone())
            .apply();

        assert_eq!(net.node(n1).unwrap().upload_bandwidth(), &bw);
    }

    #[test]
    fn configure_node_download_bandwidth() {
        let mut net = Network::<()>::new();
        let n1 = net.new_node().build();
        let bw = Bandwidth::new(2_000_000);

        net.configure_node(n1)
            .set_download_bandwidth(bw.clone())
            .apply();

        assert_eq!(net.node(n1).unwrap().download_bandwidth(), &bw);
    }

    #[test]
    fn configure_node_upload_buffer() {
        let mut net = Network::<()>::new();
        let n1 = net.new_node().build();

        net.configure_node(n1).set_upload_buffer(4096).apply();

        assert_eq!(net.node(n1).unwrap().upload_buffer_max(), 4096);
    }

    #[test]
    fn configure_node_download_buffer() {
        let mut net = Network::<()>::new();
        let n1 = net.new_node().build();

        net.configure_node(n1).set_download_buffer(8192).apply();

        assert_eq!(net.node(n1).unwrap().download_buffer_max(), 8192);
    }

    #[test]
    fn configure_node_all_properties() {
        let mut net = Network::<()>::new();
        let n1 = net.new_node().build();
        let ul_bw = Bandwidth::new(10_000_000);
        let dl_bw = Bandwidth::new(50_000_000);

        net.configure_node(n1)
            .set_upload_bandwidth(ul_bw.clone())
            .set_download_bandwidth(dl_bw.clone())
            .set_upload_buffer(1_000_000)
            .set_download_buffer(2_000_000)
            .apply();

        let node = net.node(n1).unwrap();
        assert_eq!(node.upload_bandwidth(), &ul_bw);
        assert_eq!(node.download_bandwidth(), &dl_bw);
        assert_eq!(node.upload_buffer_max(), 1_000_000);
        assert_eq!(node.download_buffer_max(), 2_000_000);
    }

    #[test]
    fn configure_node_unknown_id_is_no_op() {
        let mut net = Network::<()>::new();
        let _n1 = net.new_node().build();

        // Should not panic — setters silently skip if node not found.
        net.configure_node(NodeId::new(999))
            .set_upload_bandwidth(Bandwidth::new(1_000))
            .set_upload_buffer(100)
            .apply();
    }
}
