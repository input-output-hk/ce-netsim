use crate::SimId;

pub const DEFAULT_BYTES_PER_SEC: u64 = 1_024 * 1_024 * 1_024;

pub(crate) const DEFAULT_MUX_ID: SimId = SimId::new("__network_sim::mux");
