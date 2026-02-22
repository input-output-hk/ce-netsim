use std::sync::atomic::AtomicU64;

/// [`Gauge`] use gauge to keep track of current usage of
/// a given something
///
/// The [`Gauge`] isn't clonable but it can be safely be used.
/// across different threads as it is [`Sync`] and [`Send`] safe.
///
/// The [`Default`] capacity for the [`Gauge`] is to come with an
/// unlimited capacity. However to have a more _realistic_ experience
/// of the simulated network it is preferable to set an appropriate
/// value.
///
/// # Thread Safety
///
/// Gauge are thread safe. They use [`AtomicU64`] to account for the
/// maximum and or used capacity. However the functions are not atomic
/// they implements measures to mitigate concurrency issues with using
/// functions like [`AtomicU64::compare_exchange_weak`] and repeatedly
/// attempt the update the values until we have the expected outcome.
///
/// See [`Gauge::reserve`] and [`Gauge::free`] for more details.
///
/// A concurrency issue we know we do not handle is the [ABA Problem].
/// **However** this not an issue for our use case as we do not mind
/// if the value has been updated and then returned to the expected
/// value while we are trying to update it ourselves.
///
/// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
#[derive(Debug)]
pub struct Gauge {
    maximum_capacity: AtomicU64,
    used_capacity: AtomicU64,
}

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

impl Gauge {
    /// create a [`Gauge`] with a infinite maximum capacity.
    ///
    /// # Example
    ///
    /// Creating a new [`Gauge`] with the default maximum capacity.
    /// This function is equivalent to calling [`Gauge::with_capacity`].
    ///
    /// ```
    /// # use netsim_core::measure::Gauge;
    /// let gauge = Gauge::new();
    /// # assert_eq!(gauge.maximum_capacity(), u64::MAX);
    /// // is equivalent to calling:
    /// let gauge = Gauge::with_capacity(u64::MAX);
    /// # assert_eq!(gauge.maximum_capacity(), u64::MAX);
    /// ```
    ///
    pub fn new() -> Self {
        Self::with_capacity(u64::MAX)
    }

    /// create a [`Gauge`] with the given maximum capacity as a starting point.
    ///
    /// # Example
    ///
    /// ```
    /// # use netsim_core::measure::Gauge;
    /// let gauge = Gauge::with_capacity(1_024);
    /// # assert_eq!(gauge.maximum_capacity(), 1_024);
    /// ```
    pub fn with_capacity(maximum_capacity: u64) -> Self {
        Self {
            maximum_capacity: AtomicU64::new(maximum_capacity),
            used_capacity: AtomicU64::new(0),
        }
    }

    /// get the maximum capacity of the [`Gauge`].
    ///
    /// # Example
    ///
    /// ```
    /// # use netsim_core::measure::Gauge;
    /// let gauge = Gauge::with_capacity(1_024);
    ///
    /// assert_eq!(
    ///     gauge.maximum_capacity(),
    ///     1_024,
    /// );
    /// ```
    #[inline]
    pub fn maximum_capacity(&self) -> u64 {
        self.maximum_capacity.load(FETCH_ORDERING)
    }

    /// update the maximum capacity of the [`Gauge`]
    #[inline]
    pub fn set_maximum_capacity(&self, new: u64) {
        self.maximum_capacity.store(new, STORE_ORDERING)
    }

    /// get the currently used capacity of the [`Gauge`].
    ///
    /// # Example
    ///
    /// ```
    /// # use netsim_core::measure::Gauge;
    /// let gauge = Gauge::with_capacity(1_024);
    ///
    /// assert_eq!(
    ///     gauge.used_capacity(),
    ///     0,
    /// );
    /// ```
    #[inline]
    pub fn used_capacity(&self) -> u64 {
        self.used_capacity.load(FETCH_ORDERING)
    }

