pub(crate) mod defaults;
mod msg;
mod shutdown;
mod sim_context;
mod sim_id;
mod sim_link;
mod time_queue;

pub use self::{
    msg::HasBytesSize,
    sim_context::{SimConfiguration, SimContext},
    sim_id::SimId,
};
pub(crate) use self::{
    msg::Msg,
    shutdown::{ShutdownController, ShutdownReceiver},
    sim_link::{link, SimDownLink, SimUpLink},
    time_queue::TimeQueue,
};
use anyhow::Result;
use sim_context::MuxSend;

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
