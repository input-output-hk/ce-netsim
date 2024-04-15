use crate::SimId;
use std::time::Instant;

/// Trait for message content that will be sent via
/// [`send_to`] and [`recv`] function of the [`SimSocket`].
///
/// Messages sent aren't serialised, but we need to know the
/// bytes length representation of the messages so that we
/// can accurately treat the message in the multiplexer.
///
/// [`SimSocket`]: crate::SimSocket
/// [`send_to`]: crate::SimSocket::send_to
/// [`recv`]: crate::SimSocket::recv
pub trait HasBytesSize: Send + 'static {
    /// return the content of the message in bytes
    fn bytes_size(&self) -> u64;
}

pub struct Msg<T> {
    from: SimId,
    to: SimId,
    time: Instant,
    content: T,
}

impl<T> Msg<T> {
    pub fn new(from: SimId, to: SimId, content: T) -> Self {
        Self {
            from,
            to,
            time: Instant::now(),
            content,
        }
    }

    pub fn from(&self) -> SimId {
        self.from
    }

    pub fn to(&self) -> SimId {
        self.to
    }

    pub fn time(&self) -> Instant {
        self.time
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
impl HasBytesSize for Box<[u8]> {
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
