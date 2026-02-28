use super::{Packet, Round, SendError};
use crate::{
    data::Data,
    link::LinkChannel,
    measure::{Download, Upload},
};
use std::time::Duration;

#[derive(Debug)]
pub struct Transit<T> {
    upload: Upload,
    link: LinkChannel,
    download: Download,
    data: Packet<T>,
}

impl<T> Transit<T>
where
    T: Data,
{
    pub(crate) fn new(
        mut upload: Upload,
        link: LinkChannel,
        download: Download,
        data: Packet<T>,
    ) -> Result<Self, SendError> {
        if upload.send(data.bytes_size()) {
            Ok(Self {
                upload,
                link,
                download,
                data,
            })
        } else {
            let buffer_max_size = upload.buffer_max_size();
            let buffer_current_size = upload.buffer_size();

            Err(SendError::SenderBufferFull {
                sender: data.from(),
                buffer_max_size,
                buffer_current_size,
                packet_size: data.bytes_size(),
            })
        }
    }

    pub fn advance(&mut self, round: Round, duration: Duration) {
        self.upload.update_capacity(round, duration);
        let uploaded = self.upload.process();

        self.link.update_capacity(round, duration);
        let transited = self.link.process(uploaded);

        self.download.update_capacity(round, duration);
        self.download.process(transited);
    }

    /// check if the data transiting is corrupted
    ///
    /// this is possible if the receiver's buffer is full as we are
    /// trying to receive data.
    ///
    /// we detect this by looking if the data size is different
    /// from the data present in all the pending buffers or by
    /// looking if the _Download_ end of the tranist is corrupted
    /// already.
    pub fn corrupted(&self) -> bool {
        self.download.corrupted()
            || self.data.bytes_size()
                != self
                    .upload
                    .bytes_in_buffer()
                    .saturating_add(self.link.bytes_in_transit())
                    .saturating_add(self.download.bytes_in_buffer())
    }

    pub fn completed(&self) -> bool {
        self.data.bytes_size() == self.download.bytes_in_buffer() && self.link.completed()
    }

    #[allow(clippy::result_large_err)]
    pub fn complete(self) -> Result<Packet<T>, Self> {
        if self.completed() {
            debug_assert!(self.upload.bytes_in_buffer() == 0);
            debug_assert!(self.link.bytes_in_transit() == 0);

            Ok(self.data)
        } else {
            Err(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        link::Link,
        measure::{Bandwidth, Latency, PacketLoss},
        network::{Route, packet::PacketIdGenerator},
        node::{Node, NodeId},
    };

    // 8 Mbps
    #[allow(clippy::declare_interior_mutable_const)]
    const BD: Bandwidth = Bandwidth::new(8_000_000);

    /// Helper: build a Transit from nodes, link, and payload.
    fn make_transit(
        sender: &Node,
        link: &Link,
        recipient: &Node,
        payload: [u8; 1_042],
    ) -> Transit<[u8; 1_042]> {
        let data = Packet::builder(&PacketIdGenerator::new())
            .from(sender.id())
            .to(recipient.id())
            .data(payload)
            .build()
            .unwrap();

        Route::new(sender, link, recipient).transit(data).unwrap()
    }

    #[test]
    fn simple_case() {
        let sender = Node::new(NodeId::ZERO);
        let link = Link::new(Latency::ZERO, BD, PacketLoss::default());
        let recipient = Node::new(NodeId::ONE);

        let transit = make_transit(&sender, &link, &recipient, [0; 1_042]);

        assert!(!transit.completed());
        assert!(!transit.corrupted());

        let mut transit = transit.complete().unwrap_err();

        // 600µs per round with 1 byte/µs bandwidth: 600 bytes capacity per round.
        // Round 1 sends 600 of 1042, round 2 sends the remaining 442.
        let round = Round::ZERO.next();
        transit.advance(round, Duration::from_micros(600));

        assert!(!transit.completed());
        assert!(!transit.corrupted());

        let round = round.next();
        transit.advance(round, Duration::from_micros(600));

        assert!(transit.completed());
        assert!(!transit.corrupted());

        let _packet = transit.complete().unwrap();
    }

    #[test]
    fn latency_delays_completion() {
        let sender = Node::new(NodeId::ZERO);
        // 100ms latency, high bandwidth so bytes transfer instantly.
        let link = Link::new(
            Latency::new(Duration::from_millis(100)),
            Bandwidth::MAX,
            PacketLoss::default(),
        );
        let recipient = Node::new(NodeId::ONE);

        let mut transit = make_transit(&sender, &link, &recipient, [0; 1_042]);

        // After 50ms the bytes have been uploaded and downloaded, but the
        // link latency countdown hasn't finished yet.
        let round = Round::ZERO.next();
        transit.advance(round, Duration::from_millis(50));

        assert!(!transit.completed(), "should still be in-flight (latency)");
        assert!(!transit.corrupted());

        // After another 60ms the latency is satisfied (total 110ms > 100ms).
        let round = round.next();
        transit.advance(round, Duration::from_millis(60));

        assert!(transit.completed());
        let _packet = transit.complete().unwrap();
    }

    #[test]
    fn corruption_when_download_buffer_too_small() {
        let sender = Node::new(NodeId::ZERO);
        let link = Link::new(Latency::ZERO, BD, PacketLoss::default());
        let mut recipient = Node::new(NodeId::ONE);
        // Set a download buffer smaller than the packet.
        recipient.set_download_buffer(100);

        let mut transit = make_transit(&sender, &link, &recipient, [0; 1_042]);

        // Advance enough to transfer all bytes — the download buffer will overflow,
        // marking the transit as corrupted.
        let round = Round::ZERO.next();
        transit.advance(round, Duration::from_millis(100));

        let round = round.next();
        transit.advance(round, Duration::from_millis(100));

        assert!(transit.corrupted());
    }

    #[test]
    fn sender_buffer_full_returns_error() {
        let mut sender = Node::new(NodeId::ZERO);
        sender.set_upload_buffer(100); // smaller than the 1042-byte packet
        let link = Link::new(Latency::ZERO, BD, PacketLoss::default());
        let recipient = Node::new(NodeId::ONE);

        let data = Packet::builder(&PacketIdGenerator::new())
            .from(sender.id())
            .to(recipient.id())
            .data([0u8; 1_042])
            .build()
            .unwrap();

        let err = Route::new(&sender, &link, &recipient)
            .transit(data)
            .unwrap_err();

        assert!(
            matches!(err, SendError::SenderBufferFull { .. }),
            "expected SenderBufferFull, got {err:?}"
        );
    }

    #[test]
    fn zero_byte_packet_completes_immediately() {
        let sender = Node::new(NodeId::ZERO);
        let link = Link::new(Latency::ZERO, BD, PacketLoss::default());
        let recipient = Node::new(NodeId::ONE);

        let data = Packet::builder(&PacketIdGenerator::new())
            .from(sender.id())
            .to(recipient.id())
            .data(())
            .build()
            .unwrap();

        let mut transit = Route::new(&sender, &link, &recipient)
            .transit(data)
            .unwrap();

        let round = Round::ZERO.next();
        transit.advance(round, Duration::from_micros(1));

        assert!(transit.completed());
        assert!(!transit.corrupted());
    }
}
