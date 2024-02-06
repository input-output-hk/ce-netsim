use crate::{HasBytesSize, Msg};
use anyhow::{anyhow, Result};
use ce_netsim_util::sim_context::Link;
use tokio::sync::mpsc;

pub fn link<T>(bytes_per_sec: u64) -> (SimUpLink<T>, SimDownLink<T>) {
    let (sender, receiver) = mpsc::unbounded_channel();

    let up = SimUpLink {
        sender,
        bytes_per_sec,
    };
    let down = SimDownLink { receiver };

    (up, down)
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

pub struct SimUpLink<T> {
    sender: mpsc::UnboundedSender<Msg<T>>,
    bytes_per_sec: u64,
}

pub struct SimDownLink<T> {
    receiver: mpsc::UnboundedReceiver<Msg<T>>,
}

impl<T> SimUpLink<T>
where
    T: HasBytesSize,
{
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

    #[inline]
    pub(crate) fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }
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
            bytes_per_sec: self.bytes_per_sec,
        }
    }
}
