use std::sync::atomic::AtomicBool;

#[derive(Debug)]
pub(crate) struct Stop(AtomicBool);

/// use total ordering for the atomic operations to prevent
/// the operations to be reordered by the compiler or the CPU.
///
/// We might want to comeback to it and rethink how they are
/// being ordered and how we can allow more room for the compiler
/// and the CPU to optimise things. In the meantime this is
/// maybe a bit more costly but it is certainly marginal
/// for our current state.
///
/// the best approach might actually to use specific Ordering
/// for each operations.
const ORDERING: std::sync::atomic::Ordering = std::sync::atomic::Ordering::SeqCst;

/// The Atomic ordering used for `load` like operations
const FETCH_ORDERING: std::sync::atomic::Ordering = ORDERING;

/// The Atomic odering used for `store` like operations
const STORE_ORDERING: std::sync::atomic::Ordering = ORDERING;

impl Stop {
    pub(crate) fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    #[inline]
    pub(crate) fn get(&self) -> bool {
        self.0.load(FETCH_ORDERING)
    }

    /// set the stop signal
    #[inline]
    pub(crate) fn toggle(&self) {
        self.0.store(true, STORE_ORDERING)
    }
}

impl Default for Stop {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // allow bool assert comparison because we want to highlight
    // what we are actually expecting to have
    #[allow(clippy::bool_assert_comparison)]
    #[test]
    fn default() {
        assert_eq!(
            // the default constructor should be initialised to false
            Stop::new().get(),
            false
        );

        assert_eq!(
            // the default constructor should be initialised to false
            Stop::default().get(),
            false
        );
    }

    // allow bool assert comparison because we want to highlight
    // what we are actually expecting to have
    #[allow(clippy::bool_assert_comparison)]
    #[test]
    fn toggle() {
        let stop = Stop::new();

        assert_eq!(stop.get(), false);
        stop.toggle();
        assert_eq!(stop.get(), true);
        stop.toggle();
        assert_eq!(stop.get(), true);
    }
}
