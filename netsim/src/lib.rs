/*!
# Network Simulator

*/

mod multiplexer;
mod socket;

// convenient re-export of `netsim_core` core objects
pub use netsim_core::{data::Data, Bandwidth, Latency, NodeId, Packet, PacketBuilder, PacketId};

pub use self::{
    multiplexer::SimContext,
    socket::{RecvError, SendError, SendToError, SimSocket, TryRecvError},
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
        // assert_eq!(elapsed.as_micros(), 5000);

        assert_eq!(id, packet_id);
    }
}
