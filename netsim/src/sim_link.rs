use anyhow::{anyhow, Result};
use ce_netsim_util::{sim_context::Link, HasBytesSize, Msg};
use std::sync::mpsc;

pub fn link<T>(bytes_per_sec: u64) -> (SimUpLink<T>, SimDownLink<T>) {
    let (sender, receiver) = mpsc::channel();

    let up = SimUpLink {
        sender,
        bytes_per_sec,
    };
    let down = SimDownLink { receiver };

    (up, down)
}

pub struct SimUpLink<T> {
    sender: mpsc::Sender<Msg<T>>,
    bytes_per_sec: u64,
}

pub struct SimDownLink<T> {
    receiver: mpsc::Receiver<Msg<T>>,
}

impl<T> Link for SimUpLink<T>
where
    T: HasBytesSize,
{
    type Msg = T;

    fn download_speed(&self) -> u64 {
        self.bytes_per_sec
    }

    fn upload_speed(&self) -> u64 {
        // TODO
        u64::MAX
    }
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
            bytes_per_sec: self.bytes_per_sec,
        }
    }
}