use clap::Parser;
use netsim_async::{HasBytesSize, SimConfiguration, NodeId, SimSocket};
use netsim_core::{time::Duration, Bandwidth, LinkId, EdgePolicy, Latency, NodePolicy, PacketLoss};
use tokio::time::{sleep, Instant};

type SimContext = netsim_async::SimContext<Msg>;

#[derive(Parser)]
struct Command {
    #[arg(long, default_value = "60")]
    time: Duration,

    #[arg(long, default_value = "10")]
    every: Duration,

    #[arg(long, default_value = "10")]
    idle: Duration,
}

const LATENCY: std::time::Duration = std::time::Duration::from_millis(60);

#[tokio::main]
async fn main() {
    let cmd = Command::parse();

    let configuration = SimConfiguration {
        idle_duration: cmd.idle.into_duration(),
        ..SimConfiguration::default()
    };

    let mut context: SimContext = SimContext::with_config(configuration);

    let sink = Sink {
        socket: context.open().unwrap(),
    };
    let tap = Tap {
        socket: context.open().unwrap(),
        sink_id: sink.socket.id(),
        every: cmd.every,
    };

    context
        .set_node_policy(
            sink.socket.id(),
            NodePolicy {
                bandwidth_down: Bandwidth::MAX,
                bandwidth_up: Bandwidth::MAX,
                location: None,
            },
        )
        .unwrap();
    context
        .set_node_policy(
            tap.socket.id(),
            NodePolicy {
                bandwidth_down: Bandwidth::MAX,
                bandwidth_up: Bandwidth::MAX,
                location: None,
            },
        )
        .unwrap();
    context
        .set_edge_policy(
            LinkId::new((tap.socket.id(), sink.socket.id())),
            EdgePolicy {
                latency: Latency::new(LATENCY),
                packet_loss: PacketLoss::NONE,
                ..Default::default()
            },
        )
        .unwrap();

    let sink = tokio::spawn(sink.work());
    let tap = tokio::spawn(tap.work());

    sleep(cmd.time.into_duration()).await;

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

    async fn work(mut self) {
        while self.send_msg() {
            let now = Instant::now();
            sleep(self.every.into_duration()).await;
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
