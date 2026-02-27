use super::{Node, Packet, SendError, transit::Transit};
use crate::{
    data::Data,
    link::{Link, LinkChannel, LinkDirection},
    measure::{Download, Upload},
};
use anyhow::anyhow;

/// create a route but doesn't initiate sending a packet through the network.
///
/// This is useful to probe performances
#[derive(Default)]
pub struct RouteBuilder {
    upload: Option<Upload>,
    link: Option<LinkChannel>,
    download: Option<Download>,
}

#[derive(Debug)]
pub struct Route {
    upload: Upload,
    link: LinkChannel,
    download: Download,
}

impl RouteBuilder {
    pub fn new() -> Self {
        Self {
            upload: None,
            link: None,
            download: None,
        }
    }

    pub fn upload<T>(mut self, node: &Node<T>) -> Self {
        self.upload = Some(node.upload());
        self
    }

    pub fn download<T>(mut self, node: &Node<T>) -> Self {
        self.download = Some(node.download());
        self
    }

    pub(crate) fn link(mut self, link: &Link, direction: LinkDirection) -> Self {
        self.link = Some(link.channel(direction));
        self
    }

    pub fn build(self) -> anyhow::Result<Route> {
        let upload = self
            .upload
            .ok_or_else(|| anyhow!("The upload route hasn't been setup"))?;
        let link = self
            .link
            .ok_or_else(|| anyhow!("The link route hasn't been setup"))?;
        let download = self
            .download
            .ok_or_else(|| anyhow!("The download route hasn't been setup"))?;

        Ok(Route {
            upload,
            link,
            download,
        })
    }
}

impl Route {
    pub fn builder() -> RouteBuilder {
        RouteBuilder::new()
    }

    pub fn upload(&self) -> &Upload {
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
        link::LinkDirection,
        measure::{Bandwidth, Latency, PacketLoss},
        node::NodeId,
    };

    #[test]
    fn builder_missing_sender() {
        let error = Route::builder().build().unwrap_err();

        assert_eq!(error.to_string(), "The upload route hasn't been setup")
    }

    #[test]
    fn builder_missing_link() {
        let sender: Node<()> = Node::new(NodeId::ZERO);

        let error = Route::builder().upload(&sender).build().unwrap_err();

        assert_eq!(error.to_string(), "The link route hasn't been setup")
    }

    #[test]
    fn builder_missing_recipient() {
        let sender: Node<()> = Node::new(NodeId::ZERO);
        let link = Link::new(Latency::ZERO, Bandwidth::MAX, PacketLoss::default());

        let error = Route::builder()
            .upload(&sender)
            .link(&link, LinkDirection::Forward)
            .build()
            .unwrap_err();

        assert_eq!(error.to_string(), "The download route hasn't been setup")
    }

    #[test]
    fn build() {
        let sender: Node<()> = Node::new(NodeId::ZERO);
        let link = Link::new(Latency::ZERO, Bandwidth::MAX, PacketLoss::default());
        let recipient: Node<()> = Node::new(NodeId::ONE);

        let _route = Route::builder()
            .upload(&sender)
            .link(&link, LinkDirection::Forward)
            .download(&recipient)
            .build()
            .unwrap();
    }
}
