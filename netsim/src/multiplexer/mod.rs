pub(crate) mod command;
mod stop;

use self::{
    command::{command_channel, Command, CommandReceiver, CommandSender},
    stop::Stop,
};
use crate::socket::SimSocketBuilder;
use anyhow::{bail, Context, Result};
use command::NewNodeCommand;
use netsim_core::{data::Data, network::SendError, Network, NodeId, Packet};
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{
        mpsc::{SyncSender, TryRecvError, TrySendError},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub struct SimContext<T> {
    commands: CommandSender<T>,

    stop: Arc<Stop>,

    thread: JoinHandle<Result<()>>,
}

struct Multiplexer<T> {
    network: Network<T>,

    commands: CommandReceiver<T>,

    nodes: HashMap<NodeId, SyncSender<Packet<T>>>,

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

        let thread = std::thread::spawn(|| multiplexer_run(multiplexer));

        Ok(Self {
            stop,
            commands,
            thread,
        })
    }

    pub fn open(&mut self) -> SimSocketBuilder<'_, T> {
        SimSocketBuilder::new(self.commands.clone())
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
                    Err(SendError::Route(error)) => {
                        // failed to build the route between the two nodes;
                    }
                    Err(SendError::SenderBufferFull { .. }) => {
                        // the sender's buffer is full, we should notify the
                        // sender that the buffer is full and no new message
                        // can be added

                        // TODO:
                        //    in essense we want to avoid doing that because we are
                        //    adding too much go and back between the mutiplexer and
                        //    the nodes. Ideally we want the node to reserve the packet
                        //    on their side before the send
                        //
                        //    Or maybe this is not a problem and it only comes down to
                        //    implemented a "TCP/UDP" layer on top of `netsim`.
                    }
                }
            }
            Command::NewNode(nnc, reply) => {
                let NewNodeCommand {
                    sender,
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

                self.nodes.insert(node_id, sender);
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
            if let Entry::Occupied(mut receiver) = self.nodes.entry(packet.to()) {
                match receiver.get_mut().try_send(packet) {
                    Ok(()) => (),
                    Err(TrySendError::Disconnected(_)) => {
                        receiver.remove();
                    }
                    Err(TrySendError::Full(_)) => {
                        // receiver full, do nothing and drop the packet
                    }
                }
            }
        });
    }
}

const TARGETTED_ELAPSED: Duration = Duration::from_micros(1800);

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
        multiplexer.step(TARGETTED_ELAPSED + adjustment);

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
