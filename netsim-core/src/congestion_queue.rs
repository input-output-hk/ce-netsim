use std::{
    cmp,
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use crate::{sim_context::SimLinks, Bandwidth, Edge, HasBytesSize, Msg, Policy, SimId};

/// used to keep track of how much of a packet has been sent through
/// one of the network components (sender, link and receiver).
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
struct BufferCounter {
    counter: u64,
    since: Instant,
}

/// envelop the message [`Msg`] with additional data
/// that we will use to track the message's journey
/// through the simulated network
pub struct Envelop<T> {
    msg: Msg<T>,

    // the latency on the packet's journey between the sender and the receiver
    // (through the link).
    latency: Instant,

    sender: u64,
    link: u64,
    receiver: u64,
}

#[derive(Debug)]
struct Usage {
    upload: BufferCounter,
    download: BufferCounter,
}

pub struct CongestionQueue<T> {
    // TODO: the only utilisation we have are two fold:
    //
    // 1. we append at the end only
    // 2. when we iterate through it we remove decide to remove the
    //    entry or not.
    //
    // we can replace the VecDeque by a linked list and remove the
    // entry as we go a long. All we need is to keep a pointer
    // to the head and a weak pointer to the tail (so that we can)
    // safely happen in O(1).
    //
    queue: VecDeque<Envelop<T>>,

    nodes_usage: HashMap<SimId, Usage>,
    edge_usage: HashMap<Edge, Usage>,
}

impl BufferCounter {
    fn new(time: Instant) -> Self {
        Self {
            counter: 0,
            since: time,
        }
    }

    fn refresh(&mut self, time: Instant) {
        debug_assert!(self.since <= time);

        // because we assume the time is only moving forward (sic)
        // we can safely assume that the upload_since will always be
        // lesser or equal to `time` given in parameter
        let upload_elased = time.duration_since(self.since);
        if upload_elased >= Duration::from_secs(1) {
            self.counter = 0
        }
    }

    /// try to consume up to `size` bytes from the buffer
    ///
    /// return the number of bytes actually consummed
    pub fn consume(&mut self, time: Instant, bw: Bandwidth, size: u64) -> u64 {
        // compute the remaining available data bandwidth
        let remaining = bw.into_inner().saturating_sub(self.counter);

        let usage = cmp::min(remaining, size);

        self.since = time;
        self.counter = self.counter.saturating_add(usage);

        usage
    }
}

impl Usage {
    fn new(time: Instant) -> Self {
        Self {
            upload: BufferCounter::new(time),
            download: BufferCounter::new(time),
        }
    }

    /// this will check that the buffer counters have been
    /// properly reset to 0 if the time elapsed is greater
    /// than one second.
    fn refresh(&mut self, time: Instant) {
        self.upload.refresh(time);
        self.download.refresh(time);
    }
}

impl<T> Envelop<T>
where
    T: HasBytesSize,
{
    pub fn new(min_time: Instant, msg: Msg<T>) -> Self {
        Self {
            msg,
            latency: min_time,
            sender: 0,
            link: 0,
            receiver: 0,
        }
    }
}

impl<T> CongestionQueue<T>
where
    T: HasBytesSize,
{
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            nodes_usage: HashMap::new(),
            edge_usage: HashMap::new(),
        }
    }

    pub fn push(&mut self, min_time: Instant, msg: Msg<T>) {
        let envelop = Envelop::new(min_time, msg);
        self.queue.push_back(envelop)
    }

    fn pop<UpLink>(
        &mut self,
        time: Instant,
        nodes: &SimLinks<UpLink>,
        policy: &Policy,
        index: usize,
    ) -> Option<Msg<T>> {
        let envelop = self.queue.get_mut(index)?;
        if envelop.latency > time {
            // we ignore messages that are still meant to be delayed
            // by the operation of the latency
            return None;
        }

        let message_size = envelop.msg.content().bytes_size();

        // compute the sender's remaining buffer size
        let s = self
            .nodes_usage
            .entry(envelop.msg.from())
            .and_modify(|u| u.refresh(time))
            .or_insert_with(|| Usage::new(time));
        let s_policy = nodes[envelop.msg.from().into_index()]
            .policy()
            .unwrap_or_else(|| policy.default_node_policy());
        let remaining_size = message_size - envelop.sender;
        let used = s
            .upload
            .consume(time, s_policy.bandwidth_up, remaining_size);
        envelop.sender += used;

        let edge = Edge::new((envelop.msg.from(), envelop.msg.to()));
        let l = self
            .edge_usage
            .entry(edge)
            .and_modify(|u| u.refresh(time))
            .or_insert_with(|| Usage::new(time));
        let l_policy = policy
            .get_edge_policy(edge)
            .unwrap_or_else(|| policy.default_edge_policy());
        let remaining_size = envelop.sender - envelop.link;
        let used = l
            .upload
            .consume(time, l_policy.bandwidth_up, remaining_size);
        envelop.link += used;

        let r = self
            .nodes_usage
            .entry(envelop.msg.to())
            .and_modify(|u| u.refresh(time))
            .or_insert_with(|| Usage::new(time));
        let r_policy = nodes[envelop.msg.to().into_index()]
            .policy()
            .unwrap_or_else(|| policy.default_node_policy());
        let remaining_size = envelop.link - envelop.receiver;
        let used = r
            .download
            .consume(time, r_policy.bandwidth_down, remaining_size);
        envelop.receiver += used;

        // at all time `size >= sender >= link >= receiver`
        debug_assert!(message_size >= envelop.sender);
        debug_assert!(envelop.sender >= envelop.link);
        debug_assert!(envelop.link >= envelop.receiver);

        if message_size == envelop.receiver {
            let entry = self.queue.remove(index)?.msg;
            Some(entry)
        } else {
            None
        }
    }

    pub fn pop_many<UpLink>(
        &mut self,
        time: Instant,
        nodes: &SimLinks<UpLink>,
        policy: &Policy,
    ) -> Vec<Msg<T>> {
        let mut msgs = Vec::new();

        let mut index = 0usize;
        // we aren't using a for loop here because the sequence `0..queue.len()`
        // is larger or equal to the the actual len we will be exploring
        //
        // indeed, for every loop we will be removing an entry at a given index
        // which means that when we remove an entry we won't increase the `index`
        // but the size is still reduced because the queue has an entry less
        while index < self.queue.len() {
            if let Some(entry) = self.pop(time, nodes, policy, index) {
                msgs.push(entry);
            } else {
                index += 1;
            }
        }

        msgs
    }
}

