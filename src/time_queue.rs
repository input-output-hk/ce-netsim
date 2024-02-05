use crate::msg::{MsgWith, OrderedByTime};
use crate::Msg;
use std::{collections::BinaryHeap, time::SystemTime};
use tokio::time::sleep;

pub struct TimeQueue<T> {
    map: BinaryHeap<OrderedByTime<T>>,
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
    pub fn time_to_next_msg(&self) -> Option<SystemTime> {
        self.map.peek().map(|v| v.inner().reception_time)
    }

    pub fn pop(&mut self) -> Option<Msg<T>> {
        self.map.pop().map(|v| v.into_inner().msg)
    }

    pub fn push(&mut self, time: SystemTime, msg: Msg<T>) {
        let m = MsgWith {
            reception_time: time,
            msg: msg,
        };
        self.map.push(OrderedByTime(m))
    }

    pub async fn wait_pop(&mut self) -> Option<Msg<T>> {
        let entry = self.map.peek()?;
        let now = SystemTime::now();
        let diff = entry.inner().reception_time.duration_since(now);
        if let Ok(duration) = diff {
            sleep(duration).await;
        };
        self.pop()
    }
}

impl<T> Default for TimeQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::Instant;

    use crate::SimId;

    use super::*;

    #[tokio::test]
    async fn empty() {
        let mut c = TimeQueue::<()>::new();

        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
        assert!(c.pop().is_none());
        assert!(c.wait_pop().await.is_none());
    }

    const SIM_ID: SimId = SimId::new("a sim-id");

    #[tokio::test]
    async fn passed_entry() {
        let mut c = TimeQueue::<()>::new();

        c.push(
            SystemTime::now() - Duration::from_secs(1),
            Msg::new(SIM_ID, SIM_ID, ()),
        );

        assert!(!c.is_empty());
        assert_eq!(c.len(), 1);

        let instant = Instant::now();
        let Some(_) = c.wait_pop().await else {
            panic!("The msg should be returned")
        };
        assert!(instant.elapsed().as_millis() < 5);

        assert!(c.is_empty());
    }

    #[tokio::test]
    async fn future_entry() {
        let mut c = TimeQueue::<()>::new();
        const DURATION: Duration = Duration::from_millis(500);

        c.push(SystemTime::now() + DURATION, Msg::new(SIM_ID, SIM_ID, ()));

        assert!(!c.is_empty());
        assert_eq!(c.len(), 1);

        let instant = Instant::now();
        let Some(_) = c.wait_pop().await else {
            panic!("The msg should be returned")
        };
        assert!(instant.elapsed().as_millis() > (DURATION.as_millis() - 50));

        assert!(c.is_empty());
    }
}
