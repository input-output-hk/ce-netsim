/// Trait for message content that will be sent through the [`Network`].
///
/// Messages sent aren't serialised, but we need to know the
/// bytes length representation of the messages so that we
/// can accurately treat the message within the multiplexer.
///
/// [`Network`]: crate::network::Network
pub trait Data: Send + 'static {
    /// return the content of the message in bytes
    ///
    /// # case for `0` bytes data
    ///
    /// There is a caveat of using a message with a bytes size of `0`.
    /// This mean that the message has no data to circulate. However
    /// it should still transit through the network. The only impact
    /// should be the [`Latency`] of the route.
    ///
    /// [`Latency`]: crate::measure::Latency
    ///
    fn bytes_size(&self) -> u64;
}

impl Data for () {
    fn bytes_size(&self) -> u64 {
        0
    }
}
impl<const S: usize> Data for [u8; S] {
    fn bytes_size(&self) -> u64 {
        S as u64
    }
}
impl Data for [u8] {
    fn bytes_size(&self) -> u64 {
        self.len() as u64
    }
}
impl Data for Box<[u8]> {
    fn bytes_size(&self) -> u64 {
        self.len() as u64
    }
}
impl Data for &'static str {
    fn bytes_size(&self) -> u64 {
        self.as_bytes().bytes_size()
    }
}
impl Data for u8 {
    fn bytes_size(&self) -> u64 {
        1
    }
}
impl Data for str {
    fn bytes_size(&self) -> u64 {
        self.as_bytes().bytes_size()
    }
}
impl Data for Vec<u8> {
    fn bytes_size(&self) -> u64 {
        (self.capacity() + std::mem::size_of_val(self)) as u64
    }
}
impl Data for String {
    fn bytes_size(&self) -> u64 {
        (self.capacity() + std::mem::size_of_val(self)) as u64
    }
}
