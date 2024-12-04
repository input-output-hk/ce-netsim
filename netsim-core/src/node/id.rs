use anyhow::anyhow;
use std::{fmt, str};

/// The identifier of a peer in the SimNetwork
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct NodeId(u64);

impl NodeId {
    pub const ZERO: Self = NodeId::new(0);
    pub const ONE: Self = NodeId::new(1);

    pub(crate) const fn new(id: u64) -> Self {
        Self(id)
    }

    #[must_use = "function does not modify the current value"]
    pub(crate) fn next(self) -> Self {
        Self::new(self.0 + 1)
    }
}

impl str::FromStr for NodeId {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self).map_err(|error| anyhow!("{error}"))
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Binary for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Octal for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::LowerHex for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::UpperHex for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_binary() {
        assert_eq!(format!("{:b}", NodeId(42)), "101010")
    }
    #[test]
    fn print_octal() {
        assert_eq!(format!("{:o}", NodeId(42)), "52")
    }
    #[test]
    fn print_lower_hex() {
        assert_eq!(format!("{:x}", NodeId(42)), "2a")
    }
    #[test]
    fn print_upper_hex() {
        assert_eq!(format!("{:X}", NodeId(42)), "2A")
    }
    #[test]
    fn print() {
        assert_eq!(format!("{}", NodeId(42)), "42")
    }
    #[test]
    fn parse() {
        assert_eq!("42".parse::<NodeId>().unwrap(), NodeId(42));
    }
}
