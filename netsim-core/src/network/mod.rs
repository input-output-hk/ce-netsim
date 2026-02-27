mod linked_list;
mod packet;
mod round;
mod route;
mod transit;

use crate::{
    data::Data,
    link::{Link, LinkId},
    measure::{Bandwidth, CongestionChannel, Latency},
    node::{Node, NodeId},
};
use std::{collections::HashMap, sync::Arc, time::Duration};
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
/// buffers for sending and receiving data. It is also reponsible for
/// keeping the [`Link`] accountable for the [`Latency`] and the
/// [`Bandwidth`].
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

    // consideration:
    //   based on previous implementation we kept having one `Link` for
    //   sharing both direction. I.e. if there are a lot of packets going
    //   in one direction, it will affect the bandwidth of the packet going
    //   the opposite direction. Maybe we would like to consider having
    //   a different approach and have the `LinkId` identify also the
    //   direction of the link.
    //
    links: HashMap<LinkId, Link>,

    round: Round,

    /// current active routes (i.e. with messages)
    transit: LinkedList<Transit<T>>,

    /// the last assigned ID
    ///
    /// ID 0 is an error and shouldn't be given
    id: NodeId,
}

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

    /// Set the shared bandwidth capacity of this link.
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
        let channel = Arc::new(CongestionChannel::new(bandwidth));
        network
            .links
            .insert(id, Link::new_with_loss(latency, channel, packet_loss));
    }
}

/// Error returned when a route between two nodes cannot be established.
///
/// Nodes are not automatically connected when created — a link must be
/// explicitly configured via [`Network::configure_link`] before packets
/// can be sent between them.
#[derive(Debug, Error)]
pub enum RouteError {
    #[error("Sender ({sender}) Not Found")]
    SenderNotFound { sender: NodeId },
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

#[derive(Debug, Error)]
pub enum SendError {
    #[error("{0}")]
    Route(#[from] RouteError),
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
    pub fn set_upload_buffer(mut self, buffer_size: u64) -> Self {
        self.node.set_upload_buffer(buffer_size);
        self
    }

    pub fn set_upload_bandwidth(mut self, bandwidth: Bandwidth) -> Self {
        self.node.set_upload_bandwidth(bandwidth);
        self
    }

    pub fn set_download_buffer(mut self, buffer_size: u64) -> Self {
        self.node.set_download_buffer(buffer_size);
        self
    }

    pub fn set_download_bandwidth(mut self, bandwidth: Bandwidth) -> Self {
        self.node.set_download_bandwidth(bandwidth);
        self
    }

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
    pub fn new() -> Self {
        Self {
            packet_id_generator: PacketIdGenerator::new(),
            nodes: HashMap::new(),
            links: HashMap::new(),
            round: Round::ZERO,
            transit: LinkedList::new(),
            id: NodeId::ZERO,
        }
    }

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
    /// Returns a [`LinkBuilder`] that allows setting latency and bandwidth.
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
            .link(link)
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
        if let Some(link) = self.links.get_mut(&edge)
            && link.should_drop_packet()
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
                bytes_in_transit: link.bytes_in_transit(),
            })
            .collect();

        NetworkStats { nodes, links }
    }

    /// advsance the network state and handle network packets
    /// ready to be received.
    ///
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
