mod sim_context;
mod sim_link;

pub use self::sim_context::SimContext;
pub(crate) use self::sim_link::{link, SimDownLink, SimUpLink};
use anyhow::Result;
use netsim_core::BusSender;
pub(crate) use netsim_core::Msg;
pub use netsim_core::{
    geo, Bandwidth, LinkId, EdgePolicy, HasBytesSize, Latency, NodePolicy, PacketLoss,
    SimConfiguration, NodeId,
};

pub struct SimSocket<T>
where
    T: HasBytesSize,
{
    reader: SimSocketReadHalf<T>,
    writer: SimSocketWriteHalf<T>,
}

pub struct SimSocketReadHalf<T> {
    id: NodeId,
    down: SimDownLink<T>,
}

pub struct SimSocketWriteHalf<T>
where
    T: HasBytesSize,
{
    id: NodeId,
    up: BusSender<SimUpLink<T>>,
}

impl<T> SimSocket<T>
where
    T: HasBytesSize,
{
    pub(crate) fn new(
        id: NodeId,
        to_bus: BusSender<SimUpLink<T>>,
        receiver: SimDownLink<T>,
    ) -> Self {
        let reader = SimSocketReadHalf { id, down: receiver };
        let writer = SimSocketWriteHalf { id, up: to_bus };

        Self { reader, writer }
    }

    pub fn id(&self) -> NodeId {
        self.reader.id()
    }

    pub fn into_split(self) -> (SimSocketReadHalf<T>, SimSocketWriteHalf<T>) {
        let Self { reader, writer } = self;
        (reader, writer)
    }
}

impl<T> SimSocket<T>
where
    T: HasBytesSize,
{
    pub fn send_to(&self, to: NodeId, msg: T) -> Result<()> {
        self.writer.send_to(to, msg)
    }

    pub async fn recv(&mut self) -> Option<(NodeId, T)> {
        self.reader.recv().await
    }
}

impl<T> SimSocketWriteHalf<T>
where
    T: HasBytesSize,
{
    pub fn id(&self) -> NodeId {
        self.id
    }
}

impl<T> SimSocketWriteHalf<T>
where
    T: HasBytesSize,
{
    pub fn send_to(&self, to: NodeId, msg: T) -> Result<()> {
        let msg = Msg::new(self.id, to, msg);
        self.up.send_msg(msg)
    }
}

impl<T> SimSocketReadHalf<T> {
    pub fn id(&self) -> NodeId {
        self.id
    }
}

impl<T> SimSocketReadHalf<T>
where
    T: HasBytesSize,
{
    pub async fn recv(&mut self) -> Option<(NodeId, T)> {
        let msg = self.down.recv().await?;

        Some((msg.from(), msg.into_content()))
    }
}
