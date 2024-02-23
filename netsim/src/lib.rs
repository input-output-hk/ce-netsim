/*!
# Network Simulator

NetSim (As in Network Simulator) is a lightweight and simple to use
network simulation framework.

*/

mod sim_context;
mod sim_link;
mod sim_socket;

pub use crate::{
    sim_context::SimContext,
    sim_socket::{SimSocket, SimSocketReadHalf, SimSocketWriteHalf, TryRecv},
};
pub use netsim_core::{
    Bandwidth, Edge, EdgePolicy, HasBytesSize, Latency, NodePolicy, PacketLoss, SimConfiguration,
    SimId,
};
