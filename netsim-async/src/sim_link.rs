use crate::{HasBytesSize, Msg};
use anyhow::{anyhow, Result};
use netsim_core::sim_context::Link;
use tokio::sync::mpsc;

pub fn link<T>() -> (SimUpLink<T>, SimDownLink<T>) {
    let (sender, receiver) = mpsc::unbounded_channel();

    let up = SimUpLink { sender };
    let down = SimDownLink { receiver };

    (up, down)
}

impl<T> Link for SimUpLink<T>
where
    T: HasBytesSize,
{
    type Msg = T;
    fn send(&self, msg: Msg<T>) -> Result<()> {
        self.sender.send(msg).map_err(|error| {
            anyhow!(
                "Failed to send Msg ({size} bytes) from {from}, to {to}",
                from = error.0.from(),
                to = error.0.to(),
                size = error.0.content().bytes_size(),
            )
        })
    }
}

pub struct SimUpLink<T> {
    sender: mpsc::UnboundedSender<Msg<T>>,
}

pub struct SimDownLink<T> {
    receiver: mpsc::UnboundedReceiver<Msg<T>>,
}

impl<T> SimDownLink<T>
where
    T: HasBytesSize,
{
    pub async fn recv(&mut self) -> Option<Msg<T>> {
        self.receiver.recv().await
    }
}

impl<T> Clone for SimUpLink<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}
