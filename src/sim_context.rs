use crate::{
    defaults::DEFAULT_BYTES_PER_SEC, link, HasBytesSize, Msg, SimDownLink, SimId, SimSocket,
    SimUpLink, TimeQueue,
};
use anyhow::{anyhow, bail, Result};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
use tokio::sync::mpsc;
use tokio::{select, task::JoinHandle};

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

    mux_handler: JoinHandle<()>,
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
}

impl<T> SimContext<T>
where
    T: HasBytesSize,
{
    pub async fn new(configuration: SimConfiguration) -> Self {
        let addresses = Addresses::default();
        let (generic_up_link, bus) = mpsc::unbounded_channel();

        let mux = Mux::new(bus, Arc::clone(&addresses));
        let mux_handler = tokio::spawn(run_mux(mux));

        Self {
            configuration,
            generic_up_link: MuxSend(generic_up_link),
            addresses,
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
}

impl<T> Mux<T> {
    fn new(bus: mpsc::UnboundedReceiver<Msg<T>>, addresses: Addresses<T>) -> Self {
        Self {
            bus,
            addresses,
            msgs: TimeQueue::new(),
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
        // 2. get the link speed
        // 3. compute the msg delay
        let delay = Duration::from_secs(0);

        // 4. compute the due time
        let due_by = sent_time + delay;

        self.msgs.push(due_by, msg);
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

    async fn step(&mut self) -> Result<Option<()>> {
        let new_msg = self.bus.recv();
        let due_msg = self.msgs.wait_pop();

        select! {
            biased;

            Some(msg) = due_msg => self.propagate_msg(msg)?,
            Some(msg) = new_msg => self.process_new_msg(msg)?,
        };

        Ok(Some(()))
    }
}

async fn run_mux<T: HasBytesSize>(mut mux: Mux<T>) {
    loop {
        if let Err(error) = mux.step().await {
            todo!("Unmanaged error: {error:?}")
        }
    }
}
