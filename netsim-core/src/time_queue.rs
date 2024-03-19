use crate::msg::MsgWith;
use crate::Msg;
use core::cmp::Reverse;
use std::{collections::BinaryHeap, time::Instant};

pub struct TimeQueue<T> {
    map: BinaryHeap<Reverse<OrderedByTime<T>>>,
}

struct OrderedByTime<T>(MsgWith<T>);

impl<T> OrderedByTime<T> {
    pub fn inner(&self) -> &MsgWith<T> {
        &self.0
    }

    pub fn into_inner(self) -> MsgWith<T> {
        self.0
    }
}

impl<T> PartialEq for OrderedByTime<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.reception_time == other.0.reception_time
    }
}

impl<T> Eq for OrderedByTime<T> {}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl<T> PartialOrd for OrderedByTime<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.reception_time.partial_cmp(&other.0.reception_time)
    }
}
impl<T> Ord for OrderedByTime<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.reception_time.cmp(&other.0.reception_time)
    }
}

impl<T> TimeQueue<T> {
    pub fn new() -> Self {
        Self {
            map: BinaryHeap::new(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[inline]
    pub fn time_to_next_msg(&self) -> Option<Instant> {
        self.map.peek().map(|v| v.0.inner().reception_time)
    }

    pub fn pop(&mut self) -> Option<Msg<T>> {
        self.map.pop().map(|v| v.0.into_inner().msg)
    }

    pub fn pop_all_elapsed(&mut self, time: Instant) -> Vec<Msg<T>> {
        let mut msgs = Vec::new();
        loop {
            match self.map.peek() {
                None => break,
                Some(msg) => {
                    if msg.0.inner().reception_time <= time {
                        let msg = self
                            .pop()
                            .expect("We just peeked the map, so a pop should always work");
                        msgs.push(msg)
                    } else {
                        break;
                    }
                }
            }
        }
        msgs
    }

    pub fn push(&mut self, time: Instant, msg: Msg<T>) {
        let m = MsgWith {
            reception_time: time,
            msg,
        };
        self.map.push(Reverse(OrderedByTime(m)))
    }
}

impl<T> Default for TimeQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimId;
    use std::time::Duration;

    #[test]
    fn empty() {
        let mut c = TimeQueue::<()>::new();

        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
        assert!(c.pop().is_none());
        assert!(c.time_to_next_msg().is_none());
    }

    const SIM_ID: SimId = SimId::new(0);
    const DURATION: Duration = Duration::from_millis(1);

    #[test]
    fn entry() {
        let mut c = TimeQueue::<()>::new();
        let entry_sent_time = Instant::now();
        let entry_due_time = entry_sent_time + DURATION;
        let current_time = Instant::now() + 2 * DURATION;

        c.push(entry_due_time, Msg::new(SIM_ID, SIM_ID, ()));

        assert!(!c.is_empty());
        assert_eq!(c.len(), 1);
        let due_time = c
            .time_to_next_msg()
            .expect("There should be at least one object in the queue");
        assert_eq!(due_time, entry_due_time);

        assert!(c.pop_all_elapsed(entry_sent_time).is_empty());
        let entries = c.pop_all_elapsed(current_time);
        assert_eq!(entries.len(), 1);

        assert!(c.is_empty());
    }
}
