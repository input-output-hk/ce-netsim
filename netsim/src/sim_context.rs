use crate::{
    sim_link::{link, SimUpLink},
    SimConfiguration, SimSocket, SimSocketConfiguration,
};
use anyhow::{anyhow, bail, Context as _, Result};
use ce_netsim_util::{HasBytesSize, Msg, SimId, TimeQueue};
use std::{
    collections::HashMap,
    sync::{
        atomic::{self, AtomicBool},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime},
};

type Addresses<T> = Arc<Mutex<HashMap<SimId, SimUpLink<T>>>>;

pub struct SimContext<T> {
    #[allow(unused)]
    configuration: SimConfiguration,

    generic_up_link: MuxSend<T>,

    addresses: Addresses<T>,

    shutdown: Arc<AtomicBool>,
    mux_handler: thread::JoinHandle<Result<()>>,
}

pub struct MuxSend<T>(mpsc::Sender<Msg<T>>);

struct Mux<T> {
    bus: mpsc::Receiver<Msg<T>>,

    msgs: TimeQueue<T>,

    /// new addresses are registered on the [`SimContext`] side
    addresses: Addresses<T>,

    shutdown: Arc<AtomicBool>,
}

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

impl<T> Mux<T> {
    fn new(
        shutdown: Arc<AtomicBool>,
        bus: mpsc::Receiver<Msg<T>>,
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

impl<T> SimContext<T>
where
    T: HasBytesSize,
{
    /// create a new [`SimContext`]. Creating this object will also start a
    /// multiplexer in the background. Make sure to call [`SimContext::shutdown`]
    /// for a clean shutdown of the background process.
    ///
    pub fn new(configuration: SimConfiguration) -> Self {
        let addresses = Addresses::default();
        let (generic_up_link, bus) = mpsc::channel();

        let shutdown = Arc::new(AtomicBool::new(false));
        let mux = Mux::new(Arc::clone(&shutdown), bus, Arc::clone(&addresses));
        let mux_handler = thread::spawn(|| run_mux(mux));

        Self {
            configuration,
            generic_up_link: MuxSend(generic_up_link),
            addresses,
            shutdown,
            mux_handler,
        }
    }

    /// Open a new [`SimSocket`] with the given configuration
    pub fn open(
        &self,
        address: SimId,
        configuration: SimSocketConfiguration,
    ) -> Result<SimSocket<T>> {
        let (up, down) = link(configuration.download_bytes_per_sec);

        let mut addresses = self
            .addresses
            .lock()
            .map_err(|error| anyhow!("Failed to register address, mutex poisoned {error}"))?;
        addresses.insert(address.clone(), up);

        Ok(SimSocket::new(address, self.generic_up_link.clone(), down))
    }

    pub fn shutdown(self) -> Result<()> {
        self.shutdown.store(true, atomic::Ordering::Relaxed);

        match self.mux_handler.join() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(error).context("NetSim Multiplexer error"),
            Err(error) => {
                bail!("NetSim Multiplexer panick: {error:?}")
            }
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

    fn handle_bus_msg(&mut self) -> Result<bool> {
        match self.bus.try_recv() {
            Ok(msg) => {
                self.process_new_msg(msg)?;
                Ok(true)
            }
            Err(mpsc::TryRecvError::Empty) => Ok(false),
            Err(mpsc::TryRecvError::Disconnected) => bail!("No more sender connected on the bus"),
        }
    }

    fn step(&mut self) -> Result<bool> {
        // check we haven't been requested to shutdown
        if self.shutdown.load(atomic::Ordering::Relaxed) {
            return Ok(false);
        }

        // process all the messages on the bus
        while self.handle_bus_msg()? {}

        self.propagate_msgs()?;

        Ok(!self.shutdown.load(atomic::Ordering::Relaxed))
    }
}

fn run_mux<T: HasBytesSize>(mut mux: Mux<T>) -> Result<()> {
    while mux.step()? {
        // TODO: configure
        //
        thread::sleep(Duration::from_millis(200));
    }

    Ok(())
}
