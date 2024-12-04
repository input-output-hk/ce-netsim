use crate::node::NodeId;

/// Unique identifier of the link between two nodes
///
/// The link is bidirectional and is unique for two node. I.e.
/// For all nodes `n1` and `n2` the identifier `(n1, n2)` is the
/// same as the identifier `(n2, n1)`.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinkId {
    smaller_id: NodeId,
    larger_id: NodeId,
}

impl LinkId {
    /// create the link identifier from the given node tuple.
    ///
    /// ```
    /// # use netsim_core::{link::LinkId, node::NodeId};
    /// # let n1 = NodeId::ZERO;
    /// # let n2 = NodeId::ONE;
    /// LinkId::new((n1, n2))
    /// # ;
    /// ```
    pub fn new((a, b): (NodeId, NodeId)) -> Self {
        if a < b {
            Self {
                smaller_id: a,
                larger_id: b,
            }
        } else {
            Self {
                smaller_id: b,
                larger_id: a,
            }
        }
    }

    /// get the [`NodeId`]s that compose this link identifier
    ///
    /// # Note
    ///
    /// This function will may return the identifier from a different
    /// order than when constructed
    ///
    /// ```
    /// # use netsim_core::{link::LinkId, node::NodeId};
    /// # let n1 = NodeId::ONE;
    /// # let n2 = NodeId::ZERO;
    /// let (n2, n1) =
    ///   // construct a LinkId with (n1, n2)
    ///   LinkId::new((n1, n2))
    ///   // converting it back to a tuple
    ///   .into_nodes();
    /// # assert_eq!(n2, NodeId::ZERO);
    /// # assert_eq!(n1, NodeId::ONE);
    /// ```
    #[inline]
    pub fn into_nodes(self) -> (NodeId, NodeId) {
        (self.smaller_id, self.larger_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn n1n2_eq_n2n1() {
        let n1 = NodeId::ZERO;
        let n2 = NodeId::ONE;

        assert_eq!(
            LinkId::new((n1, n2)),
            // ==
            LinkId::new((n2, n1)),
        );
    }
}
