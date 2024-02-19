mod shutdown;
mod sim_context;
mod sim_link;

pub use self::sim_context::SimContext;
pub(crate) use self::{
    shutdown::{ShutdownController, ShutdownReceiver},
    sim_context::MuxSend,
    sim_link::{link, SimDownLink, SimUpLink},
};
use anyhow::Result;
pub(crate) use netsim_core::Msg;
pub use netsim_core::{
    Bandwidth, Edge, EdgePolicy, HasBytesSize, Latency, NodePolicy, PacketLoss, SimConfiguration,
    SimId,
};

pub struct SimSocket<T> {
    id: SimId,
    up: MuxSend<T>,
    down: SimDownLink<T>,
}

pub struct SimSocketReadHalf<T> {
    id: SimId,
    down: SimDownLink<T>,
}

pub struct SimSocketWriteHalf<T> {
    id: SimId,
    up: MuxSend<T>,
}

impl<T> SimSocket<T> {
    pub(crate) fn new(id: SimId, to_bus: MuxSend<T>, receiver: SimDownLink<T>) -> Self {
        Self {
            id,
            up: to_bus,
            down: receiver,
        }
    }

    pub fn id(&self) -> SimId {
        self.id
    }

    pub fn into_split(self) -> (SimSocketReadHalf<T>, SimSocketWriteHalf<T>) {
        let read = SimSocketReadHalf {
            id: self.id,
            down: self.down,
        };
        let write = SimSocketWriteHalf {
            id: self.id,
            up: self.up,
        };

        (read, write)
    }
}

impl<T> SimSocket<T>
where
    T: HasBytesSize,
{
    pub fn send_to(&self, to: SimId, msg: T) -> Result<()> {
        let msg = Msg::new(self.id(), to, msg);
        self.up.send(msg)
    }

    pub async fn recv(&mut self) -> Option<(SimId, T)> {
        let msg = self.down.recv().await?;

        Some((msg.from(), msg.into_content()))
    }
}

impl<T> SimSocketWriteHalf<T> {
    pub fn id(&self) -> SimId {
        self.id
    }
}

impl<T> SimSocketWriteHalf<T>
where
    T: HasBytesSize,
{
    pub fn send_to(&self, to: SimId, msg: T) -> Result<()> {
        let msg = Msg::new(self.id, to, msg);
        self.up.send(msg)
    }
}

impl<T> SimSocketReadHalf<T> {
    pub fn id(&self) -> SimId {
        self.id
    }
}

impl<T> SimSocketReadHalf<T>
where
    T: HasBytesSize,
{
    pub async fn recv(&mut self) -> Option<(SimId, T)> {
        let msg = self.down.recv().await?;

        Some((msg.from(), msg.into_content()))
    }
}
