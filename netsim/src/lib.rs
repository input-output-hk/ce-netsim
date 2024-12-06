/*!
# Network Simulator

*/

mod multiplexer;
mod socket;

// convenient re-export of `netsim_core` core objects
pub use netsim_core::{Bandwidth, Latency, NodeId, Packet, PacketBuilder, PacketId};

pub use self::{multiplexer::SimContext, socket::SimSocket};

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use socket::TryRecvError;

    use super::*;

    #[test]
    fn simple() {
        let mut network = SimContext::<()>::new().unwrap();
        let mut n1 = network.open().build().unwrap();
        let mut n2 = network.open().build().unwrap();

        let instant = Instant::now();
        let packet_id = n1.send_to(n2.id(), ()).unwrap();

        let packet;
        loop {
            packet = match n2.try_recv_packet() {
                Ok(packet) => packet,
                Err(TryRecvError::Empty) => continue,
                Err(TryRecvError::Disconnected) => panic!("disconnected prematurly"),
            };
            break;
        }

        let elapsed = instant.elapsed();

        assert_eq!(packet.id(), packet_id);

        // assert_eq!(elapsed.as_micros(), 5000);
    }
}
