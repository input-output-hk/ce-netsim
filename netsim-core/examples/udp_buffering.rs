use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};

use anyhow::{anyhow, Context, Result};
use indicatif::ProgressBar;

const SOCKET_RECV: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 9928));
const SOCKET_SEND: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 9929));
const MAX_SIZE: u64 = 10 * 1024;

/// this function was used to make sure that we understand the
/// behaviour for sending packets in the UDP model.
///
/// On MacOS we found that we manage to read up to ~3500 packets
/// and then the remaining packets are dropped or missing.
///
/// it shows that we don't really mind from the sender side what
/// the receiver will do and that we should just drop the packets
/// if they aren't received.
///
/// Something this test also shows is that the packet is still sent
/// from this side and that it affects the local bandwidth (i.e. the
/// upload congestion).
///
fn main() -> Result<()> {
    let receiving = UdpSocket::bind(SOCKET_RECV).context("Failed to open receiving socket")?;
    let sending = UdpSocket::bind(SOCKET_SEND).context("Failed to open sending socket")?;
    sending
        .connect(SOCKET_RECV)
        .context("Failed to connect to receiving socket")?;

    let pb = ProgressBar::new(MAX_SIZE);
    for i in 0..MAX_SIZE {
        sending
            .send(&i.to_be_bytes())
            .with_context(|| anyhow!("Failed to send message {i} to receiving"))?;
        pb.inc(1);
    }
    pb.finish_with_message("All sent");

    let pb = ProgressBar::new(MAX_SIZE);
    for i in 0..MAX_SIZE {
        let mut buf = [0u8; 8];
        receiving
            .recv(&mut buf)
            .with_context(|| anyhow!("Failed to receive message {i} from sender"))?;
        let y = u64::from_be_bytes(buf);
        assert_eq!(i, y);
        pb.inc(1);
    }
    pb.finish_with_message("All received");

    Ok(())
}
