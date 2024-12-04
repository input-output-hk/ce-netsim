use super::{Packet, Round, SendError};
use crate::{
    data::Data,
    link::Link,
    measure::{Download, Upload},
};
use std::time::Duration;

pub struct Transit<T> {
    upload: Upload,
    link: Link,
    download: Download,
    data: Packet<T>,
}

impl<T> Transit<T>
where
    T: Data,
{
    pub(crate) fn new(
        mut upload: Upload,
        link: Link,
        download: Download,
        data: Packet<T>,
    ) -> Result<Self, SendError> {
        if !upload.send(data.bytes_size()) {
            let buffer_max_size = 0;
            let buffer_current_size = 0;

            Err(SendError::SenderBufferFull {
                sender: data.from(),
                buffer_max_size,
                buffer_current_size,
                packet_size: data.bytes_size(),
            })
        } else {
            Ok(Self {
                upload,
                link,
                download,
                data,
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
        measure::{Bandwidth, CongestionChannel, Latency},
        network::Route,
        node::{Node, NodeId},
    };
    use std::sync::Arc;

    const BD_1KBPS: Bandwidth = Bandwidth::new(1_024, Duration::from_secs(1));

    #[test]
    fn simple_case() {
        let sender: Node<[u8; 1042]> = Node::new(NodeId::ZERO);
        let link = Link::new(Latency::ZERO, Arc::new(CongestionChannel::new(BD_1KBPS)));
        let recipient: Node<[u8; 1042]> = Node::new(NodeId::ONE);
        let data = Packet::builder()
            .from(sender.id())
            .to(recipient.id())
            .data([0; 1_042])
            .build()
            .unwrap();

        let transit = Route::builder()
            .upload(&sender)
            .link(&link)
            .download(&recipient)
            .build()
            .unwrap()
            .transit(data)
            .unwrap();

        assert!(!transit.completed());
        assert!(!transit.corrupted());

        let mut transit = transit.complete().unwrap_err();

        let round = Round::ZERO.next();
        transit.advance(round, Duration::from_secs(1));

        assert!(!transit.completed());
        assert!(!transit.corrupted());

        let round = round.next();
        transit.advance(round, Duration::from_secs(1));

        assert!(transit.completed());
        assert!(!transit.corrupted());

        let Ok(_packet) = transit.complete() else {
            panic!("Transit didn't complete")
        };
    }
}
