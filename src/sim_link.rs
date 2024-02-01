use crate::{defaults::DEFAULT_BYTES_PER_SEC, Msg, SimId};
use anyhow::{anyhow, Context, Result};
use std::time::Duration;
use tokio::{
    sync::mpsc,
    time::{self, Instant},
};

pub fn link(owner: SimId) -> (SimUpLink, SimDownLink) {
    let (sender, receiver) = mpsc::channel(1);

    let up = SimUpLink { owner, sender };
    let down = SimDownLink {
        receiver,
        bytes_per_sec: DEFAULT_BYTES_PER_SEC,
    };

    (up, down)
}

pub struct SimUpLink {
    owner: SimId,
    sender: mpsc::Sender<Msg>,
}

pub struct SimDownLink {
    receiver: mpsc::Receiver<Msg>,
    bytes_per_sec: u64,
}

impl SimUpLink {
    pub async fn send_to(&self, to: SimId, content: impl Into<Box<[u8]>>) -> Result<()> {
        let from = self.owner.clone();
        let msg = Msg::new(from.clone(), to.clone(), content.into());

        self.sender
            .send(msg)
            .await
            .with_context(|| anyhow!("Failed to send Msg from {from}, to {to}"))?;

        Ok(())
    }
}

impl SimDownLink {
    pub async fn recv(&mut self) -> Option<(SimId, Box<[u8]>)> {
        let msg = self.receiver.recv().await?;

        let delay = self.msg_delay(&msg);

        time::sleep_until(delay).await;

        Some((msg.from().clone(), msg.into_content()))
    }

    #[inline]
    fn msg_delay(&self, msg: &Msg) -> Instant {
        let content_size = msg.content().len() as u64;
        let lapse = Duration::from_secs(content_size / self.bytes_per_sec);

        msg.instant() + lapse
    }
}
