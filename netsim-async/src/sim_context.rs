use crate::{link, HasBytesSize, SimSocket, SimUpLink};
use anyhow::{Context as _, Result};
use netsim_core::sim_context::SimContextCore;
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
    pub fn open(&mut self) -> Result<SimSocket<T>> {
        let (up, down) = link();

        let address = self
            .core
            .new_link(up)
            .context("Failed to reserve a new SimId")?;

        Ok(SimSocket::new(address, self.core.bus(), down))
    }

    pub fn new() -> Self {
        let configuration = SimConfiguration::default();

        Self::with_config(configuration)
    }

    pub fn with_config(configuration: SimConfiguration<T>) -> Self {
        let sim_context_core = SimContextCore::with_config(configuration);

        Self {
            core: sim_context_core,
        }
    }

    pub fn shutdown(self) -> Result<()> {
        self.core.shutdown()
    }

    pub fn set_node_policy(&mut self, node: SimId, policy: NodePolicy) {
        self.core.set_node_policy(node, policy)
    }

    pub fn set_edge_policy(&mut self, edge: Edge, policy: EdgePolicy) {
        self.core.set_edge_policy(edge, policy)
    }

    pub fn reset_node_policy(&mut self, node: SimId) {
        self.core.reset_node_policy(node)
    }

    pub fn reset_edge_policy(&mut self, edge: Edge) {
        self.core.reset_edge_policy(edge)
    }
}

impl<T> Default for SimContext<T>
where
    T: HasBytesSize,
{
    fn default() -> Self {
        Self::new()
    }
}
