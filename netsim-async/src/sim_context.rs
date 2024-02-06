use crate::{
    link, HasBytesSize, Msg, ShutdownController, ShutdownReceiver, SimId, SimSocket, SimUpLink,
};
use anyhow::{anyhow, bail, Context, Result};
use ce_netsim_util::sim_context::{new_context, SimContextCore, SimMuxCore};
pub use ce_netsim_util::{SimConfiguration, SimSocketConfiguration};
use std::time::Duration;
use tokio::{
    select,
    task::JoinHandle,
    time::{sleep_until, Instant},
};
use tokio::{
    sync::mpsc,
    time::{sleep, Sleep},
};

/// the context to keep on in order to continue adding/removing/monitoring nodes
/// in the sim-ed network.
pub struct SimContext<T> {
    core: SimContextCore<SimUpLink<T>>,

    generic_up_link: MuxSend<T>,
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

struct Mux<T>
where
    T: HasBytesSize,
{
    core: SimMuxCore<SimUpLink<T>>,

    bus: mpsc::UnboundedReceiver<Msg<T>>,

    shutdown: ShutdownReceiver,
}

impl<T> SimContext<T>
where
    T: HasBytesSize,
{
    /// create a new [`SimContext`]. Creating this object will also start a
    /// multiplexer in the background. Make sure to call [`SimContext::shutdown`]
    /// for a clean shutdown of the background process.
    ///
    pub async fn new(configuration: SimConfiguration) -> Self {
        let (sim_context_core, sim_mux_core) = new_context(configuration);

        let (generic_up_link, bus) = mpsc::unbounded_channel();
        let shutdown = ShutdownController::new();

        let mux = Mux::new(sim_mux_core, shutdown.subscribe(), bus);
        let mux_handler = tokio::spawn(run_mux(mux));

        Self {
            core: sim_context_core,
            generic_up_link: MuxSend(generic_up_link),
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
            .core
            .links()
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

impl<T> Mux<T>
where
    T: HasBytesSize,
{
    fn new(
        core: SimMuxCore<SimUpLink<T>>,
        shutdown: ShutdownReceiver,
        bus: mpsc::UnboundedReceiver<Msg<T>>,
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
        match self.core.earliest_outbound_time() {
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
