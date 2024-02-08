pub mod defaults;
mod msg;
mod name_service;
pub mod sim_context;
mod sim_id;
mod time_queue;
mod msg_policy;

pub use self::{
    msg::{HasBytesSize, Msg, MsgWith},
    name_service::NameService,
    sim_id::SimId,
    time_queue::TimeQueue,
};

pub struct SimConfiguration {}


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimSocketConfiguration {
    pub upload_bytes_per_sec: u64,
    pub download_bytes_per_sec: u64,
}

impl Default for SimSocketConfiguration {
    fn default() -> Self {
        Self {
            upload_bytes_per_sec: defaults::DEFAULT_BYTES_PER_SEC,
            download_bytes_per_sec: defaults::DEFAULT_BYTES_PER_SEC,
        }
    }
}
