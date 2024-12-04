use std::{
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
};

use anyhow::{anyhow, Context, Result};
use indicatif::ProgressBar;

const SOCKET_ADDR: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 9928));
const MAX_SIZE: u64 = 1024 * 1024;

/// this is example is to test how the buffering works in TCP case
///
/// on MacOS we manage to send over 60K messages that are not read on the
/// receiving end. This shows the buffer is being maxed on the receiving
/// end and that it is affecting the up to the sender: the server is then
/// blocked from sending new messages because the sending buffer has been
/// filled up.
///
/// All of this is availabe in documentations about the network protocols for TCP
/// and for UDP. However it is interesting to have a feel for how things actually
/// work and these examples are here for that.
///
fn main() -> Result<()> {
    let listener = TcpListener::bind(SOCKET_ADDR).context("Failed to open listening socket")?;
    let mut sending = TcpStream::connect(SOCKET_ADDR).context("Failed to connect to remote TCP")?;

    let (mut receiving, _addr) = listener
        .accept()
        .context("Failed to accept inbound connection")?;

    let pb = ProgressBar::new(MAX_SIZE);
    for i in 0..MAX_SIZE {
        sending
            .write_all(&i.to_be_bytes())
            .with_context(|| anyhow!("Failed to send message {i}"))?;
        pb.inc(1);
    }
    pb.finish_with_message("All sent");

    let pb = ProgressBar::new(MAX_SIZE);
    for i in 0..MAX_SIZE {
        let mut buf = [0u8; 8];
        receiving
            .read_exact(&mut buf)
            .with_context(|| anyhow!("Failed to receive message {i}"))?;
        let j = u64::from_be_bytes(buf);
        assert_eq!(i, j);
        pb.inc(1);
    }
    pb.finish_with_message("All received");

    Ok(())
}
