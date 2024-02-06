pub mod defaults;
mod msg;
mod sim_id;
mod time_queue;

pub use self::{
    msg::{HasBytesSize, Msg, MsgWith},
    sim_id::SimId,
    time_queue::TimeQueue,
};

pub struct SimConfiguration {
    //
}

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
