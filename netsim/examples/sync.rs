use std::hint::black_box;

use anyhow::Result;
use netsim::SimContext;

#[derive(Debug, Clone, Copy)]
struct Data;
impl netsim_core::data::Data for Data {
    fn bytes_size(&self) -> u64 {
        1_024 * 1_024 * 1_024
    }
}

fn main() -> Result<()> {
    let mut network = SimContext::<Data>::new().unwrap();
    let mut n1 = network.open().build().unwrap();
    let mut n2 = network.open().build().unwrap();

    const COUNT: usize = 1_000_000;

    let n2_id = n2.id();
    let handle = std::thread::spawn(move || {
        for _ in 0..COUNT {
            n1.send_to(n2_id, Data).unwrap();
        }
    });

    for _ in 0..COUNT {
        let packet = n2.recv_packet().unwrap();
        let _data = black_box(packet.into_inner());
    }

    handle.join().unwrap();

    Ok(())
}
