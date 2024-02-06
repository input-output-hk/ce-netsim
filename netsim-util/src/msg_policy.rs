use crate::Msg;
use crate::sim_context::Link;

pub enum MsgPolicyResult {
    Drop,
    NoDrop
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MessagePolicy {
    DropAllPolicy,
    NoDropPolicy
}

pub(crate) trait MsgPolicy {
    fn maybe_drop<T>(&self, msg: &Msg<T>) -> MsgPolicyResult {
        MsgPolicyResult::Drop
    }
}
impl MsgPolicy for MessagePolicy {
    fn maybe_drop<T>(&self, msg: &Msg<T>) -> MsgPolicyResult {
        match self {
            MessagePolicy::NoDropPolicy => {
                // Implement behavior for NoDropPolicy
                MsgPolicyResult::NoDrop
            }
            MessagePolicy::DropAllPolicy => {
                // Implement behavior for DropAllPolicy
                MsgPolicyResult::Drop
            }
            // Handle other policies if needed
        }
    }
}
