use clap::Parser;
use netsim_async::{HasBytesSize, SimConfiguration, SimId, SimSocketReadHalf, SimSocketWriteHalf};
use rand::{thread_rng, RngCore as _};
use std::{sync::Arc, time::Duration};
use tokio::{
    select,
    sync::{Barrier, RwLock},
    time::{sleep, Instant, Sleep},
};

type SimContext = netsim_async::SimContext<Msg>;

type BeeIds = Arc<RwLock<Vec<SimId>>>;

#[derive(Parser)]
struct Command {
    #[arg(long, default_value = "1000")]
    swarm_size: usize,

    /// enable the tokio console subscriber
    #[arg(long)]
    tokio_console: bool,
}

#[tokio::main]
async fn main() {
    let cmd = Command::parse();

    if cmd.tokio_console {
        console_subscriber::init();
    }

    let ids = BeeIds::default();
    let barrier = Arc::new(Barrier::new(cmd.swarm_size));

    let configuration = SimConfiguration::<Msg>::default();
    let mut context: SimContext = SimContext::new(configuration).await;

    for _ in 0..cmd.swarm_size {
        let (reader, writer) = context.open().unwrap().into_split();

        let bee = BusyBee {
            writer,
            reader,
            bee_ids: ids.clone(),
        };

        let mut ids = ids.write().await;
        ids.push(bee.writer.id());

        tokio::spawn(bee.work(Arc::clone(&barrier)));
    }

    sleep(Duration::from_secs(60)).await;

    context.shutdown().await.unwrap();
}

struct BusyBee {
    writer: SimSocketWriteHalf<Msg>,
    reader: SimSocketReadHalf<Msg>,
    bee_ids: BeeIds,
}

impl BusyBee {
    fn when_to_write(&self) -> Sleep {
        let mut rng = thread_rng();
        let millis = rng.next_u64() % 1_000;
        sleep(Duration::from_millis(millis))
    }

    async fn sample_id(&mut self) -> SimId {
        let ids = self.bee_ids.read().await;
        let len = ids.len();
        let mut rng = thread_rng();
        let index = rng.next_u64() as usize % len;

        ids.get(index).copied().unwrap()
    }

    async fn sample_size(&mut self) -> u64 {
        let mut rng = thread_rng();
        rng.next_u64() % 1_000_000_000
    }

    async fn send_msg(&mut self) {
        let to = self.sample_id().await;
        let size = self.sample_size().await;

        let msg = Msg::new(size);
        self.writer.send_to(to, msg).unwrap()
    }

    async fn handle_inbound(&mut self, _from: SimId, msg: Msg) {
        let _id = self.reader.id();
        let _size = msg.size;
        let _elapsed = msg.time.elapsed().as_millis();
        // println!("{_from:04} -> {_id:04}: {_size} bytes ({_elapsed} ms)")
    }

    async fn work(mut self, barrier: Arc<Barrier>) {
        barrier.wait().await;

        loop {
            let time_to_write = self.when_to_write();
            let recv = self.reader.recv();

            select! {
                _ = time_to_write => { self.send_msg().await },
                Some((from, msg)) = recv => {
                    self.handle_inbound(from, msg).await
                }
            }
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