    /// get the remaining capacity of the [`Gauge`].
    ///
    /// # Example
    ///
    /// ```
    /// # use netsim_core::measure::Gauge;
    /// let gauge = Gauge::with_capacity(1_024);
    ///
    /// assert_eq!(
    ///     gauge.remaining_capacity(),
    ///     1_024,
    /// );
    /// ```
    #[inline]
    pub fn remaining_capacity(&self) -> u64 {
        self.maximum_capacity().saturating_sub(self.used_capacity())
    }

    /// function to reserve a capacity in the gauge. The function will
    /// try to reserve _up to_ the given `size`. This function returns
    /// the amount actually reseved.
    ///
    /// we know this function does not support the ABA problem but that
    /// is fine. We aren't trying to get an exact sense of what the value
    /// was and there's no need to order precisely how the gauge is updated
    ///
    /// what we want to know is that when we have finished reserving a certain
    /// amount of data then this data is properly added to the used capacity.
    ///
    /// # Example
    ///
    /// ```
    /// # use netsim_core::measure::Gauge;
    /// let gauge = Gauge::with_capacity(1_024);
    /// # let actual =
    /// gauge.reserve(1_000);
    /// # assert_eq!(actual, 1_000);
    /// let actual = gauge.reserve(30);
    /// assert_eq!(actual, 24);
    /// ```
    ///
    /// # Known issue and mitigation
    ///
    /// This function is not atomic!
    ///
    /// There's a known issue with that this function uses [`Self::remaining_capacity`]
    /// in order to know how much data can be used.
    ///
    /// While we are taking a mitigation in order to make sure that when we
    /// update the [`Self::used_capacity`]  we are updating it from a point
    /// we knew about there's a non atomic delay operation between the time
    /// we sample the [`Self::remaining_capacity`] and the time we actually
    /// exchange the new capacity. While we are protecting ourselves with
    /// an atomic [`AtomicU64::compare_exchange_weak`] call to update the
    /// [`Self::used_capacity`] this isn't preventing the [`Self::maximum_capacity`]
    /// to have been modified before the new capacity is called.
    ///
    /// We accept that this is an issue but with little consequence for our
    /// case. The [`Self::maximum_capacity`] function is not meant to be
    /// modified often and we accept that modifying this value will
    /// marginally taint our simulations.
    ///
    pub fn reserve(&self, size: u64) -> u64 {
        // get the currently known capacity
        let mut prev = self.used_capacity();

        loop {
            // this is the potentially problematic place as we are loading
            // the `maximum_capacity`.
            //
            // Now the `used_capacity` is also stored in the `prev` and we are
            // going to compare it against the current value in when we do
            // the `compare_exchange_weak`. However we aren't comparing the
            // `maximum_capacity` hasn't been udpated in between.
            //
            // we accept that this is not going to be 100% accurate in the edge
            // case where a thread modifies the `maximum_capacity` while we are
            // reserving data on the gauge.
            let maximum_capacity = self.maximum_capacity();
            let remaining_capacity = maximum_capacity.saturating_sub(prev);

            let actual_size = std::cmp::min(remaining_capacity, size);
            let next = prev.saturating_add(actual_size);

            match self.used_capacity.compare_exchange_weak(
                prev,
                next,
                STORE_ORDERING,
                FETCH_ORDERING,
            ) {
                Ok(_) => return actual_size,
                Err(next_prev) => prev = next_prev,
            }
        }
    }

