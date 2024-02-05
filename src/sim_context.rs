use crate::{
    defaults::{DEFAULT_BYTES_PER_SEC, DEFAULT_MUX_ID},
    link, HasBytesSize, Msg, SimDownLink, SimId, SimSocket, SimUpLink, TimeOrdered,
};
use anyhow::{anyhow, bail, Result};
use std::{
    alloc::System,
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
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

    generic_up_link: SimUpLink<T>,

    /// new connection will add they UpLink side on this
    /// value for the [`Mux`] to redirect messages when
    /// it's time for it
    addresses: Addresses<T>,

    mux_handler: JoinHandle<()>,
}

type Addresses<T> = Arc<Mutex<HashMap<SimId, SimUpLink<T>>>>;

struct Mux<T> {
    bus: SimDownLink<T>,

    msgs: TimeOrdered<T>,

    /// new addresses are registered on the [`SimContext`] side
    addresses: Addresses<T>,
}

impl<T> SimContext<T>
where
    T: HasBytesSize,
{
    pub async fn new(configuration: SimConfiguration) -> Self {
        let addresses = Addresses::default();
        let (generic_up_link, bus) = link(u64::MAX);

        let mux = Mux::new(bus, Arc::clone(&addresses));
        let mux_handler = tokio::spawn(run_mux(mux));

        Self {
            configuration,
            generic_up_link,
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
    fn new(bus: SimDownLink<T>, addresses: Addresses<T>) -> Self {
        Self {
            bus,
            addresses,
            msgs: TimeOrdered::new(),
        }
    }
}

impl<T> Mux<T>
where
    T: HasBytesSize,
{
    fn process_new_msg(&mut self, msg: Msg<T>) -> Result<()> {
        // 1. get the link speed
        // 2. compute the msg delay
        let delay = Duration::from_secs(0);
        let due_by = SystemTime::now() + delay;

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
            Some(msg) = new_msg => self.process_new_msg(msg)?,
            Some(msg) = due_msg => self.propagate_msg(msg)?,
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
