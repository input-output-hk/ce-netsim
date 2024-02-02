pub(crate) mod defaults;
mod msg;
mod sim_context;
mod sim_id;
mod sim_link;

pub use self::{msg::HasBytesSize, sim_context::SimContext, sim_id::SimId};
pub(crate) use self::{
    msg::Msg,
    sim_link::{link, SimDownLink, SimUpLink},
};
use anyhow::Result;

pub struct SimSocket<T> {
    up: SimUpLink<T>,
    down: SimDownLink<T>,
}

impl<T> SimSocket<T> {
    pub(crate) fn new(to_bus: SimUpLink<T>, receiver: SimDownLink<T>) -> Self {
        Self {
            up: to_bus,
            down: receiver,
        }
    }

    pub fn id(&self) -> &SimId {
        self.down.id()
    }
}

impl<T> SimSocket<T>
where
    T: HasBytesSize,
{
    pub fn send_to(&self, to: SimId, msg: T) -> Result<()> {
        self.up.send_to(to, msg)
    }

    pub async fn recv(&mut self) -> Option<(SimId, T)> {
        let msg = self.down.recv().await?;

        Some((msg.from().clone(), msg.into_content()))
    }
}
