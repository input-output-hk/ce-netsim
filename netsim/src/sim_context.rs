use crate::{
    sim_link::{link, SimUpLink},
    SimConfiguration, SimSocket, SimSocketConfiguration,
};
use anyhow::{anyhow, bail, Context as _, Result};
use ce_netsim_util::{
    sim_context::{new_context, SimContextCore, SimMuxCore},
    HasBytesSize, Msg,
};
use std::{
    sync::{
        atomic::{self, AtomicBool},
        mpsc, Arc,
    },
    thread,
    time::Duration,
};

pub struct SimContext<T> {
    core: SimContextCore<SimUpLink<T>>,
    generic_up_link: MuxSend<T>,

    shutdown: Arc<AtomicBool>,
    mux_handler: thread::JoinHandle<Result<()>>,
}

pub struct MuxSend<T>(mpsc::Sender<Msg<T>>);

struct Mux<T>
where
    T: HasBytesSize,
{
    core: SimMuxCore<SimUpLink<T>>,
    bus: mpsc::Receiver<Msg<T>>,

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

impl<T> SimContext<T>
where
    T: HasBytesSize,
{
    /// create a new [`SimContext`]. Creating this object will also start a
    /// multiplexer in the background. Make sure to call [`SimContext::shutdown`]
    /// for a clean shutdown of the background process.
    ///
    pub fn new(configuration: SimConfiguration) -> Self {
        let (sim_context_core, sim_mux_core) = new_context(configuration);
        let (generic_up_link, bus) = mpsc::channel();

        let shutdown = Arc::new(AtomicBool::new(false));
        let mux = Mux::new(sim_mux_core, Arc::clone(&shutdown), bus);
        let mux_handler = thread::spawn(|| run_mux(mux));

        Self {
            core: sim_context_core,
            generic_up_link: MuxSend(generic_up_link),
            shutdown,
            mux_handler,
        }
    }

    /// Open a new [`SimSocket`] with the given configuration
    pub fn open(&mut self, configuration: SimSocketConfiguration) -> Result<SimSocket<T>> {
        let (up, down) = link(configuration.download_bytes_per_sec);

        let address = self
            .core
            .new_link(up)
            .context("Failed to reserve a new SimId")?;

        Ok(SimSocket::new(address, self.generic_up_link.clone(), down))
    }

    pub fn shutdown(self) -> Result<()> {
        self.shutdown.store(true, atomic::Ordering::Relaxed);

        match self.mux_handler.join() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(error).context("NetSim Multiplexer error"),
            Err(error) => {
                bail!("NetSim Multiplexer panicked: {error:?}")
            }
        }
    }
}

impl<T> Mux<T>
where
    T: HasBytesSize,
{
    fn new(
        core: SimMuxCore<SimUpLink<T>>,
        shutdown: Arc<AtomicBool>,
        bus: mpsc::Receiver<Msg<T>>,
    ) -> Self {
        Self {
            core,
            bus,
            shutdown,
        }
    }

    fn process_new_msg(&mut self, msg: Msg<T>) -> Result<()> {
        self.core.inbound_message(msg)
    }

    fn propagate_msgs(&mut self) -> Result<()> {
        for msg in self.core.outbound_messages()? {
            self.propagate_msg(msg)?;
        }

        Ok(())
    }

    fn propagate_msg(&mut self, msg: Msg<T>) -> Result<()> {
        let dst = msg.to();
        let mut addresses = self
            .core
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
