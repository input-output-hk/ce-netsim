use crate::{HasBytesSize, Msg, SimId};
use anyhow::{anyhow, Result};
use tokio::sync::mpsc;

pub fn link<T>(bytes_per_sec: u64) -> (SimUpLink<T>, SimDownLink<T>) {
    let (sender, receiver) = mpsc::unbounded_channel();

    let up = SimUpLink { sender };
    let down = SimDownLink {
        receiver,
        bytes_per_sec,
    };

    (up, down)
}

pub struct SimUpLink<T> {
    sender: mpsc::UnboundedSender<Msg<T>>,
}

pub struct SimDownLink<T> {
    receiver: mpsc::UnboundedReceiver<Msg<T>>,
    bytes_per_sec: u64,
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

    /*
    #[inline]
    fn msg_delay(&self, msg: &Msg<T>) -> std::time::SystemTime {
        let content_size = msg.content().bytes_size();
        let lapse = Duration::from_secs(content_size / self.bytes_per_sec);

        msg.sent() + lapse
    }
    */
}

impl<T> Clone for SimUpLink<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}
