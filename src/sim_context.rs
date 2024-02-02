use crate::{
    defaults::{DEFAULT_BYTES_PER_SEC, DEFAULT_MUX_ID},
    link, HasBytesSize, SimDownLink, SimId, SimSocket, SimUpLink,
};
use anyhow::{anyhow, bail, Result};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::task::JoinHandle;

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

    /// new addresses are registered on the [`SimContext`] side
    addresses: Addresses<T>,
}

impl<T> SimContext<T>
where
    T: HasBytesSize,
{
    pub async fn new(configuration: SimConfiguration) -> Self {
        let addresses = Addresses::default();
        let (generic_up_link, bus) = link(DEFAULT_MUX_ID, u64::MAX);

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
        let (up, down) = link(address, self.configuration.bytes_per_sec);

        let mut addresses = self
            .addresses
            .lock()
            .map_err(|error| anyhow!("Failed to register address, mutex poisoned {error}"))?;
        addresses.insert(down.id().clone(), up);

        Ok(SimSocket::new(self.generic_up_link.clone(), down))
    }
}

impl<T> Mux<T> {
    fn new(bus: SimDownLink<T>, addresses: Addresses<T>) -> Self {
        Self { bus, addresses }
    }
}

impl<T> Mux<T>
where
    T: HasBytesSize,
{
    async fn step(&mut self) -> Result<Option<()>> {
        if let Some(msg) = self.bus.recv().await {
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

            Ok(Some(()))
        } else {
            Ok(None)
        }
    }
}

async fn run_mux<T: HasBytesSize>(mut mux: Mux<T>) {
    loop {
        if let Err(error) = mux.step().await {
            todo!("Unmanaged error: {error:?}")
        }
    }
}