impl<T: HasBytesSize> Default for CongestionQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::{sim_context::SimLink, EdgePolicy, Latency, NodePolicy, PacketLoss};

    use super::*;

    #[test]
    fn buffer_counter_refresh() {
        let time = Instant::now();
        let mut bc = BufferCounter::new(time);

        let consummed = bc.consume(time, Bandwidth::from_str("1gbps").unwrap(), 100);

        assert_eq!(bc.counter, consummed);

        let time = time + Duration::from_secs(2);
        bc.refresh(time);

        assert_eq!(bc.counter, 0)
    }

    #[test]
    fn buffer_counter_consume() {
        let time = Instant::now();
        let mut bc = BufferCounter::new(time);

        let consummed = bc.consume(time, Bandwidth::from_str("1kbps").unwrap(), 900);
        assert_eq!(consummed, 900, "We expect the whole message is consummed");
        assert_eq!(bc.counter, consummed);

        let consummed = bc.consume(time, Bandwidth::from_str("1kbps").unwrap(), 200);
        assert_eq!(consummed, 124, "only part of the message is consummed");
        assert_eq!(bc.counter, 1024);

        let consummed = bc.consume(time, Bandwidth::from_str("1kbps").unwrap(), 100);
        assert_eq!(consummed, 0, "We expect none of the message is consummed");
        assert_eq!(bc.counter, 1024);
    }

    struct Event;
    impl HasBytesSize for Event {
        fn bytes_size(&self) -> u64 {
            1_000
        }
    }

    const ALICE: SimId = SimId::new(0);
    const BOB: SimId = SimId::new(1);

    macro_rules! test_pop_message {
        ($cq:ident, $nodes:ident, $policy:ident, t: $time:expr, $sender:ident : $s:expr, $link:ident: $l:expr, $receiver:ident : $r:expr $(,)?) => {
            assert!($cq.pop($time, &$nodes, &$policy, 0).is_none());
            let sender = $cq.nodes_usage.get(&$sender).unwrap();
            assert_eq!(
                sender.upload.counter,
                $s,
                "expecting `{sender}' to have `{s}' but only `{sa}' received",
                sender = ::std::stringify!($sender),
                s = $s,
                sa = sender.upload.counter,
            );
            assert_eq!(sender.download.counter, 0); // untouched

            let link = $cq.edge_usage.get(&$link).unwrap();
            assert_eq!(link.download.counter, 0); // always 0
            assert_eq!(
                link.upload.counter,
                $l,
                "expecting `{link}' to have `{l}' but only `{la}' received",
                link = ::std::stringify!($link),
                l = $l,
                la = link.upload.counter,
            );

            let receiver = $cq.nodes_usage.get(&$receiver).unwrap();
            assert_eq!(receiver.upload.counter, 0);
            assert_eq!(
                receiver.download.counter,
                $r,
                "expecting `{receiver}' to have `{r}' but only `{ra}' received",
                receiver = ::std::stringify!($receiver),
                r = $r,
                ra = receiver.download.counter,
            );
        };
    }

    #[test]
    #[allow(clippy::vec_init_then_push, non_snake_case)]
    fn congestion_queue_pop() {
        let ALICE_BOB: Edge = Edge::new((ALICE, BOB));

        let mut policy = Policy::new();
        policy.set_default_node_policy(NodePolicy {
            bandwidth_down: "100bps".parse().unwrap(),
            bandwidth_up: "100bps".parse().unwrap(),
            location: None,
        });
        policy.set_default_edge_policy(EdgePolicy {
            bandwidth_down: "10bps".parse().unwrap(),
            bandwidth_up: "10bps".parse().unwrap(),
            latency: Latency::new(Duration::ZERO),
            packet_loss: PacketLoss::NONE,
        });

        let mut nodes = SimLinks::<()>::new();
        nodes.push(SimLink::new(()));
        nodes.push(SimLink::new(()));

        let mut cq = CongestionQueue::<Event>::new();

        let time = Instant::now();
        cq.push(time, Msg::new(ALICE, BOB, Event));

        // First we will need to do 10 iterations to clear alice's buffer
        for i in 0..10 {
            test_pop_message!(
                cq, nodes, policy,
                t: time + Duration::from_secs(i),
                ALICE: 100,
                ALICE_BOB: 10,
                BOB: 10,
            );
        }

        // then we need another 99 operation to _nearly_ clear
        // the link and BOB's buffers
        for i in 10..99 {
            // we expect the counter to be rest and fully used once more
            test_pop_message!(
                cq, nodes, policy,
                t: time + Duration::from_secs(i),
                ALICE: 0,
                ALICE_BOB: 10,
                BOB: 10,
            );
        }

        // it should take 100 iteration to pop the message
        assert!(cq
            .pop(time + Duration::from_secs(99), &nodes, &policy, 0)
            .is_some());
    }
}
