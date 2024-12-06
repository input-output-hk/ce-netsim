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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn void() {
        assert_eq!(().bytes_size(), 0);
    }

    #[test]
    fn u8() {
        assert_eq!(u8::MIN.bytes_size(), 1);
        assert_eq!(42u8.bytes_size(), 1);
        assert_eq!(u8::MAX.bytes_size(), 1);
    }

    #[test]
    fn box_u8() {
        assert_eq!(
            <Box<[u8]> as Data>::bytes_size(&vec![0u8].into_boxed_slice()),
            1
        );
        assert_eq!(
            <Box<[u8]> as Data>::bytes_size(&vec![0u8; 12].into_boxed_slice()),
            12
        );
    }

    #[test]
    fn string() {
        #![allow(clippy::identity_op)]

        const STRING_OVERHEAD: u64 = 24;

        assert_eq!(String::new().bytes_size(), 0 + STRING_OVERHEAD);
        assert_eq!(
            "hello world!".to_string().bytes_size(),
            12 + STRING_OVERHEAD
        );
    }

    #[test]
    fn static_str_() {
        const STR: &str = "hello world!";

        assert_eq!((&STR).bytes_size(), 12);
    }

    #[test]
    fn str_() {
        assert_eq!("".bytes_size(), 0);
        assert_eq!("hello world!".bytes_size(), 12);
    }

    #[test]
    fn vec() {
        #![allow(clippy::identity_op)]

        const VEC_OVERHEAD: u64 = 24;

        assert_eq!(vec![].bytes_size(), 0 + VEC_OVERHEAD);
        assert_eq!(vec![0u8; 12].bytes_size(), 12 + VEC_OVERHEAD);
    }
}
