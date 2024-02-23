use crate::{
    sim_link::{link, SimUpLink},
    SimConfiguration, SimSocket,
};
use anyhow::{Context as _, Result};
use netsim_core::{sim_context::SimContextCore, Edge, EdgePolicy, HasBytesSize, NodePolicy, SimId};

/// This is the execution context/controller of a simulated network
///
/// It is possible to have multiple [SimContext] opened concurrently
/// in the same process. Howver the nodes of a given context
/// will not be able to send messages to nodes of different context.
///
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
    /// This function use the default [`SimConfiguration`].
    /// Use [`SimContext::with_config`] to start a [`SimContext`] with specific
    /// configurations.
    /// [`NodePolicy`] and [`EdgePolicy`] may still be set dynamically while the
    /// simulation is running.
    ///
    /// Note that this function starts a _multiplexer_ in a physical thread.
    pub fn new() -> Self {
        let configuration = SimConfiguration::default();

        Self::with_config(configuration)
    }

    /// create a new [`SimContext`]. Creating this object will also start a
    /// multiplexer in the background. Make sure to call [`SimContext::shutdown`]
    /// for a clean shutdown of the background process.
    ///
    /// Note that this function starts a _multiplexer_ in a physical thread.
    ///
    pub fn with_config(configuration: SimConfiguration<T>) -> Self {
        let sim_context_core = SimContextCore::new(configuration);

        Self {
            core: sim_context_core,
        }
    }

    /// set a specific policy between the two `Node` that compose the [`Edge`].
    ///
    /// when no specific policies are set, the default policies are used.
    /// To reset, use [`SimContext::reset_edge_policy`], and the default
    /// policy will be used again.
    ///
    pub fn set_edge_policy(&mut self, edge: Edge, policy: EdgePolicy) {
        self.core.set_edge_policy(edge, policy)
    }

    /// Reset the [`EdgePolicy`] between two nodes of an [`Edge`]. The default
    /// EdgePolicy for this SimContext will be used.
    ///
    pub fn reset_edge_policy(&mut self, edge: Edge) {
        self.core.reset_edge_policy(edge)
    }

    /// Set a specific [`NodePolicy`] for a given node ([SimSocket]).
    ///
    /// If not set, the default [NodePolicy] for the [SimContext] will be
    /// used instead.
    ///
    /// Call [`SimContext::reset_node_policy`] to reset the [`NodePolicy`]
    /// so that the default policy will be used onward.
    pub fn set_node_policy(&mut self, node: SimId, policy: NodePolicy) {
        self.core.set_node_policy(node, policy)
    }

    /// Reset the specific [`NodePolicy`] associated to the given node
    /// ([SimSocket]) so that the default policy will be used again going
    /// forward.
    pub fn reset_node_policy(&mut self, node: SimId) {
        self.core.reset_node_policy(node)
    }

    /// Open a new [`SimSocket`] within the given context
    ///
    pub fn open(&mut self) -> Result<SimSocket<T>> {
        let (up, down) = link();

        let address = self
            .core
            .new_link(up)
            .context("Failed to reserve a new SimId")?;

        Ok(SimSocket::new(address, self.core.bus(), down))
    }

    /// Shutdown the context. All remaining opened [SimSocket] will become
    /// non functional and will return a `Disconnected` error when trying
    /// to receive messages or when trying to send messages
    ///
    /// This function is blocking and will block until the multiplexer
    /// thread has shutdown.
    ///
    pub fn shutdown(self) -> Result<()> {
        self.core.shutdown()
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
