mod linked_list;
mod packet;
mod round;
mod route;
mod transit;

use crate::{
    data::Data,
    link::{Link, LinkId},
    measure::Bandwidth,
    node::{Node, NodeId},
};
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

#[derive(Debug, Error)]
pub enum RouteError {
    #[error("Sender ({sender}) Not Found")]
    SenderNotFound { sender: NodeId },
    #[error("Recipient ({recipient}) Not Found")]
    RecipientNotFound { recipient: NodeId },
    #[error("Link ({link:?}) Not Found")]
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
        self.node.set_upload_bandwidth(bandwidth);
        self
    }

    pub fn build(self) -> NodeId {
        let Self { node, network } = self;

        let id = node.id();

        debug_assert_ne!(id, NodeId::ZERO, "The NodeId `0` shouldn't be given.");

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

    pub fn new_node(&mut self) -> NodeBuilder<'_, T> {
        self.id = self.id.next();
        NodeBuilder {
            node: Node::new(self.id),
            network: self,
        }
    }

    /// get the route between two nodes.
    ///
    /// This allows to monitor the transit of packets between two specific
    /// nodes.
    ///
    pub fn route(&self, from: NodeId, to: NodeId) -> Result<Route, RouteError> {
        let edge = LinkId::new((from, to));

        let Some(from) = self.nodes.get(&from) else {
            return Err(RouteError::SenderNotFound { sender: from });
        };
        let Some(to) = self.nodes.get(&to) else {
            return Err(RouteError::RecipientNotFound { recipient: to });
        };
        let link = self
            .links
            .get(&edge)
            .map(|l| l.duplicate())
            .unwrap_or_default();

        let route = RouteBuilder::new()
            .upload(from)
            .link(&link)
            .download(to)
            .build()
            .expect("Failed to build a Route");

        Ok(route)
    }

    /// send a packet through the network
    ///
    /// # Error
    ///
    /// This will return an error if the _route_ cannot be built
    /// (receiver or sender not found).
    ///
    /// If the message cannot be sent because the sender's buffer
    /// is full.
    ///
    pub fn send(&mut self, packet: Packet<T>) -> Result<(), SendError> {
        let from = packet.from();
        let to = packet.to();

        let route = self.route(from, to)?;

        let transit = route.transit(packet)?;

        self.transit.push(transit);

        Ok(())
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
            let remove;
            {
                let Some(transit) = cursor.as_mut() else {
                    break;
                };

                transit.advance(self.round, duration);

                remove = transit.completed() || transit.corrupted();
            }

            if remove {
                if let Some(transit) = cursor.remove_entry() {
                    if let Ok(packet) = transit.complete() {
                        handle(packet)
                    }
                }
            } else {
                cursor.move_next();
            }
        }
    }
}
