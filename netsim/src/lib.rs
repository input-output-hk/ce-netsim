/*!
# Network Simulator

*/

mod multiplexer;
mod socket;
pub mod stats;

// convenient re-export of `netsim_core` core objects
pub use netsim_core::{
    data::Data, Bandwidth, Latency, LinkId, NodeId, Packet, PacketBuilder, PacketId, PacketLoss,
};

pub use self::{
    multiplexer::{SimContext, SimLinkBuilder},
    socket::{RecvError, SendError, SendToError, SimSocket, TryRecvError},
    stats::{NodeStats, SimStats},
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ops::Deref, time::Instant};

    #[derive(Debug)]
    struct Msg(Instant);
    impl Deref for Msg {
        type Target = Instant;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl Msg {
        pub fn new() -> Self {
            Self(Instant::now())
        }
    }
    impl Data for Msg {
        fn bytes_size(&self) -> u64 {
            0
        }
    }

    #[test]
    fn simple() {
        let mut network = SimContext::<Msg>::new().unwrap();
        let mut n1 = network.open().build().unwrap();
        let mut n2 = network.open().build().unwrap();

        let packet_id = n1.send_to(n2.id(), Msg::new()).unwrap();

        let packet = n2
            .recv_packet()
            .expect("Should receive packets before disconnecting...");
        let id = packet.id();
        let msg = packet.into_inner();
        let elapsed = msg.elapsed();

        assert_eq!(id, packet_id);
        // The default link latency is 5ms. Wall-clock time will always be >= 5ms
        // because the multiplexer drives simulation time at real-time pace.
        // We use a loose upper bound to avoid flakiness on slow machines.
        assert!(
            elapsed.as_micros() >= 5000,
            "elapsed {elapsed:?} should be >= 5ms (default latency)"
        );
        assert!(
            elapsed.as_millis() < 1000,
            "elapsed {elapsed:?} should arrive in under 1s"
        );
    }
}
