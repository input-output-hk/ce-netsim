mod bus;
mod congestion_queue;
pub mod defaults;
mod geo;
mod msg;
mod policy;
pub mod sim_context;
mod sim_id;
pub mod time;

use std::time::Duration;

use defaults::DEFAULT_IDLE;

pub use self::{
    bus::BusSender,
    msg::{HasBytesSize, Msg},
    policy::{Bandwidth, Edge, EdgePolicy, Latency, NodePolicy, PacketLoss, Policy},
    sim_id::SimId,
};

pub struct OnDrop<T> {
    on_drop: extern "C" fn(T),
}
impl<T> OnDrop<T> {
    pub(crate) fn handle(&self, value: T) {
        (self.on_drop)(value)
    }
}
impl<T> From<extern "C" fn(T)> for OnDrop<T> {
    fn from(value: extern "C" fn(T)) -> Self {
        Self { on_drop: value }
    }
}
unsafe impl<T> Send for OnDrop<T> {}
impl<T> Drop for OnDrop<T> {
    fn drop(&mut self) {
        // Do nothing, this is a pointer that is expected to live all the way
    }
}

pub struct SimConfiguration<T> {
    pub policy: policy::Policy,

    pub on_drop: Option<OnDrop<T>>,

    /// set the maximum IDLE duration time. This is the time the Multiplexer
    /// will wait before checking for pending messages and transition delays.
    ///
    /// By default the value is set to [DEFAULT_IDLE] and this should be
    /// enough for most cases.
    ///
    /// It is possible to change the values for this. However it will affect the
    /// processing overhead of the simulation's multiplexer.
    ///
    /// Reducing it will guarantee the messages are propagated in time with
    /// fine granularity. However it will cost more in CPU activity as the
    /// thread will be busy working for likely empty queues and containers.
    ///
    /// Increasing the value will allow more CPU time for other tasks but will
    /// reduce the precision of the message transition. This will have for
    /// effect to make messages arrive much later than they should have.
    ///
    /// In short, if you are doing a small simulation with a few nodes that
    /// needs to communicate very precisely a lower IDLE time will work fine.
    /// If you are going to simulate hundreds of nodes or more then you might
    /// want to allow a larger IDLE time to allow more CPU time for the
    /// different actors.
    ///
    /// The default settings should allow for hundreds of nodes to work with a
    /// submilliseconds granularity precision on a recent computer.
    pub idle_duration: Duration,
}

impl<T> Default for SimConfiguration<T> {
    fn default() -> Self {
        Self {
            policy: policy::Policy::new(),
            on_drop: None,
            idle_duration: DEFAULT_IDLE,
        }
    }
}
