use clap::Parser;
use netsim_async::{HasBytesSize, SimConfiguration, SimId, SimSocket};
use netsim_core::{Bandwidth, Edge, EdgePolicy, Latency, NodePolicy, PacketLoss};
use std::time::Duration;
use tokio::time::{sleep, Instant};

type SimContext = netsim_async::SimContext<Msg>;

#[derive(Parser)]
struct Command {
    #[arg(long, default_value = "60")]
    time: u64,

    #[arg(long, default_value = "10")]
    every: u64,
}

const LATENCY: Duration = Duration::from_millis(60);

#[tokio::main]
async fn main() {
    let cmd = Command::parse();

    let configuration = SimConfiguration::default();

    let mut context: SimContext = SimContext::with_config(configuration);

    let sink = Sink {
        socket: context.open().unwrap(),
    };
    let tap = Tap {
        socket: context.open().unwrap(),
        sink_id: sink.socket.id(),
        every: Duration::from_millis(cmd.every),
    };

    context.set_node_policy(
        sink.socket.id(),
        NodePolicy {
            bandwidth_down: Bandwidth::bits_per(u64::MAX, Duration::from_secs(1)),
            bandwidth_up: Bandwidth::bits_per(u64::MAX, Duration::from_secs(1)),
            location: None,
        },
    );
    context.set_node_policy(
        tap.socket.id(),
        NodePolicy {
            bandwidth_down: Bandwidth::bits_per(u64::MAX, Duration::from_secs(1)),
            bandwidth_up: Bandwidth::bits_per(u64::MAX, Duration::from_secs(1)),
            location: None,
        },
    );
    context.set_edge_policy(
        Edge::new((tap.socket.id(), sink.socket.id())),
        EdgePolicy {
            latency: Latency::new(LATENCY),
            packet_loss: PacketLoss::NONE,
        },
    );

    let sink = tokio::spawn(sink.work());
    let tap = tokio::spawn(tap.work());

    sleep(Duration::from_secs(cmd.time)).await;

    context.shutdown().unwrap();
    sink.await.unwrap();
    tap.await.unwrap();
}

struct Sink {
    socket: SimSocket<Msg>,
}

impl Sink {
    async fn work(mut self) {
        let mut delays = Vec::with_capacity(1_000_000);

        while let Some((_from, msg)) = self.socket.recv().await {
            let latency = msg.time.elapsed();

            let diff = if latency < LATENCY {
                LATENCY - latency
            } else {
                latency - LATENCY
            };

            delays.push(diff);
        }

        let len = delays.len();
        let total = delays.iter().copied().sum::<Duration>();
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
    sink_id: SimId,
    every: Duration,
}

impl Tap {
    fn send_msg(&mut self) -> bool {
        let msg = Msg::new(1);
        self.socket.send_to(self.sink_id, msg).is_ok()
    }

    async fn work(mut self) {
        while self.send_msg() {
            let now = Instant::now();
            sleep(self.every).await;
            let elapsed = now.elapsed();

            println!("{elapsed:?}");
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

impl HasBytesSize for Msg {
    fn bytes_size(&self) -> u64 {
        self.size
    }
}
