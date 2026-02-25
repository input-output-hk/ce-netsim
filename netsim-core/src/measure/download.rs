use super::{Bandwidth, CongestionChannel, Gauge};
use crate::network::Round;
use std::{sync::Arc, time::Duration};

/// The download tracker for the [super::Route] of a message.
///
/// This will keep track of how much can be downloaded into the
/// recipient's Buffer and will make sure the buffer's limit are
/// respected.
///
/// The implementation of this is similar to how it would look like
/// in UDP messages of the network. The data will be downloaded
/// but if the buffer is full the data will be lost.
///
/// If that happens the data is then [`Self::corrupted`] because of missing
/// part or all of its content.
///
/// On `Drop` the associated data in the buffer is freed.
#[derive(Debug)]
pub struct Download {
    channel: Arc<CongestionChannel>,
    buffer: Arc<Gauge>,
    in_buffer: u64,
    corrupted: bool,
}

impl Download {
    pub(crate) fn new(channel: Arc<CongestionChannel>, buffer: Arc<Gauge>) -> Self {
        Self {
            channel,
            buffer,
            in_buffer: 0,
            corrupted: false,
        }
    }

    /// Returns `true` if any bytes of this packet were lost in transit to the
    /// receiver's buffer.
    ///
    /// In a simulation this models two real-world UDP drop conditions:
    ///
    /// 1. **Bandwidth saturation** — the link or receiver's download channel did
    ///    not have enough remaining capacity in this time step, so fewer bytes
    ///    were accepted than were offered.
    /// 2. **Buffer overflow** — the receiver's inbound buffer was full, so bytes
    ///    that cleared the channel could not be stored and were silently dropped.
    ///
    /// Once set, the flag is **sticky**: it remains `true` for the lifetime of
    /// this [`Download`] even if subsequent time steps have ample capacity.
    /// This reflects the fact that a UDP datagram with missing bytes is
    /// permanently unusable regardless of later network conditions.
    ///
    /// From a simulation user's perspective, a `Transit` whose download is
    /// corrupted will be discarded rather than delivered — the `handle` closure
    /// passed to [`Network::advance_with`] will not be called for that packet.
    ///
    /// [`Network::advance_with`]: crate::network::Network::advance_with
    pub fn corrupted(&self) -> bool {
        self.corrupted
    }

    pub fn update_capacity(&mut self, round: Round, duration: Duration) {
        self.channel.update_capacity(round, duration);
    }

    pub fn process(&mut self, size: u64) {
        let processed = self.channel.reserve(size);
        let downloaded = self.buffer.reserve(processed);

        self.corrupted = self.corrupted || size != processed || processed != downloaded;

        self.in_buffer = self.in_buffer.saturating_add(downloaded);
    }

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

    /// get the remaining bandwidth capacity of the download channel
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

impl Drop for Download {
    fn drop(&mut self) {
        self.buffer.free(self.in_buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::Bandwidth;

    // 1 byte/µs = 1_000_000 bytes/sec (minimum representable bandwidth)
    const BW: Bandwidth = Bandwidth::new(1, Duration::from_micros(1));

    #[test]
    fn create() {
        let gauge = Arc::new(Gauge::new());
        let channel = Arc::new(CongestionChannel::new(BW));

        let download = Download::new(channel, gauge);

        assert_eq!(download.bytes_in_buffer(), 0);
        assert!(!download.corrupted());
    }

    #[test]
    fn free_on_drop() {
        let gauge = Arc::new(Gauge::new());
        let channel = Arc::new(CongestionChannel::new(BW));

        let reserved = gauge.reserve(100);
        assert_eq!(reserved, 100);
        let mut download = Download::new(channel, gauge.clone());
        let round = Round::ZERO.next();

        download.update_capacity(round, Duration::from_secs(1));

        assert_eq!(gauge.used_capacity(), 100);
        download.process(100);

        assert_eq!(gauge.used_capacity(), 200);
        std::mem::drop(download);

        assert_eq!(gauge.used_capacity(), 100);
    }

    #[test]
    fn corrupted_no_buffer() {
        let gauge = Arc::new(Gauge::with_capacity(24));
        let channel = Arc::new(CongestionChannel::new(BW));
        let mut download = Download::new(channel, gauge);
        let round = Round::ZERO.next();

        assert_eq!(download.bytes_in_buffer(), 0);
        assert_eq!(download.channel.capacity(), 0);

        download.update_capacity(round, Duration::from_secs(1));

        download.process(1_024);
        assert!(download.corrupted());
        assert_eq!(download.in_buffer, 24);
    }

    #[test]
    fn corrupted_no_capacity() {
        let gauge = Arc::new(Gauge::new());
        let channel = Arc::new(CongestionChannel::new(BW));
        let mut download = Download::new(channel, gauge);

        assert_eq!(download.bytes_in_buffer(), 0);
        assert_eq!(download.channel.capacity(), 0);

        download.process(1_042);
        assert!(download.corrupted());
    }

    #[test]
    fn process_zero_does_not_corrupt() {
        let gauge = Arc::new(Gauge::new());
        let channel = Arc::new(CongestionChannel::new(BW));
        let mut download = Download::new(channel, gauge);
        let round = Round::ZERO.next();

        download.update_capacity(round, Duration::from_secs(1));
        download.process(0);
        assert!(!download.corrupted());
        assert_eq!(download.bytes_in_buffer(), 0);
    }

    #[test]
    fn corrupted_flag_is_sticky() {
        let gauge = Arc::new(Gauge::new());
        let channel = Arc::new(CongestionChannel::new(BW));
        let mut download = Download::new(channel, gauge);

        // No capacity → corrupted
        download.process(100);
        assert!(download.corrupted());

        // Even after updating capacity and processing 0, flag stays set
        let round = Round::ZERO.next();
        download.update_capacity(round, Duration::from_secs(1));
        download.process(0);
        assert!(download.corrupted());
    }

    #[test]
    fn corrupted_both_channel_and_buffer_limited() {
        // Channel capacity 50 bytes (1 byte/µs × 50µs), buffer capacity 30 bytes
        let gauge = Arc::new(Gauge::with_capacity(30));
        let channel = Arc::new(CongestionChannel::new(BW));
        let mut download = Download::new(channel, gauge);
        let round = Round::ZERO.next();

        download.update_capacity(round, Duration::from_micros(50));
        download.process(100); // 100 > 50 (channel) and 50 > 30 (buffer)

        assert!(download.corrupted());
        assert_eq!(download.bytes_in_buffer(), 30);
    }

    #[test]
    fn drop_frees_bytes_in_buffer() {
        let gauge = Arc::new(Gauge::new());
        let channel = Arc::new(CongestionChannel::new(BW));
        let mut download = Download::new(channel, gauge.clone());
        let round = Round::ZERO.next();

        // Pre-reserve some bytes so baseline is non-zero
        gauge.reserve(100);
        download.update_capacity(round, Duration::from_secs(1));
        download.process(200);

        assert_eq!(download.bytes_in_buffer(), 200);
        std::mem::drop(download);
        // Drop must free the 200 bytes that were in the download buffer
        assert_eq!(gauge.used_capacity(), 100);
    }
}
