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

#[derive(Debug, Clone, Default)]
pub struct SimConfiguration {
    pub policy: policy::Policy,
}
