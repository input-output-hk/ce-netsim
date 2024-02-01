use std::time::Duration;

use crate::SimId;
use tokio::time::Instant;

pub struct Msg {
    from: SimId,
    to: SimId,
    sent: Instant,
    content: Box<[u8]>,
}

impl Msg {
    pub fn new(from: SimId, to: SimId, content: Box<[u8]>) -> Self {
        Self {
            from,
            to,
            sent: Instant::now(),
            content,
        }
    }

    pub fn from(&self) -> &SimId {
        &self.from
    }

    pub fn to(&self) -> &SimId {
        &self.to
    }

    pub fn instant(&self) -> Instant {
        self.sent
    }

    pub fn elapsed(&self) -> Duration {
        self.sent.elapsed()
    }

    pub fn content(&self) -> &[u8] {
        self.content.as_ref()
    }

    pub fn into_content(self) -> Box<[u8]> {
        self.content
    }
}
