pub(crate) mod command;
mod stop;

use self::{
    command::{Command, CommandReceiver, CommandSender, command_channel},
    stop::Stop,
};
use crate::socket::SimSocketBuilder;
use anyhow::{Context, Result, bail};
use command::NewNodeCommand;
use netsim_core::{
    Network, NodeId, Packet,
    data::Data,
    network::{PacketIdGenerator, SendError},
};
use std::{
    collections::{HashMap, hash_map::Entry},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{SyncSender, TryRecvError, TrySendError},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

struct NodeEntry<T> {
    sender: SyncSender<Packet<T>>,
    dropped: Arc<AtomicU64>,
}

pub struct SimContext<T> {
    commands: CommandSender<T>,

    packet_id_generator: PacketIdGenerator,

    stop: Arc<Stop>,

    thread: JoinHandle<Result<()>>,
}

/// Builder for configuring a link between two nodes in a [`SimContext`].
///
/// Obtained via [`SimContext::configure_link`]. Call [`.apply()`](SimLinkBuilder::apply) to send
/// the configuration to the multiplexer.
pub struct SimLinkBuilder<'a, T> {
    a: NodeId,
    b: NodeId,
    latency: netsim_core::Latency,
    bandwidth: netsim_core::Bandwidth,
    packet_loss: netsim_core::PacketLoss,
    commands: &'a mut CommandSender<T>,
}

impl<T> SimLinkBuilder<'_, T> {
    /// Set the one-way latency of this link.
    pub fn set_latency(mut self, latency: netsim_core::Latency) -> Self {
        self.latency = latency;
        self
    }

    /// Set the shared bandwidth capacity of this link.
    pub fn set_bandwidth(mut self, bandwidth: netsim_core::Bandwidth) -> Self {
        self.bandwidth = bandwidth;
        self
    }

    /// Set the probabilistic packet loss rate for this link.
    pub fn set_packet_loss(mut self, packet_loss: netsim_core::PacketLoss) -> Self {
        self.packet_loss = packet_loss;
        self
    }

    /// Send the link configuration to the multiplexer.
    pub fn apply(self) -> Result<()> {
        self.commands.send_configure_link(
            self.a,
            self.b,
            self.latency,
            self.bandwidth,
            self.packet_loss,
        )
    }
}

struct Multiplexer<T> {
    network: Network<T>,

    commands: CommandReceiver<T>,

    nodes: HashMap<NodeId, NodeEntry<T>>,

    stop: Arc<Stop>,
}

impl<T> SimContext<T>
where
    T: Data,
{
    pub fn new() -> Result<Self> {
        let stop = Arc::new(Stop::new());
        let (commands, receiver) = command_channel();

        let multiplexer = Multiplexer::<T>::new(receiver, Arc::clone(&stop));

        let packet_id_generator = multiplexer.network.packet_id_generator().clone();

        let thread = std::thread::spawn(|| multiplexer_run(multiplexer));

        Ok(Self {
            stop,
            packet_id_generator,
            commands,
            thread,
        })
    }

    pub fn open(&mut self) -> SimSocketBuilder<'_, T> {
        SimSocketBuilder::new(self.commands.clone(), self.packet_id_generator.clone())
    }

    /// Returns a point-in-time snapshot of the network state.
    ///
    /// Blocks briefly until the multiplexer processes the request.
    /// Includes per-node buffer usage, bandwidth, drop counts,
    /// and per-link latency, bandwidth, packet loss, and bytes in transit.
    pub fn stats(&mut self) -> Result<crate::stats::SimStats> {
        self.commands.send_stats()
    }

    /// Configure the link between two nodes.
    ///
    /// Returns a [`SimLinkBuilder`] to set latency and bandwidth.
    /// Call [`.apply()`](SimLinkBuilder::apply) to send the configuration to the multiplexer.
    ///
    /// This must be called after both nodes are created.
    pub fn configure_link(&mut self, a: NodeId, b: NodeId) -> SimLinkBuilder<'_, T> {
        SimLinkBuilder {
            a,
            b,
            latency: netsim_core::Latency::default(),
            bandwidth: netsim_core::Bandwidth::default(),
            packet_loss: netsim_core::PacketLoss::default(),
            commands: &mut self.commands,
        }
    }

    pub fn shutdown(self) -> Result<()> {
        self.stop.toggle();

        match self.thread.join() {
            Err(join_error) => {
                bail!("Multiplexer failed to clean shutdown: {join_error:?}")
            }
            Ok(Err(error)) => Err(error).context("Multiplexer failed with error"),
            Ok(Ok(())) => Ok(()),
        }
    }
}

