use crate::{link, HasBytesSize, SimSocket, SimUpLink};
use anyhow::{Context as _, Result};
use netsim_core::sim_context::{new_context, SimContextCore};
pub use netsim_core::{Edge, EdgePolicy, NodePolicy, SimConfiguration, SimId};

/// the context to keep on in order to continue adding/removing/monitoring nodes
/// in the sim-ed network.
pub struct SimContext<T: HasBytesSize> {
    core: SimContextCore<SimUpLink<T>>,
}

impl<T> SimContext<T>
where
    T: HasBytesSize,
{
    /// create a new [`SimContext`]. Creating this object will also start a
    /// multiplexer in the background. Make sure to call [`SimContext::shutdown`]
    /// for a clean shutdown of the background process.
    ///
    pub async fn new(configuration: SimConfiguration<T>) -> Self {
        let core = new_context(configuration);

        Self { core }
    }

    pub fn set_edge_policy(&mut self, edge: Edge, policy: EdgePolicy) {
        self.core.set_edge_policy(edge, policy)
    }

    pub fn set_node_policy(&mut self, node: SimId, policy: NodePolicy) {
        self.core.set_node_policy(node, policy)
    }

    /// Open a new [`SimSocket`] with the given configuration
    pub fn open(&mut self) -> Result<SimSocket<T>> {
        let (up, down) = link();

        let address = self
            .core
            .new_link(up)
            .context("Failed to reserve a new SimId")?;

        Ok(SimSocket::new(address, self.core.bus(), down))
    }

    /// clean shutdown the mutex.
    ///
    /// There are not timeout to this operation, for now this is left to the
    /// calling user to use the appropriate timeout as needed.
    pub async fn shutdown(self) -> Result<()> {
        self.core.shutdown()
    }
}
