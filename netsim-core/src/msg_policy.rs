
use std::time::{Duration, SystemTime};
use std::collections::HashMap;
use std::sync::Mutex;
use rand::prelude::*;
use crate::{HasBytesSize, Msg, SimId};

struct MessageDropPolicy {
   //count dropped messages
   dropped_count: Mutex<u64>,
   probability_of_drop: f64,
}

impl MessageDropPolicy {

    pub fn new(probability_of_drop: f64) -> Self {
        Self {
            dropped_count: Mutex::new(0),
            probability_of_drop
        }
    }
    fn randomly_drop(&self, probability: f64) -> bool {
        let mut rng = thread_rng();
        let random_number: f64 = rng.gen(); // Generate a random number between 0.0 and 1.0
        random_number < probability
    }

    fn should_drop(&self) -> bool {
        self.randomly_drop(self.probability_of_drop)
    }

    fn drop_message<T>(&self, msg: T, from: SimId, to: SimId) -> bool {

        if self.should_drop() {
            let mut dropped_count = self.dropped_count.lock().unwrap();
            *dropped_count += 1;
            true
        } else {
            false
        }

    }
}
pub(crate) struct MsgPolicy {
    link_speeds: HashMap<(SimId, SimId), u64>,
    message_drops: MessageDropPolicy,
}

impl MsgPolicy {

    fn recv<T: HasBytesSize>(&self, msg: &Msg<T>, from: SimId, to: SimId) -> Outcome {
        if self.message_drops.drop_message(msg, from, to) {
            Outcome::Drop
        } else {
            match self.compute_message_speed(from, to) {
                None => {
                    Outcome::PassThrough
                }
                Some(speed) => {
                    let content_size = msg.content().bytes_size();
                    let delay = Duration::from_secs(content_size / speed);
                    let due_by = msg.time() + delay;
                    Outcome::Throttle { until: due_by }
                }
            }
        }
    }


    fn compute_message_speed(&self, from: SimId, to: SimId) -> Option<u64> {
        self.link_speeds.get(&(from, to)).cloned()
    }
}


pub enum Outcome {
    Drop,  // drop the message altogether
    PassThrough, // no drop, pass directly
    // the message will be sent to recipient, but not until the time has elapsed
    Throttle { until: SystemTime },
}