impl<T> Multiplexer<T>
where
    T: Data,
{
    fn new(commands: CommandReceiver<T>, stop: Arc<Stop>) -> Self {
        let network = Network::<T>::new();
        let nodes = HashMap::new();
        Self {
            network,
            commands,
            nodes,
            stop,
        }
    }

    fn stopped(&self) -> bool {
        self.stop.get()
    }

    fn inbound(&mut self, command: Command<T>) {
        match command {
            Command::Packet(packet) => {
                match self.network.send(packet) {
                    Ok(()) => (),
                    Err(SendError::Route(_error)) => {
                        // failed to build the route between the two nodes;
                    }
                    Err(SendError::SenderBufferFull { sender, .. }) => {
                        // UDP semantics: dropped packets are not errors, they are expected.
                        // Increment the drop counter so callers can observe the loss.
                        if let Some(entry) = self.nodes.get(&sender) {
                            entry.dropped.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
            Command::ConfigureLink {
                a,
                b,
                latency,
                bandwidth,
                packet_loss,
            } => {
                self.network
                    .configure_link(a, b)
                    .set_latency(latency)
                    .set_bandwidth(bandwidth)
                    .set_packet_loss(packet_loss)
                    .apply();
            }
            Command::NewNode(nnc, reply) => {
                let NewNodeCommand {
                    sender,
                    dropped,
                    upload_bandwidth,
                    upload_buffer,
                    download_bandwidth,
                    download_buffer,
                } = nnc;
                let node_id = self
                    .network
                    .new_node()
                    .set_upload_bandwidth(upload_bandwidth)
                    .set_upload_buffer(upload_buffer)
                    .set_download_bandwidth(download_bandwidth)
                    .set_download_buffer(download_buffer)
                    .build();
                let Ok(()) = reply.send(node_id) else {
                    // the only reason this would happen is if the receiving end is dropped
                    // before we reply. This shouldn't happen so an error happened before.
                    // we just ignore and continue; if the `SimContext` is dropped
                    // we will detect it on the loop and we will stop the thread eventually
                    return;
                };

                self.nodes.insert(node_id, NodeEntry { sender, dropped });
            }
            Command::Stats(reply) => {
                use crate::stats::{NodeStats, SimStats};

                let core_stats = self.network.stats();

                let nodes = core_stats
                    .nodes
                    .into_iter()
                    .map(|ns| {
                        let packets_dropped = self
                            .nodes
                            .get(&ns.id)
                            .map(|e| e.dropped.load(Ordering::Relaxed))
                            .unwrap_or(0);
                        NodeStats {
                            inner: ns,
                            packets_dropped,
                        }
                    })
                    .collect();

                let stats = SimStats {
                    nodes,
                    links: core_stats.links,
                };

                // ignore if the receiver was already dropped
                let _ = reply.send(stats);
            }
        }
    }

    fn inbounds(&mut self) {
        loop {
            match self.commands.try_recv() {
                Err(TryRecvError::Disconnected) => {
                    // we will never receive any new messages and it is okay to
                    // disconnect and stop the thread.
                    self.stop.toggle();

                    break;
                }
                Err(TryRecvError::Empty) => break,
                Ok(command) => self.inbound(command),
            }
        }
    }

    fn step(&mut self, duration: Duration) {
        self.inbounds();

        self.network.advance_with(duration, |packet| {
            if let Entry::Occupied(mut entry) = self.nodes.entry(packet.to()) {
                match entry.get_mut().sender.try_send(packet) {
                    Ok(()) => (),
                    Err(TrySendError::Disconnected(_)) => {
                        entry.remove();
                    }
                    Err(TrySendError::Full(_)) => {
                        // receiver full, do nothing and drop the packet
                    }
                }
            }
        });
    }
}

const TARGETTED_ELAPSED: Duration = Duration::from_micros(200);

fn multiplexer_run<T>(mut multiplexer: Multiplexer<T>) -> Result<()>
where
    T: Data,
{
    let mut instant = Instant::now();

    // adjust for how much time we overspent in the last
    // computation of the `step`.
    //
    // i.e. we allow up to `5ms`. However if it took `6ms`
    // this means that the loop took `1ms` more than expected.
    //
    // on the next round we add this extra duration in the
    // step so that we will have taken into account that extra
    // time in propagating the messages through the network.
    let mut adjustment = Duration::ZERO;

    while !multiplexer.stopped() {
        multiplexer.step(TARGETTED_ELAPSED.saturating_add(adjustment));

        // compute how much time actually elapsed while performing the
        // multiplexer core operations
        let elapsed = instant.elapsed();

        // if we have too much time in our hand (i.e. TARGET > elapsed)
        // then we will want to sleep off the remaining time
        let sleep_duration = TARGETTED_ELAPSED.saturating_sub(elapsed);

        // if we have not enough time (i.e. TARGET < elapsed) then
        // we will want to account for that extra time on the following
        // run.
        adjustment = elapsed.saturating_sub(TARGETTED_ELAPSED);

        std::thread::sleep(sleep_duration);

        instant = Instant::now();
    }

    Ok(())
}
