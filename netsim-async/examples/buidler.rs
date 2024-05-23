use std::sync::{Arc, Mutex};

use clap::Parser;
use netsim_async::{HasBytesSize, SimConfiguration, SimId, SimSocket};
use netsim_core::{time::Duration, Bandwidth, EdgePolicy, Latency, NodePolicy};
use tokio::{
    select,
    sync::watch,
    time::{Instant, MissedTickBehavior},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key(usize);

struct Network {
    topology: Arc<Vec<SimId>>,
    socket: SimSocket<Block>,
}

struct Consensus {
    keys: Vec<Key>,
    slot_time: Duration,
    start: Instant,
}

impl Consensus {
    fn new(size: usize, slot_time: Duration, start: Instant) -> Arc<Self> {
        Arc::new(Consensus {
            keys: (0..size).map(Key).collect(),
            slot_time,
            start,
        })
    }

    fn slot_at(&self, time: Instant) -> usize {
        let elapsed = time.duration_since(self.start);
        (elapsed.as_millis() / self.slot_time.into_duration().as_millis()) as usize
    }

    fn key_at(&self, time: Instant) -> Key {
        let num_slots = self.slot_at(time);
        Key(num_slots % self.keys.len())
    }
}

#[derive(Default)]
struct State {
    blocks: Vec<usize>,
}

struct Node {
    consensus: Arc<Consensus>,
    key: Key,
    block_size: usize,
    last_block: usize,
    stop: watch::Receiver<bool>,
    network: Network,

    state: Arc<Mutex<State>>,
    blocks: Vec<(Instant, Block)>,
}

impl State {
    fn add_block(&mut self, block: Block) {
        let index = block.slot;
        if index >= self.blocks.len() {
            self.blocks.extend(vec![0; index - self.blocks.len() + 1]);
        }
        self.blocks[index] += 1;
    }

    fn to_data(&self) -> Vec<(String, u64)> {
        self.blocks
            .iter()
            .enumerate()
            .map(|(i, c)| (i.to_string(), *c as u64))
            .collect()
    }
}

impl Node {
    fn accept_block(&self, time: Instant, block: Block) -> bool {
        let expected_slot = self.consensus.slot_at(time);
        let expected_from = self.consensus.key_at(time);

        expected_slot == block.slot && expected_from == block.from
    }

    fn make_block(&self, time: Instant) -> Block {
        debug_assert!(
            self.is_self_turn(time),
            "[slot: {slot:04}]Not my time to create a block {key:?} (expected {expected:?})",
            key = self.key,
            expected = self.consensus.key_at(time),
            slot = self.consensus.slot_at(time),
        );

        Block {
            from: self.key,
            slot: self.consensus.slot_at(time),
            size: self.block_size,
        }
    }

    fn is_self_turn(&self, time: Instant) -> bool {
        self.consensus.key_at(time) == self.key
    }

    fn instant_until_turn(&self, time: Instant) -> Instant {
        let current_slot = self.consensus.slot_at(time);
        let last = self.consensus.keys.len();
        let Key(now) = self.consensus.key_at(time);
        let Key(me) = self.key;

        let num_slots = if now == me && current_slot == self.last_block {
            last as u32 // wait a whole cycle, we already produce
        } else if now <= me {
            (me - now) as u32
        } else {
            (last - now + me) as u32
        };
        let duration = self.consensus.slot_time.into_duration() * num_slots;

        time + duration
    }

    fn produce_block(&mut self, time: Instant) {
        if !self.is_self_turn(time) {
            return;
        }
        let block = self.make_block(time);

        self.last_block = block.slot;
        self.blocks.push((time, block));
        println!("[{key:?}][{slot:03}]", key = self.key, slot = block.slot);

        for peer in self.network.topology.iter().copied() {
            if peer == self.network.socket.id() {
                continue;
            }
            if self.network.socket.send_to(peer, block).is_err() {
                break;
            }
        }
    }

    fn accept_block_(&mut self, time: Instant, block: Block) {
        if self.accept_block(time, block) {
            // Good
            self.last_block = block.slot;
            self.state.lock().unwrap().add_block(block);
        } else {
            // Bad
        }
    }

    async fn start(mut self) {
        let time = Instant::now();

        let start = self.instant_until_turn(time);
        let my_interval =
            self.consensus.slot_time.into_duration() * self.consensus.keys.len() as u32;

        let mut interval = tokio::time::interval_at(start, my_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut stop = self.stop.clone();
        loop {
            select! {
                time = interval.tick() => self.produce_block(time),
                Some((_, block)) = self.network.socket.recv() => self.accept_block_(Instant::now(), block),
                _ = stop.wait_for(|b| *b) => break,
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Block {
    from: Key,
    slot: usize,
    size: usize,
}

impl HasBytesSize for Block {
    fn bytes_size(&self) -> u64 {
        self.size as u64
    }
}

type SimContext = netsim_async::SimContext<Block>;

#[derive(Parser)]
struct Command {
    /// duration of the experiment
    #[arg(long, default_value = "60s")]
    time: Duration,

    /// the number of seconds between two blocks
    #[arg(long, default_value = "1s")]
    slot_time: Duration,

    /// the number of nodes in the simulation
    #[arg(long, default_value = "42")]
    size: usize,

    /// the size of the blocks shared
    #[arg(long, default_value = "64000000")]
    block: usize,

    #[arg(long, default_value = "1mbps")]
    bandwidth: Bandwidth,

    /// the default latency for all messages
    #[arg(long, default_value = "500ms")]
    latency: Latency,

    /// parameter for the simulator's routing
    #[arg(long, default_value = "10us")]
    idle: Duration,
}

#[tokio::main]
async fn main() {
    let cmd = Command::parse();

    let mut configuration = SimConfiguration {
        idle_duration: cmd.idle.into_duration(),
        ..Default::default()
    };

    configuration.policy.set_default_node_policy(NodePolicy {
        bandwidth_down: cmd.bandwidth,
        bandwidth_up: cmd.bandwidth,
        ..Default::default()
    });
    configuration.policy.set_default_edge_policy(EdgePolicy {
        latency: cmd.latency,
        ..Default::default()
    });

    let mut context: SimContext = SimContext::with_config(configuration);
    let stop = watch::channel(false);
    let state = Arc::new(Mutex::new(State::default()));

    let sockets: Vec<SimSocket<Block>> = (0..cmd.size).map(|_| context.open().unwrap()).collect();
    let topology: Arc<Vec<_>> = Arc::new(sockets.iter().map(|socket| socket.id()).collect());

    let consensus = Consensus::new(cmd.size, cmd.slot_time, Instant::now());
    let nodes: Vec<Node> = consensus
        .keys
        .clone()
        .into_iter()
        .zip(sockets)
        .map(|(key, socket)| Node {
            consensus: Arc::clone(&consensus),
            block_size: cmd.block,
            key,
            network: Network {
                socket,
                topology: Arc::clone(&topology),
            },
            last_block: 0,
            stop: stop.1.clone(),
            blocks: Vec::new(),
            state: Arc::clone(&state),
        })
        .collect();

    let mut workers = Vec::with_capacity(nodes.len());
    for node in nodes {
        workers.push(tokio::spawn(node.start()));
    }

    // tokio::time::sleep(cmd.time.into_duration()).await;

    main_tui(state).unwrap();

    context.shutdown().unwrap();
    stop.0.send(true).unwrap();
}

// UI Stuff

struct App {
    state: Arc<Mutex<State>>,
    data: Vec<(String, u64)>,
}

impl App {
    fn new(state: Arc<Mutex<State>>) -> Self {
        App {
            state,
            data: Vec::new(),
        }
    }

    fn on_tick(&mut self) {
        self.data = self.state.lock().unwrap().to_data();
    }
}

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{error::Error, io};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{BarChart, Block as WBlock, Borders},
    Frame, Terminal,
};

fn main_tui(state: Arc<Mutex<State>>) -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let tick_rate = std::time::Duration::from_millis(250);
    let app = App::new(state);
    let res = run_app(&mut terminal, app, tick_rate);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: std::time::Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| ui(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| std::time::Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    return Ok(());
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &App) {
    let data: Vec<_> = app.data.iter().map(|(a, v)| (a.as_str(), *v)).collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(f.size());
    let barchart = BarChart::default()
        .block(WBlock::default().title("Data1").borders(Borders::ALL))
        .data(&data[data.len().saturating_sub(30)..])
        .bar_width(3)
        .bar_style(Style::default().fg(Color::LightGreen))
        .value_style(Style::default().fg(Color::Black).bg(Color::LightGreen));
    f.render_widget(barchart, chunks[0]);
}
