use crate::{
    Edge, EdgePolicy, HasBytesSize, Msg, NameService, NodePolicy, SimConfiguration, SimId,
    TimeQueue,
};
use anyhow::{anyhow, Result};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
    time::SystemTime,
};

/// the collections of up links to other sockets
///
/// This is parameterised so that we can set async or non async channel
type Links<UpLink> = Arc<Mutex<HashMap<SimId, UpLink>>>;

pub trait Link {
    type Msg: HasBytesSize;
}

pub struct SimContextCore<UpLink> {
    configuration: Arc<RwLock<SimConfiguration>>,

    ns: NameService,

    next_sim_id: SimId,

    links: Links<UpLink>,
}

pub struct SimMuxCore<UpLink: Link> {
    configuration: Arc<RwLock<SimConfiguration>>,

    links: Links<UpLink>,

    msgs: TimeQueue<UpLink::Msg>,
}

pub fn new_context<UpLink: Link>(
    configuration: SimConfiguration,
) -> (SimContextCore<UpLink>, SimMuxCore<UpLink>) {
    let context = SimContextCore::new(configuration);
    let mux = SimMuxCore::new(
        Arc::clone(context.configuration()),
        Arc::clone(context.links()),
    );

    (context, mux)
}

impl<UpLink> SimContextCore<UpLink> {
    fn new(configuration: SimConfiguration) -> Self {
        let configuration = Arc::new(RwLock::new(configuration));
        let links = Arc::new(Mutex::new(HashMap::new()));
        let next_sim_id = SimId::ZERO.next(); // Starts at 1
        let ns = NameService::new();

        Self {
            ns,
            configuration,
            next_sim_id,
            links,
        }
    }

    pub fn configuration(&self) -> &Arc<RwLock<SimConfiguration>> {
        &self.configuration
    }

    pub fn links(&self) -> &Links<UpLink> {
        &self.links
    }

    pub fn ns(&self) -> &NameService {
        &self.ns
    }

    pub fn set_edge_policy(&mut self, edge: Edge, policy: EdgePolicy) {
        self.configuration
            .write()
            .unwrap()
            .policy
            .set_edge_policy(edge, policy)
    }

    pub fn set_node_policy(&mut self, node: SimId, policy: NodePolicy) {
        self.configuration
            .write()
            .unwrap()
            .policy
            .set_node_policy(node, policy)
    }

    pub fn new_link(&mut self, link: UpLink) -> Result<SimId> {
        let id = self.next_sim_id;

        let collision = self
            .links
            .lock()
            .map_err(|error| anyhow!("Failed to lock on the links: {error}"))?
            .insert(id, link);

        debug_assert!(
            collision.is_none(),
            "Collision of SimId (here: {id}) shouldn't be possible"
        );

        self.next_sim_id = id.next();
        Ok(id)
    }
}

impl<UpLink> SimMuxCore<UpLink>
where
    UpLink: Link,
{
    fn new(configuration: Arc<RwLock<SimConfiguration>>, links: Links<UpLink>) -> Self {
        let msgs = TimeQueue::new();
        Self {
            configuration,
            links,
            msgs,
        }
    }

    pub fn configuration(&self) -> &Arc<RwLock<SimConfiguration>> {
        &self.configuration
    }

    pub fn links(&self) -> &Links<UpLink> {
        &self.links
    }

    /// process an inbound message
    ///
    /// The message propagation speed will be computed based on
    /// the upload, download and general link speed between
    pub fn inbound_message(&mut self, msg: Msg<UpLink::Msg>) -> Result<()> {
        let due_by = self
            .configuration
            .read()
            .unwrap()
            .policy
            .message_due_time(&msg);

        self.msgs.push(due_by, msg);

        Ok(())
    }

    /// function to returns all the outbound messages
    ///
    /// these are the messages that are due to be sent.
    /// This function may returns an empty `Vec` and this
    /// simply means there are no messages to be forwarded
    pub fn outbound_messages(&mut self) -> Result<Vec<Msg<UpLink::Msg>>> {
        Ok(self.msgs.pop_all_elapsed(SystemTime::now()))
    }

    /// get the earliest time to the next message
    ///
    /// Function returns `None` if there are no due messages
    /// to forward
    pub fn earliest_outbound_time(&self) -> Option<SystemTime> {
        self.msgs.time_to_next_msg()
    }
}
