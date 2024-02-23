use crate::{
    bus::{open_bus, BusMessage, BusReceiver, BusSender},
    policy::PolicyOutcome,
    Edge, EdgePolicy, HasBytesSize, Msg, NameService, NodePolicy, OnDrop, Policy, SimConfiguration,
    SimId, TimeQueue,
};
use anyhow::{anyhow, bail, Context, Result};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
    thread,
    time::{Duration, SystemTime},
};

/// the collections of up links to other sockets
///
/// This is parameterised so that we can set async or non async channel
type Links<UpLink> = Arc<Mutex<HashMap<SimId, UpLink>>>;

pub trait Link {
    type Msg: HasBytesSize;

    fn send(&self, msg: Msg<Self::Msg>) -> Result<()>;
}

pub struct SimContextCore<UpLink: Link> {
    policy: Arc<RwLock<Policy>>,

    ns: NameService,

    next_sim_id: SimId,

    bus: BusSender<UpLink::Msg>,

    links: Links<UpLink>,

    mux_handler: thread::JoinHandle<Result<()>>,
}

pub struct SimMuxCore<UpLink: Link> {
    policy: Arc<RwLock<Policy>>,

    on_drop: Option<OnDrop<UpLink::Msg>>,

    idle_duration: Duration,

    bus: BusReceiver<UpLink::Msg>,

    links: Links<UpLink>,

    msgs: TimeQueue<UpLink::Msg>,
}

impl<UpLink> SimContextCore<UpLink>
where
    UpLink: Link + Send + 'static,
{
    pub fn new(configuration: SimConfiguration<UpLink::Msg>) -> Self {
        let policy = Arc::new(RwLock::new(configuration.policy));
        let links = Arc::new(Mutex::new(HashMap::new()));
        let next_sim_id = SimId::ZERO.next(); // Starts at 1
        let ns = NameService::new();

        let (sender, receiver) = open_bus();

        let mux = SimMuxCore::new(
            Arc::clone(&policy),
            configuration.on_drop,
            configuration.idle_duration,
            receiver,
            Arc::clone(&links),
        );

        let mux_handler = thread::spawn(|| run_mux(mux));

        Self {
            ns,
            policy,
            next_sim_id,
            bus: sender,
            links,
            mux_handler,
        }
    }

    pub fn configuration(&self) -> &Arc<RwLock<Policy>> {
        &self.policy
    }

    pub fn links(&self) -> &Links<UpLink> {
        &self.links
    }

    pub fn ns(&self) -> &NameService {
        &self.ns
    }

    pub fn set_edge_policy(&mut self, edge: Edge, policy: EdgePolicy) {
        self.policy.write().unwrap().set_edge_policy(edge, policy)
    }

    pub fn reset_edge_policy(&mut self, edge: Edge) {
        self.policy.write().unwrap().reset_edge_policy(edge)
    }

    pub fn set_node_policy(&mut self, node: SimId, policy: NodePolicy) {
        self.policy.write().unwrap().set_node_policy(node, policy)
    }

    pub fn reset_node_policy(&mut self, node: SimId) {
        self.policy.write().unwrap().reset_node_policy(node)
    }

    pub fn bus(&self) -> BusSender<UpLink::Msg> {
        self.bus.clone()
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

    pub fn shutdown(self) -> Result<()> {
        self.bus
            .send_shutdown()
            .context("Failed to send shutdown signal to the mutiplexer")?;

        match self.mux_handler.join() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(error).context("Multiplexer fails with an error"),
            Err(join_error) => bail!("Failed to await the mutiplexer's to finish: {join_error:?}"),
        }
    }
}

impl<UpLink> SimMuxCore<UpLink>
where
    UpLink: Link,
{
    fn new(
        policy: Arc<RwLock<Policy>>,
        on_drop: Option<OnDrop<UpLink::Msg>>,
        idle_duration: Duration,
        bus: BusReceiver<UpLink::Msg>,
        links: Links<UpLink>,
    ) -> Self {
        let msgs = TimeQueue::new();
        Self {
            policy,
            on_drop,
            idle_duration,
            links,
            bus,
            msgs,
        }
    }

    pub fn configuration(&self) -> &Arc<RwLock<Policy>> {
        &self.policy
    }

    pub fn links(&self) -> &Links<UpLink> {
        &self.links
    }

    /// process an inbound message
    ///
    /// The message propagation speed will be computed based on
    /// the upload, download and general link speed between
    pub fn inbound_message(&mut self, msg: Msg<UpLink::Msg>) -> Result<()> {
        let mut configuration = self.policy.write().expect("Never poisonned");

        match configuration.process(&msg) {
            PolicyOutcome::Drop => {
                if let Some(on_drop) = self.on_drop.as_ref() {
                    on_drop.handle(msg.into_content())
                }
            }
            PolicyOutcome::Delay { until } => self.msgs.push(until, msg),
        }

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

    fn propagate_msgs(&mut self) -> Result<()> {
        for msg in self.outbound_messages()? {
            self.propagate_msg(msg)?;
        }

        Ok(())
    }

    fn propagate_msg(&mut self, msg: Msg<UpLink::Msg>) -> Result<()> {
        let dst = msg.to();
        let mut addresses = self
            .links()
            .lock()
            .map_err(|error| anyhow!("Failed to acquire address, mutex poisonned {error}"))?;

        match addresses.entry(dst) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                if entry.get().send(msg).is_err() {
                    entry.remove();
                }
            }
            std::collections::hash_map::Entry::Vacant(_) => {
                // do nothing
            }
        }

        Ok(())
    }

    fn step(&mut self) -> Result<MuxOutcome> {
        while let Some(bus_message) = self.bus.try_receive() {
            match bus_message {
                BusMessage::Disconnected | BusMessage::Shutdown => {
                    return Ok(MuxOutcome::Shutdown);
                }
                BusMessage::Message(msg) => self.inbound_message(msg)?,
            }
        }

        self.propagate_msgs()?;

        Ok(MuxOutcome::Continue)
    }

    pub(crate) fn sleep_time(&mut self) -> Duration {
        let Some(time) = self.earliest_outbound_time() else {
            return self.idle_duration;
        };

        SystemTime::now()
            .duration_since(time)
            .unwrap_or(self.idle_duration)
    }
}

enum MuxOutcome {
    Continue,
    Shutdown,
}

fn run_mux<UpLink: Link>(mut mux: SimMuxCore<UpLink>) -> Result<()> {
    loop {
        match mux.step()? {
            MuxOutcome::Continue => (),
            MuxOutcome::Shutdown => break,
        }

        thread::sleep(mux.sleep_time());
    }

    Ok(())
}
