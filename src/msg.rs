use crate::SimId;
use tokio::time::Instant;

pub trait HasBytesSize: Send + 'static {
    fn bytes_size(&self) -> u64;
}

pub struct Msg<T> {
    from: SimId,
    to: SimId,
    sent: Instant,
    content: T,
}

impl<T> Msg<T> {
    pub fn new(from: SimId, to: SimId, content: T) -> Self {
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

    pub fn content(&self) -> &T {
        &self.content
    }

    pub fn into_content(self) -> T {
        self.content
    }
}

impl HasBytesSize for [u8] {
    fn bytes_size(&self) -> u64 {
        self.len() as u64
    }
}
impl HasBytesSize for &'static str {
    fn bytes_size(&self) -> u64 {
        self.as_bytes().bytes_size()
    }
}
impl HasBytesSize for str {
    fn bytes_size(&self) -> u64 {
        self.as_bytes().bytes_size()
    }
}
impl HasBytesSize for Vec<u8> {
    fn bytes_size(&self) -> u64 {
        (self.capacity() + std::mem::size_of_val(self)) as u64
    }
}
impl HasBytesSize for String {
    fn bytes_size(&self) -> u64 {
        (self.capacity() + std::mem::size_of_val(self)) as u64
    }
}
