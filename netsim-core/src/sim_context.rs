use crate::{
    bus::{open_bus, BusMessage, BusReceiver, BusSender},
    congestion_queue::CongestionQueue,
    policy::PolicyOutcome,
    Edge, EdgePolicy, HasBytesSize, Msg, NodePolicy, Policy, SimConfiguration, SimId,
};
use anyhow::{bail, Context, Result};
use std::{sync::mpsc, thread, time::Instant};

/// the collections of up links to other sockets
///
/// This is parameterised so that we can set async or non async channel
pub(crate) type SimLinks<UpLink> = Vec<SimLink<UpLink>>;

pub trait Link {
    type Msg: HasBytesSize;

    fn send(&self, msg: Msg<Self::Msg>) -> Result<()>;
}

pub(crate) struct SimLink<UpLink> {
    link: UpLink,
    policy: Option<NodePolicy>,
}

/// This is the execution context/controller of a simulated network
///
/// It is possible to have multiple [SimContext] opened concurrently
/// in the same process. However the nodes of a given context
/// will not be able to send messages to nodes of different context.
///
pub struct SimContextCore<UpLink: Link> {
    bus: BusSender<UpLink>,

    mux_handler: thread::JoinHandle<Result<()>>,
}

pub struct SimMuxCore<UpLink: Link> {
    next_sim_id: SimId,

    configuration: SimConfiguration<UpLink::Msg>,

    bus: BusReceiver<UpLink>,

    links: SimLinks<UpLink>,

    msgs: CongestionQueue<UpLink::Msg>,
}

impl<UpLink> SimLink<UpLink> {
    pub(crate) fn new(link: UpLink) -> Self {
        Self { link, policy: None }
    }

    pub(crate) fn policy(&self) -> Option<NodePolicy> {
        self.policy
    }
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
        let (sender, receiver) = open_bus();

        let mux = SimMuxCore::<UpLink>::new(configuration, receiver);

        let mux_handler = thread::spawn(|| run_mux(mux));

        Self {
            bus: sender,
            mux_handler,
        }
    }

    pub fn configuration(&self) -> Policy {
        todo!()
    }

    /// set a specific policy between the two `Node` that compose the [`Edge`].
    ///
    /// when no specific policies are set, the default policies are used.
    /// To reset, use [`SimContext::reset_edge_policy`], and the default
    /// policy will be used again.
    ///
    #[inline]
    pub fn set_edge_policy(&mut self, edge: Edge, policy: EdgePolicy) -> Result<()> {
        self.bus().send_edge_policy_set(edge, policy)
    }

    /// Reset the [`EdgePolicy`] between two nodes of an [`Edge`]. The default
    /// EdgePolicy for this SimContext will be used.
    ///
    #[inline]
    pub fn reset_edge_policy(&mut self, edge: Edge) -> Result<()> {
        self.bus().send_edge_policy_reset(edge)
    }

    /// Set a specific [`NodePolicy`] for a given node ([SimSocket]).
    ///
    /// If not set, the default [NodePolicy] for the [SimContext] will be
    /// used instead.
    ///
    /// Call [`SimContext::reset_node_policy`] to reset the [`NodePolicy`]
    /// so that the default policy will be used onward.
    ///
    #[inline]
    pub fn set_node_policy(&mut self, node: SimId, policy: NodePolicy) -> Result<()> {
        self.bus().send_node_policy_set(node, policy)
    }

    /// Reset the specific [`NodePolicy`] associated to the given node
    /// ([SimSocket]) so that the default policy will be used again going
    /// forward.
    #[inline]
    pub fn reset_node_policy(&mut self, node: SimId) -> Result<()> {
        self.bus().send_node_policy_reset(node)
    }

    #[inline]
    pub fn bus(&self) -> BusSender<UpLink> {
        self.bus.clone()
    }

    #[inline]
    pub fn new_link(&mut self, link: UpLink) -> Result<SimId> {
        let (send_reply, reply) = mpsc::sync_channel(1);
        self.bus().send_node_add(link, send_reply)?;

        reply
            .recv()
            .context("Failed to receive reply from the Routing thread")
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
    fn new(configuration: SimConfiguration<UpLink::Msg>, bus: BusReceiver<UpLink>) -> Self {
        let msgs = CongestionQueue::new();
        let next_sim_id = SimId::ZERO; // Starts at 0
        let links = Vec::new();
        Self {
            configuration,
            next_sim_id,
            links,
            bus,
            msgs,
        }
    }

    /// process an inbound message
    ///
    /// The message propagation speed will be computed based on
    /// the upload, download and general link speed between
    pub fn inbound_message(&mut self, time: Instant, msg: Msg<UpLink::Msg>) -> Result<()> {
        match self.configuration.policy.process(&msg) {
            PolicyOutcome::Drop => {
                if let Some(on_drop) = self.configuration.on_drop.as_ref() {
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
        Ok(self
            .msgs
            .pop_many(time, &self.links, &self.configuration.policy))
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

        if let Some(sim_link) = self.links.get_mut(dst.into_index()) {
            let _error = sim_link.link.send(msg);
            Ok(())
        } else {
            panic!("We shouldn't have any recipient of messages with an index that is not valid")
        }
    }

    fn step(&mut self, time: Instant) -> Result<MuxOutcome> {
        while let Some(bus_message) = self.bus.try_receive() {
            match bus_message {
                BusMessage::Disconnected | BusMessage::Shutdown => {
                    return Ok(MuxOutcome::Shutdown);
                }
                BusMessage::Message(msg) => self.inbound_message(time, msg)?,

                BusMessage::NodeAdd(link, reply) => {
                    let id = self.next_sim_id;

                    self.links.push(SimLink::new(link));
                    self.next_sim_id = self.next_sim_id.next();

                    debug_assert_eq!(
                        self.links.len(),
                        self.next_sim_id.into_index(),
                        "The next available SimId is the lenght of the vec"
                    );

                    if let Err(error) = reply.send(id) {
                        bail!("Failed to reply to a new node creation request: {error:?}")
                    }
                }

                BusMessage::NodePolicyDefault(policy) => {
                    self.configuration.policy.set_default_node_policy(policy)
                }
                BusMessage::NodePolicySet(id, policy) => {
                    let _policy_set = self
                        .links
                        .get_mut(id.into_index())
                        .map(|node| node.policy = Some(policy))
                        .is_some();

                    debug_assert!(_policy_set, "We should always have a node for any given ID")
                }
                BusMessage::NodePolicyReset(id) => {
                    let _policy_set = self
                        .links
                        .get_mut(id.into_index())
                        .map(|node| node.policy = None)
                        .is_some();

                    debug_assert!(_policy_set, "We should always have a node for any given ID")
                }
                BusMessage::EdgePolicyDefault(policy) => {
                    self.configuration.policy.set_default_edge_policy(policy)
                }
                BusMessage::EdgePolicySet(id, policy) => {
                    self.configuration.policy.set_edge_policy(id, policy)
                }
                BusMessage::EdgePolicyReset(id) => self.configuration.policy.reset_edge_policy(id),
            }
        }

        self.propagate_msgs(time)?;

        Ok(MuxOutcome::Continue)
    }

    pub(crate) fn sleep_time(&mut self, current_time: Instant) -> Instant {
        let Some(time) = self.earliest_outbound_time() else {
            return current_time + self.configuration.idle_duration;
        };

        std::cmp::min(time, current_time + self.configuration.idle_duration)
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
