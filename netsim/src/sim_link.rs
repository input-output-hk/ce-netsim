use anyhow::{anyhow, Result};
use ce_netsim_core::{sim_context::Link, HasBytesSize, Msg};
use std::sync::mpsc;

pub fn link<T>() -> (SimUpLink<T>, SimDownLink<T>) {
    let (sender, receiver) = mpsc::channel();

    let up = SimUpLink { sender };
    let down = SimDownLink { receiver };

    (up, down)
}

pub struct SimUpLink<T> {
    sender: mpsc::Sender<Msg<T>>,
}

pub struct SimDownLink<T> {
    receiver: mpsc::Receiver<Msg<T>>,
}

impl<T> Link for SimUpLink<T>
where
    T: HasBytesSize,
{
    type Msg = T;
}

impl<T> SimUpLink<T>
where
    T: HasBytesSize,
{
    /// non blocking call to send a message
    ///
    /// Error only occurs if the receiving ends has hanged up
    pub(crate) fn send(&self, msg: Msg<T>) -> Result<()> {
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

impl<T> SimDownLink<T>
where
    T: HasBytesSize,
{
    /// blocking call to receiving message on the channel
    ///
    /// returns `None` if the sending end has disconnected (no more senders)
    pub fn recv(&mut self) -> Option<Msg<T>> {
        self.receiver.recv().ok()
    }

    pub fn try_recv(&mut self) -> std::result::Result<Msg<T>, mpsc::TryRecvError> {
        self.receiver.try_recv()
    }
}

impl<T> Clone for SimUpLink<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}
