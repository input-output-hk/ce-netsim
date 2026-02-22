use clap::Parser;
use netsim::{Data, NodeId, SimSocket};
use netsim_core::{time::Duration, Bandwidth};
use std::{
    thread::{self, sleep},
    time::Instant,
};

type SimContext = netsim::SimContext<Msg>;

#[derive(Parser)]
struct Command {
    /// duration in seconds
    #[arg(long, default_value = "60s")]
    time: Duration,

    /// in milliseconds
    #[arg(long, default_value = "10ms")]
    every: Duration,

    #[arg(long, default_value = "2")]
    num_tap: usize,

    #[arg(long, default_value = "500us")]
    idle: Duration,

    #[arg(long, default_value = "10gbps")]
    bandwidth_down: Bandwidth,
    #[arg(long, default_value = "10gbps")]
    bandwidth_up: Bandwidth,

    #[arg(long, default_value = "1ms")]
    latency: Duration,
}

fn main() {
    let cmd = Command::parse();

    let mut context: SimContext = SimContext::new().unwrap();

    let sink = Sink {
        socket: context
            .open()
            .set_download_bandwidth(cmd.bandwidth_down.clone())
            .set_upload_bandwidth(cmd.bandwidth_up.clone())
            .build()
            .unwrap(),
        latency: cmd.latency,
    };

    let mut taps = Vec::with_capacity(cmd.num_tap);
    for _ in 0..cmd.num_tap {
        let tap = Tap {
            socket: context
                .open()
                .set_download_bandwidth(cmd.bandwidth_down.clone())
                .set_upload_bandwidth(cmd.bandwidth_up.clone())
                .build()
                .unwrap(),
            sink_id: sink.socket.id(),
            every: cmd.every,
        };

        taps.push(tap);
    }

    let sink = thread::spawn(|| sink.work());

    let mut taps_ = Vec::with_capacity(cmd.num_tap);
    for tap in taps {
        taps_.push(thread::spawn(|| tap.work()));
    }

    sleep(cmd.time.into_duration());

    context.shutdown().unwrap();
    sink.join().unwrap();
    for tap in taps_ {
        tap.join().unwrap();
    }
}

struct Sink {
    socket: SimSocket<Msg>,
    latency: Duration,
}

impl Sink {
    fn work(mut self) {
        let mut delays = Vec::with_capacity(1_000_000);

        while let Ok(packet) = self.socket.recv_packet() {
            let msg = packet.into_inner();
            let latency = msg.time.elapsed();

            let diff = if latency < self.latency.into_duration() {
                self.latency.into_duration() - latency
            } else {
                latency - self.latency.into_duration()
            };

            delays.push(diff);
        }

        let len = delays.len();
        let total = delays.iter().copied().sum::<std::time::Duration>();
        let avg = total / delays.len() as u32;

        println!("sent {len} messages over. Msg received with an average of {avg:?} delays to the expected LATENCY");

        for (i, delay) in delays.iter().copied().enumerate().take(10) {
            println!("[{i}]: additional latency of {delay:?}");
        }
        println!("...");
    }
}

struct Tap {
    socket: SimSocket<Msg>,
    sink_id: NodeId,
    every: Duration,
}

impl Tap {
    fn send_msg(&mut self) -> bool {
        let msg = Msg::new(1);
        self.socket.send_to(self.sink_id, msg).is_ok()
    }

    fn work(mut self) {
        while self.send_msg() {
            sleep(self.every.into_duration());
        }
    }
}

struct Msg {
    time: Instant,
    size: u64,
}

impl Msg {
    fn new(size: u64) -> Self {
        Self {
            time: Instant::now(),
            size,
        }
    }
}

impl Data for Msg {
    fn bytes_size(&self) -> u64 {
        self.size
    }
}
