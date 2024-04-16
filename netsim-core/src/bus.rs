use crate::{sim_context::Link, Edge, EdgePolicy, Msg, NodePolicy, SimId};
use anyhow::{anyhow, Result};
use std::sync::mpsc;

pub enum BusMessage<UpLink: Link> {
    Message(Msg<UpLink::Msg>),
    NodeAdd(UpLink, mpsc::SyncSender<SimId>),
    NodePolicyDefault(NodePolicy),
    NodePolicySet(SimId, NodePolicy),
    NodePolicyReset(SimId),
    EdgePolicyDefault(EdgePolicy),
    EdgePolicySet(Edge, EdgePolicy),
    EdgePolicyReset(Edge),
    Shutdown,
    Disconnected,
}

pub struct BusSender<UpLink: Link> {
    sender: mpsc::Sender<BusMessage<UpLink>>,
}

pub(crate) struct BusReceiver<UpLink: Link> {
    receiver: mpsc::Receiver<BusMessage<UpLink>>,
}

pub(crate) fn open_bus<UpLink: Link>() -> (BusSender<UpLink>, BusReceiver<UpLink>) {
    let (sender, receiver) = mpsc::channel();
    (BusSender::new(sender), BusReceiver::new(receiver))
}

impl<UpLink: Link> BusSender<UpLink> {
    fn new(sender: mpsc::Sender<BusMessage<UpLink>>) -> Self {
        Self { sender }
    }

    fn send(&self, msg: BusMessage<UpLink>) -> Result<()> {
        self.sender
            .send(msg)
            .map_err(|error| anyhow!("failed to send message: {error}"))
    }

    pub fn send_msg(&self, msg: Msg<UpLink::Msg>) -> Result<()> {
        self.send(BusMessage::Message(msg))
    }

    pub fn send_node_add(&self, link: UpLink, reply: mpsc::SyncSender<SimId>) -> Result<()> {
        self.send(BusMessage::NodeAdd(link, reply))
    }

    pub fn send_node_policy_default(&self, policy: NodePolicy) -> Result<()> {
        self.send(BusMessage::NodePolicyDefault(policy))
    }

    pub fn send_node_policy_set(&self, id: SimId, policy: NodePolicy) -> Result<()> {
        self.send(BusMessage::NodePolicySet(id, policy))
    }

    pub fn send_node_policy_reset(&self, id: SimId) -> Result<()> {
        self.send(BusMessage::NodePolicyReset(id))
    }

    pub fn send_edge_policy_default(&self, policy: EdgePolicy) -> Result<()> {
        self.send(BusMessage::EdgePolicyDefault(policy))
    }

    pub fn send_edge_policy_set(&self, id: Edge, policy: EdgePolicy) -> Result<()> {
        self.send(BusMessage::EdgePolicySet(id, policy))
    }

    pub fn send_edge_policy_reset(&self, id: Edge) -> Result<()> {
        self.send(BusMessage::EdgePolicyReset(id))
    }

    pub(crate) fn send_shutdown(&self) -> Result<()> {
        self.send(BusMessage::Shutdown)
    }
}

impl<UpLink: Link> BusReceiver<UpLink> {
    fn new(receiver: mpsc::Receiver<BusMessage<UpLink>>) -> Self {
        Self { receiver }
    }

    pub(crate) fn try_receive(&mut self) -> Option<BusMessage<UpLink>> {
        match self.receiver.try_recv() {
            Ok(bus_msg) => Some(bus_msg),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(BusMessage::Disconnected),
        }
    }
}

impl<UpLink: Link> Clone for BusSender<UpLink> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}
