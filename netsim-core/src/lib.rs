pub mod defaults;
mod msg;
mod name_service;
mod policy;
pub mod sim_context;
mod sim_id;
mod time_queue;

pub use self::{
    msg::{HasBytesSize, Msg, MsgWith},
    name_service::NameService,
    policy::{Bandwidth, Edge, EdgePolicy, Latency, NodePolicy, PacketLoss, Policy},
    sim_id::SimId,
    time_queue::TimeQueue,
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
        // Do nothing, this is a poitner that is expected to live all the way
    }
}

pub struct SimConfiguration<T> {
    pub policy: policy::Policy,

    pub on_drop: Option<OnDrop<T>>,
}

impl<T> Default for SimConfiguration<T> {
    fn default() -> Self {
        Self {
            policy: policy::Policy::new(),
            on_drop: None,
        }
    }
}
