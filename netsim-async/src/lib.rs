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
pub(crate) use ce_netsim_util::Msg;
pub use ce_netsim_util::{HasBytesSize, SimConfiguration, SimId, SimSocketConfiguration};

pub struct SimSocket<T> {
    id: SimId,
    up: MuxSend<T>,
    down: SimDownLink<T>,
}

impl<T> SimSocket<T> {
    pub(crate) fn new(id: SimId, to_bus: MuxSend<T>, receiver: SimDownLink<T>) -> Self {
        Self {
            id,
            up: to_bus,
            down: receiver,
        }
    }

    pub fn id(&self) -> &SimId {
        &self.id
    }
}

impl<T> SimSocket<T>
where
    T: HasBytesSize,
{
    pub fn send_to(&self, to: SimId, msg: T) -> Result<()> {
        let msg = Msg::new(self.id().clone(), to, msg);
        self.up.send(msg)
    }

    pub async fn recv(&mut self) -> Option<(SimId, T)> {
        let msg = self.down.recv().await?;

        Some((msg.from().clone(), msg.into_content()))
    }
}
