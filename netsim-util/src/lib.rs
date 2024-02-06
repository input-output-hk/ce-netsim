pub mod defaults;
mod msg;
pub mod sim_context;
mod sim_id;
mod time_queue;
mod msg_policy;

pub use self::{
    msg::{HasBytesSize, Msg, MsgWith},
    sim_id::SimId,
    time_queue::TimeQueue,
    msg_policy::{MsgPolicyResult, MessagePolicy, MessagePolicy::NoDropPolicy, MessagePolicy::DropAllPolicy}
};

pub struct SimConfiguration {
    //
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimSocketConfiguration {
    pub upload_bytes_per_sec: u64,
    pub download_bytes_per_sec: u64,
    pub msg_filter_policy: MessagePolicy
}

impl Default for SimSocketConfiguration {
    fn default() -> Self {
        Self {
            upload_bytes_per_sec: defaults::DEFAULT_BYTES_PER_SEC,
            download_bytes_per_sec: defaults::DEFAULT_BYTES_PER_SEC,
            msg_filter_policy: MessagePolicy::NoDropPolicy,
        }
    }
}
