/// the iteration round
///
/// This is used to know what round we are at and to allow different
/// threads to update the CongestionChannel capacities of the nodes
/// concurrently without affecting performances too much
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(C)]
pub struct Round(u64);

impl Round {
    pub const ZERO: Self = Round(0);

    /// get a new [`Round`].
    ///
    /// ```
    /// # use netsim_core::network::Round;
    /// # let _round =
    /// Round::new()
    /// # ;
    /// ```
    pub const fn new() -> Self {
        Self::ZERO
    }

    /// get the next round.
    ///
    /// # Example
    ///
    /// ```
    /// # use netsim_core::network::Round;
    /// # let prev = Round::new();
    /// let next = prev.next();
    /// # assert_ne!(prev, next);
    /// assert!(prev < next);
    /// ```
    ///
    /// # Wrap-around
    ///
    /// After `u64::MAX` advances the counter wraps to [`Round::ZERO`].
    /// This is not reachable in practice — at one advance per nanosecond
    /// it would take ~584 years — but if it did occur,
    /// [`CongestionChannel::update_capacity`] would treat the wrapped
    /// round as "not newer" and skip the capacity reset for one step.
    /// No data is lost or corrupted; the channel simply keeps the
    /// previous step's remaining capacity for that single round.
    ///
    /// [`CongestionChannel::update_capacity`]: crate::measure::CongestionChannel::update_capacity
    #[inline(always)]
    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    #[inline(always)]
    pub fn into_u64(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_wrap_around_on_overflow() {
        let round = Round(u64::MAX).next();
        assert_eq!(round, Round::ZERO);
    }
}
