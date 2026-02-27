use anyhow::anyhow;
use std::{fmt, str};

/// Unique identifier for a node in the simulated network.
///
/// `NodeId` is a lightweight, copy-able, comparable wrapper around a `u64`.
/// Every node created with [`Network::new_node`] receives a unique `NodeId`
/// that is stable for the lifetime of the `Network`. Use it to address packets
/// and to configure links between nodes.
///
/// ## Sentinels
///
/// Two sentinel values are provided for use in tests and as default/null
/// placeholders:
///
/// - [`NodeId::ZERO`] — represents "no node" or an uninitialised ID. The
///   [`Network`] never assigns this ID to a real node.
/// - [`NodeId::ONE`] — the first ID that [`Network::new_node`] will assign.
///
/// ## Display and parsing
///
/// `NodeId` implements [`Display`](fmt::Display), [`Debug`], and
/// [`FromStr`](str::FromStr) so it can be used in log messages and
/// round-tripped through text configs:
///
/// ```
/// use netsim_core::NodeId;
///
/// let id: NodeId = "42".parse().unwrap();
/// assert_eq!(id.to_string(), "42");
/// ```
///
/// It also implements [`Binary`](fmt::Binary), [`Octal`](fmt::Octal),
/// [`LowerHex`](fmt::LowerHex), and [`UpperHex`](fmt::UpperHex) for
/// debugging purposes:
///
/// ```
/// use netsim_core::NodeId;
///
/// let id: NodeId = "255".parse().unwrap();
/// assert_eq!(format!("{id:x}"), "ff");
/// assert_eq!(format!("{id:b}"), "11111111");
/// ```
///
/// [`Network`]: crate::network::Network
/// [`Network::new_node`]: crate::network::Network::new_node
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct NodeId(u64);

impl NodeId {
    /// Sentinel value meaning "no node" or an uninitialised identifier.
    ///
    /// [`Network::new_node`] starts assigning IDs from `1`, so `ZERO` is
    /// never returned by the network. It is useful as a "null" placeholder
    /// in tests and data structures that need a default `NodeId`.
    ///
    /// [`Network::new_node`]: crate::network::Network::new_node
    pub const ZERO: Self = NodeId::new(0);

    /// The first real node identifier assigned by [`Network::new_node`].
    ///
    /// Subsequent calls to `new_node` produce `2`, `3`, etc. `ONE` is
    /// provided as a convenient sentinel for tests that hardcode two nodes
    /// without constructing a full `Network`.
    ///
    /// [`Network::new_node`]: crate::network::Network::new_node
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
