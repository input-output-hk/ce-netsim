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
    pub fn channel_bandwidth(&self) -> Bandwidth {
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

    const BW: Bandwidth = Bandwidth::new(1_024, Duration::from_secs(1));

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

        let sent = upload.send(1_042);
        assert!(sent);

        assert_eq!(upload.bytes_in_buffer(), 1_042);
        assert_eq!(upload.channel.capacity(), 0);

        upload.update_capacity(round, Duration::from_secs(1));

        let processed = upload.process();
        assert_eq!(processed, 1_024);
        assert_eq!(upload.bytes_in_buffer(), 18);

        // if we are updating the capacity with the same round
        // we aren't really updating it and we can't process more
        // data.
        upload.update_capacity(round, Duration::from_secs(1));

        let processed = upload.process();
        assert_eq!(processed, 0);
        assert_eq!(upload.bytes_in_buffer(), 18);

        // only if we move to a subsequent round we do get more
        // capacity
        upload.update_capacity(round.next(), Duration::from_secs(1));

        let processed = upload.process();
        assert_eq!(processed, 18);
        assert_eq!(upload.bytes_in_buffer(), 0);
    }
}
