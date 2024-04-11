use crate::{
    bus::{open_bus, BusMessage, BusReceiver, BusSender},
    congestion_queue::CongestionQueue,
    policy::PolicyOutcome,
    Edge, EdgePolicy, HasBytesSize, Msg, NodePolicy, OnDrop, Policy, SimConfiguration, SimId,
};
use anyhow::{anyhow, bail, Context, Result};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
    thread,
    time::{Duration, Instant},
};

/// the collections of up links to other sockets
///
/// This is parameterised so that we can set async or non async channel
type Links<UpLink> = Arc<Mutex<HashMap<SimId, UpLink>>>;

pub trait Link {
    type Msg: HasBytesSize;

    fn send(&self, msg: Msg<Self::Msg>) -> Result<()>;
}

/// This is the execution context/controller of a simulated network
///
/// It is possible to have multiple [SimContext] opened concurrently
/// in the same process. However the nodes of a given context
/// will not be able to send messages to nodes of different context.
///
pub struct SimContextCore<UpLink: Link> {
    policy: Arc<RwLock<Policy>>,

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

    msgs: CongestionQueue<UpLink::Msg>,
}

impl<UpLink> SimContextCore<UpLink>
where
    UpLink: Link + Send + 'static,
{
    /// create a new [`SimContext`]. Creating this object will also start a
    /// multiplexer in the background. Make sure to call [`SimContext::shutdown`]
    /// for a clean shutdown of the background process.
    ///
    /// This function use the default [`SimConfiguration`].
    /// Use [`SimContext::with_config`] to start a [`SimContext`] with specific
    /// configurations.
    /// [`NodePolicy`] and [`EdgePolicy`] may still be set dynamically while the
    /// simulation is running.
    ///
    /// Note that this function starts a _multiplexer_ in a physical thread.
    pub fn new() -> Self {
        Self::with_config(SimConfiguration::default())
    }

    /// create a new [`SimContext`]. Creating this object will also start a
    /// multiplexer in the background. Make sure to call [`SimContext::shutdown`]
    /// for a clean shutdown of the background process.
    ///
    /// Note that this function starts a _multiplexer_ in a physical thread.
    ///
    pub fn with_config(configuration: SimConfiguration<UpLink::Msg>) -> Self {
        let policy = Arc::new(RwLock::new(configuration.policy));
        let links = Arc::new(Mutex::new(HashMap::new()));
        let next_sim_id = SimId::ZERO.next(); // Starts at 1

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

    /// set a specific policy between the two `Node` that compose the [`Edge`].
    ///
    /// when no specific policies are set, the default policies are used.
    /// To reset, use [`SimContext::reset_edge_policy`], and the default
    /// policy will be used again.
    ///
    pub fn set_edge_policy(&mut self, edge: Edge, policy: EdgePolicy) {
        self.policy.write().unwrap().set_edge_policy(edge, policy)
    }

    /// Reset the [`EdgePolicy`] between two nodes of an [`Edge`]. The default
    /// EdgePolicy for this SimContext will be used.
    ///
    pub fn reset_edge_policy(&mut self, edge: Edge) {
        self.policy.write().unwrap().reset_edge_policy(edge)
    }

    /// Set a specific [`NodePolicy`] for a given node ([SimSocket]).
    ///
    /// If not set, the default [NodePolicy] for the [SimContext] will be
    /// used instead.
    ///
    /// Call [`SimContext::reset_node_policy`] to reset the [`NodePolicy`]
    /// so that the default policy will be used onward.
    ///
    pub fn set_node_policy(&mut self, node: SimId, policy: NodePolicy) {
        self.policy.write().unwrap().set_node_policy(node, policy)
    }

    /// Reset the specific [`NodePolicy`] associated to the given node
    /// ([SimSocket]) so that the default policy will be used again going
    /// forward.
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

    /// Shutdown the context. All remaining opened [SimSocket] will become
    /// non functional and will return a `Disconnected` error when trying
    /// to receive messages or when trying to send messages
    ///
    /// This function is blocking and will block until the multiplexer
    /// thread has shutdown.
    ///
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
        let msgs = CongestionQueue::new();
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
    pub fn inbound_message(&mut self, time: Instant, msg: Msg<UpLink::Msg>) -> Result<()> {
        let mut configuration = self.policy.write().expect("Never poisonned");

        match configuration.process(&msg) {
            PolicyOutcome::Drop => {
                if let Some(on_drop) = self.on_drop.as_ref() {
                    on_drop.handle(msg.into_content())
                }
            }
            PolicyOutcome::Delay { delay } => self.msgs.push(time + delay, msg),
        }

        Ok(())
    }

    /// function to returns all the outbound messages
    ///
    /// these are the messages that are due to be sent.
    /// This function may returns an empty `Vec` and this
    /// simply means there are no messages to be forwarded
    pub fn outbound_messages(&mut self, time: Instant) -> Result<Vec<Msg<UpLink::Msg>>> {
        Ok(self.msgs.pop_many(time, &self.policy.read().unwrap()))
    }

    /// get the earliest time to the next message
    ///
    /// Function returns `None` if there are no due messages
    /// to forward
    pub fn earliest_outbound_time(&self) -> Option<Instant> {
        // self.msgs.time_to_next_msg()
        None
    }

    fn propagate_msgs(&mut self, time: Instant) -> Result<()> {
        for msg in self.outbound_messages(time)? {
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

    fn step(&mut self, time: Instant) -> Result<MuxOutcome> {
        while let Some(bus_message) = self.bus.try_receive() {
            match bus_message {
                BusMessage::Disconnected | BusMessage::Shutdown => {
                    return Ok(MuxOutcome::Shutdown);
                }
                BusMessage::Message(msg) => self.inbound_message(time, msg)?,
            }
        }

        self.propagate_msgs(time)?;

        Ok(MuxOutcome::Continue)
    }

    pub(crate) fn sleep_time(&mut self, current_time: Instant) -> Instant {
        let Some(time) = self.earliest_outbound_time() else {
            return current_time + self.idle_duration;
        };

        std::cmp::min(time, current_time + self.idle_duration)
    }
}

enum MuxOutcome {
    Continue,
    Shutdown,
}

fn run_mux<UpLink: Link>(mut mux: SimMuxCore<UpLink>) -> Result<()> {
    loop {
        let time = Instant::now();

        match mux.step(time)? {
            MuxOutcome::Continue => (),
            MuxOutcome::Shutdown => break,
        }

        #[cfg(not(feature = "thread_sleep_until"))]
        {
            let dur = mux.sleep_time(time).duration_since(Instant::now());
            thread::sleep(dur)
        }

        // TODO: use when thread_sleep_until is stabilised
        // https://github.com/rust-lang/rust/issues/113752
        #[cfg(feature = "thread_sleep_until")]
        thread::sleep_until(mux.sleep_time(time));
    }

    Ok(())
}

impl<UpLink> Default for SimContextCore<UpLink>
where
    UpLink: Link + Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}
