use super::{Node, Packet, SendError, transit::Transit};
use crate::{
    data::Data,
    link::{Link, LinkChannel, LinkDirection},
    measure::{Download, Upload},
};

#[derive(Debug)]
pub struct Route {
    upload: Upload,
    link: LinkChannel,
    download: Download,
}

impl Route {
    pub(super) fn new<T>(from: &Node<T>, link: &Link, to: &Node<T>) -> Self {
        let direction = if from.id() < to.id() {
            LinkDirection::Forward
        } else {
            LinkDirection::Reverse
        };

        Self {
            upload: from.upload(),
            link: link.channel(direction),
            download: to.download(),
        }
    }

    pub fn upload(&self) -> &crate::measure::Upload {
        &self.upload
    }

    pub fn download(&self) -> &Download {
        &self.download
    }

    pub fn transit<T>(self, data: Packet<T>) -> Result<Transit<T>, SendError>
    where
        T: Data,
    {
        Transit::new(self.upload, self.link, self.download, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        measure::{Bandwidth, Latency, PacketLoss},
        node::NodeId,
    };

    #[test]
    fn new() {
        let sender: Node<()> = Node::new(NodeId::ZERO);
        let link = Link::new(Latency::ZERO, Bandwidth::MAX, PacketLoss::default());
        let recipient: Node<()> = Node::new(NodeId::ONE);

        let _route = Route::new(&sender, &link, &recipient);
    }
}
