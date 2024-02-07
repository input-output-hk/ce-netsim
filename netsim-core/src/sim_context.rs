use crate::{HasBytesSize, Msg, NameService, SimConfiguration, SimId, TimeQueue};
use anyhow::{anyhow, Result};
use std::{
    cmp,
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

/// the collections of up links to other sockets
///
/// This is parameterised so that we can set async or non async channel
type Links<UpLink> = Arc<Mutex<HashMap<SimId, UpLink>>>;

pub trait Link {
    type Msg: HasBytesSize;
    fn download_speed(&self) -> u64;
    fn upload_speed(&self) -> u64;
}

pub struct SimContextCore<UpLink> {
    configuration: Arc<SimConfiguration>,

    ns: NameService,

    next_sim_id: SimId,

    links: Links<UpLink>,
}

pub struct SimMuxCore<UpLink: Link> {
    configuration: Arc<SimConfiguration>,

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
        let configuration = Arc::new(configuration);
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

    pub fn configuration(&self) -> &Arc<SimConfiguration> {
        &self.configuration
    }

    pub fn links(&self) -> &Links<UpLink> {
        &self.links
    }

    pub fn ns(&self) -> &NameService {
        &self.ns
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
    fn new(configuration: Arc<SimConfiguration>, links: Links<UpLink>) -> Self {
        let msgs = TimeQueue::new();
        Self {
            configuration,
            links,
            msgs,
        }
    }

    pub fn configuration(&self) -> &Arc<SimConfiguration> {
        &self.configuration
    }

    pub fn links(&self) -> &Links<UpLink> {
        &self.links
    }

    /// compute the message speed (bytes per second) of a given message
    ///
    /// Will return `None` if there are no senders or recipients for this message
    fn compute_message_speed(&self, msg: &Msg<UpLink::Msg>) -> Option<u64> {
        // lock the links so we can query the recipient's link and the sender's link
        // and get the necessa
        let links = self
            .links
            .lock()
            .expect("Under no condition we expect the mutex to be poisoned");

        // 2. get the upload speed (the sender of the message)
        let upload_speed = links.get(&msg.from()).map(|link| link.upload_speed())?;
        // 3. get the download speed (the recipient of the message)
        let download_speed = links.get(&msg.to()).map(|link| link.download_speed())?;
        // 4. the message speed is the minimum value between the upload and download
        Some(cmp::min(upload_speed, download_speed))
    }

    /// process an inbound message
    ///
    /// The message propagation speed will be computed based on
    /// the upload, download and general link speed between
    pub fn inbound_message(&mut self, msg: Msg<UpLink::Msg>) -> Result<()> {
        // 1. get the message sent time
        let sent_time = msg.time();
        // 2. get the message speed
        let Some(speed) = self.compute_message_speed(&msg) else {
            // if we don't have a message speed, it means we don't have
            // recipients or senders for this message, and we can ignore
            // it.
            return Ok(());
        };
        // 3. compute the delay of the message
        let content_size = msg.content().bytes_size();
        let delay = Duration::from_secs(content_size / speed);
        // 4. compute the due by time
        let due_by = sent_time + delay;

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
