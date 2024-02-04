use crate::Msg;
use std::{collections::BTreeMap, time::SystemTime};
use tokio::time::sleep;

pub struct TimeOrdered<T> {
    map: BTreeMap<SystemTime, Vec<Msg<T>>>,
}

impl<T> TimeOrdered<T> {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
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
        self.map.first_key_value().map(|(time, _entries)| *time)
    }

    pub fn pop(&mut self) -> Option<Msg<T>> {
        let now = SystemTime::now();
        let mut msg = None;

        let mut entry = self.map.first_entry()?;
        if *entry.key() <= now {
            msg = entry.get_mut().pop();
        }
        if entry.get().is_empty() {
            entry.remove();
        }

        msg
    }

    pub fn push(&mut self, time: SystemTime, msg: Msg<T>) {
        self.map.entry(time).or_default().push(msg);
    }

    pub async fn wait_pop(&mut self) -> Option<Msg<T>> {
        let entry = self.map.first_entry()?;

        let time = *entry.key();
        if let Err(error) = time.elapsed() {
            let duration = error.duration();

            sleep(duration).await;
        };

        self.pop()
    }
}

impl<T> Default for TimeOrdered<T> {
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
        let mut c = TimeOrdered::<()>::new();

        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
        assert!(c.pop().is_none());
        assert!(c.wait_pop().await.is_none());
    }

    const SIM_ID: SimId = SimId::new("a sim-id");

    #[tokio::test]
    async fn passed_entry() {
        let mut c = TimeOrdered::<()>::new();

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
        let mut c = TimeOrdered::<()>::new();
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
