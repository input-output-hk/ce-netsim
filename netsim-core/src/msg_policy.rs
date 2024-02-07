use std::cmp;
use crate::Msg;
use crate::sim_context::Link;
use std::time::{Duration, SystemTime};


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MessagePolicy {
    DropAllPolicy,
    DefaultPolicy
}

pub(crate) trait MsgPolicy {

    fn recv<T>(&self, msg: T) -> Outcome<T> {
        Outcome::PassThrough(msg)
    }

    fn compute_message_speed<UpLink: Link>(&self, msg: &Msg<UpLink::Msg>) -> Option<u64> {
        // // lock the links so we can query the recipient's link and the sender's link
        // // and get the necessa
        // let locked_links =
        //     links
        //         .lock()
        //         .expect("Under no condition we expect the mutex to be poisoned");
        //
        // // 2. get the upload speed (the sender of the message)
        // let upload_speed = locked_links.get(&msg.from()).map(|link| link.upload_speed())?;
        // // 3. get the download speed (the recipient of the message)
        // let download_speed = locked_links.get(&msg.to()).map(|link| link.download_speed())?;
        // // 4. the message speed is the minimum value between the upload and download
        // Some(cmp::min(upload_speed, download_speed))
        Some(40)
    }
}
impl MsgPolicy for MessagePolicy {
    fn recv<T>(&self, msg: T) -> Outcome<T> {
        match self {
            MessagePolicy::DropAllPolicy => {
                Outcome::Drop(msg)
            }
            MessagePolicy::DefaultPolicy => {
                return Outcome::PassThrough(msg);
                // let Some(speed) = self.compute_message_speed(msg) else {
                //     return Outcome::PassThrough(msg)
                // };
                //
                // let content_size = msg.content().bytes_size();
                // let delay = Duration::from_secs(content_size / speed);
                // // 4. compute the due by time
                // let due_by = msg.time() + delay;
                // Outcome::Throttle { until: due_by, msg }
            }
        }
    }
}


pub enum Outcome<T> {
    Drop(T),  // drop the message altogether
    PassThrough(T), // no drop, pass directly
    // the message will be sent to recipient, but not until the time has elapsed
    Throttle { until: SystemTime, msg: T },
}
