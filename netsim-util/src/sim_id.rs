use std::fmt;

/// The identifier of a peer in the SimNetwork
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SimId(u64);

impl SimId {
    pub(crate) const fn new(id: u64) -> Self {
        Self(id)
    }

    #[must_use = "function does not modify the current value"]
    pub(crate) fn next(self) -> Self {
        Self::new(self.0 + 1)
    }
}

impl fmt::Display for SimId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
