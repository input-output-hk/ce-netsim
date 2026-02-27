use super::{Bandwidth, CongestionChannel, Gauge};
use crate::network::Round;
use std::{sync::Arc, time::Duration};

/// The upload tracker for the [`super::Route`] of a message.
///
/// The upload will keep track of the buffer (i.e. it will start
/// releasing the space in the buffer as we process messages through
/// the [`CongestionChannel`] -- the actual act of uploading data
/// to the network)
///
/// On `Drop` the remaining data in the sender's upload buffer will
/// be freed.
#[derive(Debug)]
pub struct Upload {
    buffer: Arc<Gauge>,
    in_buffer: u64,

    channel: Arc<CongestionChannel>,
}

impl Upload {
    pub fn new(buffer: Arc<Gauge>, channel: Arc<CongestionChannel>) -> Self {
        Self {
            buffer,
            in_buffer: 0,
            channel,
        }
    }

    pub fn send(&mut self, size: u64) -> bool {
        let reserved = self.buffer.reserve(size);
        if reserved != size {
            self.buffer.free(reserved);
            false
        } else {
            self.in_buffer = size;
            true
        }
    }

    pub fn update_capacity(&mut self, round: Round, duration: Duration) {
        self.channel.update_capacity(round, duration);
    }

    /// attempt to process as much as possible
    pub fn process(&mut self) -> u64 {
        let reserved = self.channel.reserve(self.in_buffer);
        self.buffer.free(reserved);
        self.in_buffer = self.in_buffer.saturating_sub(reserved);
        reserved
    }

    /// this is only relevant for the use of the [`Transit`]
    ///
    /// Get the number of bytes for the given _route_ for the given data
    /// that is currently in the upload part of the journey.
    ///
    /// [`Transit`]: crate::network::Transit
    pub(crate) fn bytes_in_buffer(&self) -> u64 {
        self.in_buffer
    }

    /// get the maximum capacity of the buffer
    pub fn buffer_max_size(&self) -> u64 {
        self.buffer.maximum_capacity()
    }

    /// get the current buffer size (the current used part of the buffer)
    ///
    pub fn buffer_size(&self) -> u64 {
        self.buffer.used_capacity()
    }

    /// get the configured bandwidth for this component
    pub fn channel_bandwidth(&self) -> &Bandwidth {
        self.channel.bandwidth()
    }

    /// get the remaining bandwidth capacity of the upload channel
    ///
    /// If this is `0` this means that the bandwidth was used up and
    /// there is no more capacity.
    ///
    /// If there is more bytes in the buffer than there is remaining
    /// bandwidth capacity this means that this component of the network
    /// is becoming a bottleneck.
    pub fn channel_remaining_bandwidth(&self) -> u64 {
        self.channel.capacity()
    }
}

impl Drop for Upload {
    fn drop(&mut self) {
        self.buffer.free(self.in_buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::Bandwidth;

    // 1 byte/µs = 1_000_000 bytes/sec (minimum representable bandwidth)
    #[allow(clippy::declare_interior_mutable_const)]
    const BW: Bandwidth = Bandwidth::new(1, Duration::from_micros(1));

    #[test]
    fn create() {
        let gauge = Arc::new(Gauge::new());
        let channel = Arc::new(CongestionChannel::new(BW));

        let upload = Upload::new(gauge, channel);

        assert_eq!(upload.bytes_in_buffer(), 0);
    }

    #[test]
    fn free_on_drop() {
        let gauge = Arc::new(Gauge::new());
        let channel = Arc::new(CongestionChannel::new(BW));

        let reserved = gauge.reserve(100);
        assert_eq!(reserved, 100);
        let mut upload = Upload::new(gauge.clone(), channel);

        assert_eq!(gauge.used_capacity(), 100);
        assert!(upload.send(100));

        assert_eq!(gauge.used_capacity(), 200);
        std::mem::drop(upload);

        assert_eq!(gauge.used_capacity(), 100);
    }

    #[test]
    fn process() {
        let gauge = Arc::new(Gauge::new());
        let channel = Arc::new(CongestionChannel::new(BW));
        let mut upload = Upload::new(gauge, channel);
        let round = Round::ZERO.next();

        assert_eq!(upload.bytes_in_buffer(), 0);
        assert_eq!(upload.channel.capacity(), 0);

        // send 1_500_000 bytes; bandwidth allows 1_000_000 per second
        let sent = upload.send(1_500_000);
        assert!(sent);

        assert_eq!(upload.bytes_in_buffer(), 1_500_000);
        assert_eq!(upload.channel.capacity(), 0);

        upload.update_capacity(round, Duration::from_secs(1));

        let processed = upload.process();
        assert_eq!(processed, 1_000_000);
        assert_eq!(upload.bytes_in_buffer(), 500_000);

        // same round — capacity is exhausted, nothing more processed
        upload.update_capacity(round, Duration::from_secs(1));

        let processed = upload.process();
        assert_eq!(processed, 0);
        assert_eq!(upload.bytes_in_buffer(), 500_000);

        // advance to next round — fresh 1_000_000-byte capacity
        upload.update_capacity(round.next(), Duration::from_secs(1));

        let processed = upload.process();
        assert_eq!(processed, 500_000);
        assert_eq!(upload.bytes_in_buffer(), 0);
    }

    #[test]
    fn send_zero_succeeds_without_touching_buffer() {
        let gauge = Arc::new(Gauge::new());
        let channel = Arc::new(CongestionChannel::new(BW));
        let mut upload = Upload::new(gauge.clone(), channel);

        assert!(upload.send(0));
        assert_eq!(upload.bytes_in_buffer(), 0);
        assert_eq!(gauge.used_capacity(), 0);
    }

    #[test]
    fn send_fails_when_buffer_full_and_leaves_no_leak() {
        // Buffer with capacity 50
        let gauge = Arc::new(Gauge::with_capacity(50));
        let channel = Arc::new(CongestionChannel::new(BW));
        let mut upload = Upload::new(gauge.clone(), channel);

        // Fill the buffer
        assert!(upload.send(50));
        assert_eq!(gauge.used_capacity(), 50);

        // A second upload sharing the same gauge cannot send any more
        let mut upload2 = Upload::new(gauge.clone(), Arc::new(CongestionChannel::new(BW)));
        assert!(!upload2.send(1));
        assert_eq!(upload2.bytes_in_buffer(), 0);
        assert_eq!(gauge.used_capacity(), 50); // unchanged
    }

    #[test]
    fn process_with_empty_buffer_returns_zero() {
        let gauge = Arc::new(Gauge::new());
        let channel = Arc::new(CongestionChannel::new(BW));
        let mut upload = Upload::new(gauge, channel);
        let round = Round::ZERO.next();

        upload.update_capacity(round, Duration::from_secs(1));
        let processed = upload.process();
        assert_eq!(processed, 0);
    }

    #[test]
    fn drop_frees_only_remaining_in_buffer() {
        let gauge = Arc::new(Gauge::new());
        let channel = Arc::new(CongestionChannel::new(BW));
        let mut upload = Upload::new(gauge.clone(), channel);
        let round = Round::ZERO.next();

        upload.send(1_500_000);
        upload.update_capacity(round, Duration::from_secs(1));
        upload.process(); // processes 1_000_000, leaves 500_000 in buffer

        assert_eq!(upload.bytes_in_buffer(), 500_000);
        let used_before_drop = gauge.used_capacity();
        std::mem::drop(upload);
        // Drop must free exactly the remaining 500_000
        assert_eq!(gauge.used_capacity(), used_before_drop - 500_000);
    }
}
