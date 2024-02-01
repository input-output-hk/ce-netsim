pub(crate) mod defaults;
mod msg;
mod sim_context;
mod sim_id;
mod sim_link;

pub(crate) use self::{
    msg::Msg,
    sim_link::{link, SimDownLink, SimUpLink},
};
pub use self::{sim_context::SimContext, sim_id::SimId};
use anyhow::Result;

pub struct SimSocket {
    up: SimUpLink,
}

impl SimSocket {
    pub async fn send_to(&self, to: SimId, msg: impl Into<Box<[u8]>>) -> Result<()> {
        self.up.send_to(to, msg).await
    }
}
