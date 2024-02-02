use std::{borrow::Cow, fmt};

/// The identifier of a peer in the SimNetwork
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimId(Cow<'static, str>);

impl SimId {
    pub const fn new(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }
}

impl fmt::Display for SimId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
