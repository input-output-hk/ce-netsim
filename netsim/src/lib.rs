mod sim_context;
mod sim_link;

use std::sync::mpsc;

pub use crate::sim_context::SimContext;
use anyhow::Result;
pub use netsim_core::{
    Bandwidth, Edge, EdgePolicy, HasBytesSize, Latency, NodePolicy, PacketLoss, SimConfiguration,
    SimId,
};
use netsim_core::{BusSender, Msg};
use sim_link::SimDownLink;

pub struct SimSocket<T> {
    id: SimId,
    up: BusSender<T>,
    down: SimDownLink<T>,
}

impl<T> SimSocket<T> {
    pub(crate) fn new(id: SimId, to_bus: BusSender<T>, receiver: SimDownLink<T>) -> Self {
        Self {
            id,
            up: to_bus,
            down: receiver,
        }
    }

    pub fn id(&self) -> SimId {
        self.id
    }
}

pub enum TryRecv<T> {
    Some(T),
    NoMsg,
    Disconnected,
}

impl<T> SimSocket<T>
where
    T: HasBytesSize,
{
    pub fn send_to(&self, to: SimId, msg: T) -> Result<()> {
        let msg = Msg::new(self.id(), to, msg);
        self.up.send_msg(msg)
    }

    /// blocking call to receiving message on the channel
    ///
    /// returns None if the sending end has disconnected (no more senders)
    pub fn recv(&mut self) -> Option<(SimId, T)> {
        let msg = self.down.recv()?;

        Some((msg.from(), msg.into_content()))
    }

    /// blocking call to receiving message on the channel
    ///
    /// returns None if the sending end has disconnected (no more senders)
    pub fn try_recv(&mut self) -> TryRecv<(SimId, T)> {
        match self.down.try_recv() {
            Ok(msg) => TryRecv::Some((msg.from(), msg.into_content())),
            Err(mpsc::TryRecvError::Empty) => TryRecv::NoMsg,
            Err(mpsc::TryRecvError::Disconnected) => TryRecv::Disconnected,
        }
    }
}