    /// Attempts to free up to `size` from the gauge.
    ///
    /// Unlike [`Self::reserve`] this function is atomic safe though
    /// it still have the draw back of not being _ABA safe_ (but in our
    /// case this is still not a problem because we want to only account
    /// for the quantity used not the ordering it is updated).
    ///
    /// # Example
    ///
    /// ```
    /// # use netsim_core::measure::Gauge;
    /// let gauge = Gauge::with_capacity(1_024);
    /// # let actual =
    /// gauge.reserve(100);
    /// # assert_eq!(actual, 100);
    /// let actual = gauge.free(90);
    /// assert_eq!(actual, 90);
    /// let actual = gauge.free(20);
    /// assert_eq!(actual, 10);
    /// let actual = gauge.free(0);
    /// assert_eq!(actual, 0);
    /// ```
    ///
    /// # Known issue and mitigation
    ///
    /// this function is not atomic. However we are applying the changes
    /// in such a way that we cannot fall in an invalid state. Thanks to
    /// using [`AtomicU64::compare_exchange_weak`] which make sure we are
    /// only updating the used capacity if the previous state is the same
    /// as we expected when we computed what the next `used_capacity` should
    /// be.
    ///
    pub fn free(&self, size: u64) -> u64 {
        // get the currently known capacity
        let mut prev = self.used_capacity();

        loop {
            let actual_size = std::cmp::min(prev, size);
            let next = prev.saturating_sub(actual_size);

            match self.used_capacity.compare_exchange_weak(
                prev,
                next,
                STORE_ORDERING,
                FETCH_ORDERING,
            ) {
                Ok(_) => return actual_size,
                Err(next_prev) => prev = next_prev,
            }
        }
    }
}

impl Default for Gauge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// this test check that the boundary of the gauge is
    /// properly respected in the ideal situation.
    ///
    /// This test doesn't handled the case where the `maximum_capacity`
    /// is updated concurrently as we are `reserving` more space.
    #[test]
    fn upper_bound() {
        let gauge = Gauge::with_capacity(10);

        let reserved = gauge.reserve(0);
        assert_eq!(reserved, 0);

        let reserved = gauge.reserve(10);
        assert_eq!(reserved, 10);

        let reserved = gauge.reserve(10);
        assert_eq!(reserved, 0);
    }

    /// this function test the lower bound (i.e. we cannot free more than the
    /// `used_capacity`).
    ///
    #[test]
    fn lower_bound() {
        let gauge = Gauge::new();

        let freed = gauge.free(10);
        assert_eq!(freed, 0);

        gauge.reserve(100);
        let freed = gauge.free(90);
        assert_eq!(freed, 90);

        let freed = gauge.free(0);
        assert_eq!(freed, 0);

        let freed = gauge.free(20);
        assert_eq!(freed, 10);

        let freed = gauge.free(20);
        assert_eq!(freed, 0);

        let freed = gauge.free(0);
        assert_eq!(freed, 0);
    }

    #[test]
    fn zero_capacity_gauge_reserves_nothing() {
        let gauge = Gauge::with_capacity(0);
        assert_eq!(gauge.reserve(1), 0);
        assert_eq!(gauge.reserve(1_000), 0);
        assert_eq!(gauge.used_capacity(), 0);
    }

    #[test]
    fn set_maximum_capacity_limits_future_reserves() {
        let gauge = Gauge::new(); // u64::MAX capacity
        gauge.reserve(500);
        assert_eq!(gauge.used_capacity(), 500);

        // Shrink max to 600; 500 already used → only 100 more reservable
        gauge.set_maximum_capacity(600);
        let reserved = gauge.reserve(200);
        assert_eq!(reserved, 100);
        assert_eq!(gauge.used_capacity(), 600);
    }

    #[test]
    fn free_more_than_used_caps_at_zero() {
        let gauge = Gauge::with_capacity(100);
        gauge.reserve(30);
        // Free more than reserved — should only free what's used
        let freed = gauge.free(1_000);
        assert_eq!(freed, 30);
        assert_eq!(gauge.used_capacity(), 0);
    }

    #[test]
    fn reserve_and_free_zero_are_noops() {
        let gauge = Gauge::with_capacity(100);
        gauge.reserve(50);

        assert_eq!(gauge.reserve(0), 0);
        assert_eq!(gauge.free(0), 0);
        assert_eq!(gauge.used_capacity(), 50); // unchanged
    }
}
