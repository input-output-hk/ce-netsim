use std::{fmt, str};

use anyhow::anyhow;

/// The identifier of a peer in the SimNetwork
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct SimId(u64);

impl SimId {
    pub(crate) const ZERO: Self = SimId::new(0);

    pub(crate) const fn new(id: u64) -> Self {
        Self(id)
    }

    #[must_use = "function does not modify the current value"]
    pub(crate) fn next(self) -> Self {
        Self::new(self.0 + 1)
    }
}

impl str::FromStr for SimId {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self).map_err(|error| anyhow!("{error}"))
    }
}

impl fmt::Display for SimId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Binary for SimId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Octal for SimId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::LowerHex for SimId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::UpperHex for SimId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_binary() {
        assert_eq!(format!("{:b}", SimId(42)), "101010")
    }
    #[test]
    fn print_octal() {
        assert_eq!(format!("{:o}", SimId(42)), "52")
    }
    #[test]
    fn print_lower_hex() {
        assert_eq!(format!("{:x}", SimId(42)), "2a")
    }
    #[test]
    fn print_upper_hex() {
        assert_eq!(format!("{:X}", SimId(42)), "2A")
    }
    #[test]
    fn print() {
        assert_eq!(format!("{}", SimId(42)), "42")
    }
    #[test]
    fn parse() {
        assert_eq!("42".parse::<SimId>().unwrap(), SimId(42));
    }
}
