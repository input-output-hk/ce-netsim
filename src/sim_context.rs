use crate::{
    defaults::DEFAULT_BYTES_PER_SEC, link, HasBytesSize, Msg, ShutdownController, ShutdownReceiver,
    SimId, SimSocket, SimUpLink, TimeQueue,
};
use anyhow::{anyhow, bail, Context, Result};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
use tokio::{
    select,
    task::JoinHandle,
    time::{sleep_until, Instant},
};
use tokio::{
    sync::mpsc,
    time::{sleep, Sleep},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SimConfiguration {
    pub bytes_per_sec: u64,
}

impl Default for SimConfiguration {
    fn default() -> Self {
        Self {
            bytes_per_sec: DEFAULT_BYTES_PER_SEC,
        }
    }
}

/// the context to keep on in order to continue adding/removing/monitoring nodes
/// in the sim-ed network.
pub struct SimContext<T> {
    configuration: SimConfiguration,

    generic_up_link: MuxSend<T>,

    /// new connection will add they UpLink side on this
    /// value for the [`Mux`] to redirect messages when
    /// it's time for it
    addresses: Addresses<T>,

    shutdown: ShutdownController,
    mux_handler: JoinHandle<Result<()>>,
}

pub struct MuxSend<T>(mpsc::UnboundedSender<Msg<T>>);

impl<T> Clone for MuxSend<T> {
    fn clone(&self) -> Self {
        MuxSend(self.0.clone())
    }
}

impl<T: HasBytesSize> MuxSend<T> {
    pub(crate) fn send(&self, msg: Msg<T>) -> Result<()> {
        self.0.send(msg).map_err(|error| {
            anyhow!(
                "Failed to send Msg ({size} bytes) from {from}, to {to}",
                from = error.0.from(),
                to = error.0.to(),
                size = error.0.content().bytes_size(),
            )
        })
    }
}

type Addresses<T> = Arc<Mutex<HashMap<SimId, SimUpLink<T>>>>;

struct Mux<T> {
    bus: mpsc::UnboundedReceiver<Msg<T>>,

    msgs: TimeQueue<T>,

    /// new addresses are registered on the [`SimContext`] side
    addresses: Addresses<T>,

    shutdown: ShutdownReceiver,
}

impl<T> SimContext<T>
where
    T: HasBytesSize,
{
    pub async fn new(configuration: SimConfiguration) -> Self {
        let addresses = Addresses::default();
        let (generic_up_link, bus) = mpsc::unbounded_channel();
        let shutdown = ShutdownController::new();

        let mux = Mux::new(shutdown.subscribe(), bus, Arc::clone(&addresses));
        let mux_handler = tokio::spawn(run_mux(mux));

        Self {
            configuration,
            generic_up_link: MuxSend(generic_up_link),
            addresses,
            shutdown,
            mux_handler,
        }
    }

    pub fn open(&self, address: SimId) -> Result<SimSocket<T>> {
        let (up, down) = link(self.configuration.bytes_per_sec);

        let mut addresses = self
            .addresses
            .lock()
            .map_err(|error| anyhow!("Failed to register address, mutex poisoned {error}"))?;
        addresses.insert(address.clone(), up);

        Ok(SimSocket::new(address, self.generic_up_link.clone(), down))
    }

    /// clean shutdown the mutex.
    ///
    /// There are not timeout to this operation, for now this is left to the
    /// calling user to use the appropriate timeout as needed.
    pub async fn shutdown(self) -> Result<()> {
        self.shutdown.shutdown();

        match self.mux_handler.await {
            // all good
            Ok(Ok(())) => Ok(()),
            // Mux error
            Ok(Err(error)) => Err(error).context("NetSim Multiplexer error"),
            // join error
            Err(error) => {
                Err(error).context("Failed to await for the NetSim Multiplexer to finish")
            }
        }
    }
}

impl<T> Mux<T> {
    fn new(
        shutdown: ShutdownReceiver,
        bus: mpsc::UnboundedReceiver<Msg<T>>,
        addresses: Addresses<T>,
    ) -> Self {
        Self {
            bus,
            addresses,
            msgs: TimeQueue::new(),
            shutdown,
        }
    }
}

impl<T> Mux<T>
where
    T: HasBytesSize,
{
    fn process_new_msg(&mut self, msg: Msg<T>) -> Result<()> {
        // 1. get the message time
        let sent_time = msg.time();
        // 2. get the link speed (bytes per seconds)
        let link_speed = {
            let dst = msg.to().clone();
            let mut addresses = self.addresses.lock().unwrap();

            match addresses.entry(dst) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.get().speed(),
                std::collections::hash_map::Entry::Vacant(_) => {
                    // by itself this is not an error, just someone sending something
                    // to an unknown address
                    return Ok(());
                }
            }
        };
        // 3. compute the msg delay
        let content_size = msg.content().bytes_size();
        let delay = Duration::from_secs(content_size / link_speed);

        // 4. compute the due time
        let due_by = sent_time + delay;

        self.msgs.push(due_by, msg);
        Ok(())
    }

    fn propagate_msgs(&mut self) -> Result<()> {
        for msg in self.msgs.pop_all_elapsed(SystemTime::now()) {
            self.propagate_msg(msg)?;
        }

        Ok(())
    }

    fn propagate_msg(&mut self, msg: Msg<T>) -> Result<()> {
        let dst = msg.to();
        let mut addresses = self
            .addresses
            .lock()
            .map_err(|error| anyhow!("Failed to acquire address, mutex poisonned {error}"))?;

        match addresses.entry(dst.clone()) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                if let Err(error) = entry.get().send(msg) {
                    if entry.get().is_closed() {
                        entry.remove();
                        // ignore the message, the other side is only shutdown
                    } else {
                        bail!("Failed to send message: {error}")
                    }
                }
            }
            std::collections::hash_map::Entry::Vacant(_) => {
                // do nothing
            }
        }

        Ok(())
    }

    fn wait_next_msg(&self) -> Sleep {
        match self.msgs.time_to_next_msg() {
            // if we are empty, we can wait a long time
            None => sleep_until(Instant::now() + Duration::from_secs(5)),
            // take the due time and compute the lapsed between now and then
            Some(then) => {
                let delay = if let Err(error) = then.elapsed() {
                    error.duration()
                } else {
                    Duration::ZERO
                };
                sleep(delay)
            }
        }
    }

    async fn step(&mut self) -> Result<bool> {
        let mut shutdown = self.shutdown.clone();
        let is_shutingdown = shutdown.is_shutting_down();
        let due_msg = self.wait_next_msg();
        let new_msg = self.bus.recv();

        select! {
            biased;

            // instruct the `run_mux` loop it's time to stop
            true = is_shutingdown => return Ok(false),
            // process all the pending messages
            _ = due_msg => self.propagate_msgs()?,
            // receive a new message from the bus
            Some(msg) = new_msg => self.process_new_msg(msg)?,
        };

        // all good, instruct we can continue
        Ok(true)
    }
}

async fn run_mux<T: HasBytesSize>(mut mux: Mux<T>) -> Result<()> {
    while mux.step().await? {}

    Ok(())
}
