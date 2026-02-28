use leptos::prelude::*;
use netsim_core::{
    Bandwidth, Latency, LinkId, NodeId, Packet, PacketId, PacketLoss, data::Data, network::Network,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};

// ---------------------------------------------------------------------------
// Payload type
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct SimMessage {
    pub size: u64,
    pub flow_id: Option<FlowId>,
}

impl Data for SimMessage {
    fn bytes_size(&self) -> u64 {
        self.size
    }
}

// ---------------------------------------------------------------------------
// Traffic flows
// ---------------------------------------------------------------------------

pub type FlowId = u64;

#[derive(Clone, Debug)]
pub struct TrafficFlow {
    pub id: FlowId,
    pub from: NodeId,
    pub to: NodeId,
    pub total_bytes: u64,
    pub bytes_sent: u64,
    pub bytes_delivered: u64,
    pub chunk_size: u64,
    pub active: bool,
    /// Sim time when the first chunk was actually sent.
    pub started_at: Option<Duration>,
    /// Sim time when the last byte left the sender.
    pub finished_sending_at: Option<Duration>,
}

impl TrafficFlow {
    /// True when every byte has been delivered.
    pub fn completed(&self) -> bool {
        self.bytes_delivered >= self.total_bytes
    }

    /// Progress as a ratio 0.0..=1.0 based on bytes delivered.
    pub fn progress(&self) -> f64 {
        if self.total_bytes == 0 {
            1.0
        } else {
            (self.bytes_delivered as f64 / self.total_bytes as f64).min(1.0)
        }
    }
}

// ---------------------------------------------------------------------------
// Metadata types (stored in signals for reactivity)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct NodeMeta {
    pub id: NodeId,
    pub label: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug)]
pub struct LinkMeta {
    pub id: LinkId,
    pub from: NodeId,
    pub to: NodeId,
    pub latency_ms: f64,
    pub bandwidth_bps: u64,
    pub packet_loss_pct: f64,
}

#[derive(Clone, Debug)]
pub struct InFlightPacket {
    pub id: PacketId,
    pub from: NodeId,
    pub to: NodeId,
    pub sent_at: Duration,
    pub est_latency: Duration,
    /// If this packet belongs to a flow, its ID.
    pub flow_id: Option<FlowId>,
}

#[derive(Clone, Debug)]
pub struct NodeStats {
    pub upload_bandwidth_bps: u64,
    pub download_bandwidth_bps: u64,
    pub upload_buffer_max: u64,
    pub upload_buffer_used: u64,
    pub download_buffer_max: u64,
    pub download_buffer_used: u64,
}

// ---------------------------------------------------------------------------
// Per-tick congestion snapshots (updated each step)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct NodeCongestion {
    /// Upload buffer fill (0–100%).
    pub upload_buffer_pct: f64,
    /// Upload bandwidth utilization (0–100%).
    pub upload_bw_pct: f64,
    /// Download buffer fill (0–100%).
    pub download_buffer_pct: f64,
    /// Download bandwidth utilization (0–100%).
    pub download_bw_pct: f64,
}

#[derive(Clone, Debug, Default)]
pub struct LinkCongestion {
    /// Forward (smaller→larger NodeId) bandwidth utilization (0–100%).
    pub forward_pct: f64,
    /// Reverse (larger→smaller NodeId) bandwidth utilization (0–100%).
    pub reverse_pct: f64,
}

// ---------------------------------------------------------------------------
// Per-tick transit snapshots (updated each step)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct TransitSnapshot {
    pub packet_id: PacketId,
    pub from: NodeId,
    pub to: NodeId,
    pub bytes_total: u64,
    pub upload_pending: u64,
    pub link_pending: u64,
    pub download_pending: u64,
    pub status: TransitStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TransitStatus {
    Active,
    Delivered,
    Dropped,
}

// ---------------------------------------------------------------------------
// Interaction mode
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub ul_bw: u64,
    pub dl_bw: u64,
    pub ul_buf: u64,
    pub dl_buf: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Select,
    AddNode,
    AddLink(Option<NodeId>),
    SendPacket(Option<NodeId>),
}

// ---------------------------------------------------------------------------
// Playground context
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Playground {
    network: Arc<Mutex<Network<SimMessage>>>,
    pub nodes: RwSignal<Vec<NodeMeta>>,
    pub links: RwSignal<Vec<LinkMeta>>,
    pub in_flight: RwSignal<Vec<InFlightPacket>>,
    pub sim_time: RwSignal<Duration>,
    pub mode: RwSignal<Mode>,
    pub selected_node: RwSignal<Option<NodeId>>,
    pub selected_link: RwSignal<Option<LinkId>>,
    pub playing: RwSignal<bool>,
    pub step_ms: RwSignal<f64>,
    node_counter: Arc<Mutex<u32>>,
    pub packet_size: RwSignal<u64>,
    pub flows: RwSignal<Vec<TrafficFlow>>,
    flow_counter: Arc<Mutex<FlowId>>,
    pub node_congestion: RwSignal<HashMap<NodeId, NodeCongestion>>,
    pub link_congestion: RwSignal<HashMap<LinkId, LinkCongestion>>,
    pub transit_table: RwSignal<Vec<TransitSnapshot>>,
}

impl Playground {
    pub fn new() -> Self {
        let pg = Self {
            network: Arc::new(Mutex::new(Network::new())),
            nodes: RwSignal::new(Vec::new()),
            links: RwSignal::new(Vec::new()),
            in_flight: RwSignal::new(Vec::new()),
            sim_time: RwSignal::new(Duration::ZERO),
            mode: RwSignal::new(Mode::Select),
            selected_node: RwSignal::new(None),
            selected_link: RwSignal::new(None),
            playing: RwSignal::new(false),
            step_ms: RwSignal::new(10.0),
            node_counter: Arc::new(Mutex::new(0)),
            packet_size: RwSignal::new(1_000_000), // 1 MB default
            flows: RwSignal::new(Vec::new()),
            flow_counter: Arc::new(Mutex::new(0)),
            node_congestion: RwSignal::new(HashMap::new()),
            link_congestion: RwSignal::new(HashMap::new()),
            transit_table: RwSignal::new(Vec::new()),
        };
        pg.build_demo_topology();
        pg
    }

    /// Build an initial 3-node demo topology.
    ///
    /// ```text
    ///   Server ──────── Relay
    ///      \             /
    ///       \           /
    ///        \         /
    ///         Client
    /// ```
    fn build_demo_topology(&self) {
        // Server: high capacity data center node
        let server = self.add_named_node(
            "Server",
            NodeConfig {
                label: String::new(),
                x: 250.0,
                y: 180.0,
                ul_bw: 1_000_000_000,  // 1 Gbps upload
                dl_bw: 1_000_000_000,  // 1 Gbps download
                ul_buf: 1_000_000_000, // 1 GB upload buffer
                dl_buf: 1_000_000_000, // 1 GB download buffer
            },
        );

        // Relay: mid-tier forwarding node
        let relay = self.add_named_node(
            "Relay",
            NodeConfig {
                label: String::new(),
                x: 750.0,
                y: 180.0,
                ul_bw: 500_000_000,  // 500 Mbps upload
                dl_bw: 500_000_000,  // 500 Mbps download
                ul_buf: 100_000_000, // 100 MB upload buffer
                dl_buf: 100_000_000, // 100 MB download buffer
            },
        );

        // Client: typical end-user connection
        let client = self.add_named_node(
            "Client",
            NodeConfig {
                label: String::new(),
                x: 500.0,
                y: 420.0,
                ul_bw: 50_000_000,   // 50 Mbps upload
                dl_bw: 200_000_000,  // 200 Mbps download
                ul_buf: 10_000_000,  // 10 MB upload buffer
                dl_buf: 100_000_000, // 100 MB download buffer
            },
        );

        // Server ↔ Relay
        self.add_link_with_config(server, relay, 10.0, 1_000_000_000, 0.0);

        // Relay ↔ Client
        self.add_link_with_config(relay, client, 10.0, 1_000_000_000, 0.0);

        // Server ↔ Client
        self.add_link_with_config(server, client, 10.0, 1_000_000_000, 0.0);
    }

    /// Reset the playground to its initial state.
    pub fn reset(&self) {
        *self.network.lock().unwrap() = Network::new();
        *self.node_counter.lock().unwrap() = 0;
        self.nodes.set(Vec::new());
        self.links.set(Vec::new());
        self.in_flight.set(Vec::new());
        self.sim_time.set(Duration::ZERO);
        self.mode.set(Mode::Select);
        self.selected_node.set(None);
        self.selected_link.set(None);
        self.playing.set(false);
        self.flows.set(Vec::new());
        *self.flow_counter.lock().unwrap() = 0;
        self.node_congestion.set(HashMap::new());
        self.link_congestion.set(HashMap::new());
        self.transit_table.set(Vec::new());
        self.build_demo_topology();
    }

    pub fn add_node(&self, x: f64, y: f64) -> NodeId {
        let mut counter = self.node_counter.lock().unwrap();
        *counter += 1;
        let label = format!("N{counter}");
        self.add_node_with_config(NodeConfig {
            label,
            x,
            y,
            ul_bw: 100_000_000,
            dl_bw: 1_000_000_000,
            ul_buf: 100_000_000,
            dl_buf: 100_000_000,
        })
    }

    fn add_named_node(&self, label: &str, cfg: NodeConfig) -> NodeId {
        let mut counter = self.node_counter.lock().unwrap();
        *counter += 1;
        self.add_node_with_config(NodeConfig {
            label: label.to_string(),
            ..cfg
        })
    }

    fn add_node_with_config(&self, cfg: NodeConfig) -> NodeId {
        let id = {
            let mut net = self.network.lock().unwrap();
            net.new_node()
                .set_upload_bandwidth(Bandwidth::new(cfg.ul_bw))
                .set_download_bandwidth(Bandwidth::new(cfg.dl_bw))
                .set_upload_buffer(cfg.ul_buf)
                .set_download_buffer(cfg.dl_buf)
                .build()
        };

        self.nodes.update(|nodes| {
            nodes.push(NodeMeta {
                id,
                label: cfg.label,
                x: cfg.x,
                y: cfg.y,
            });
        });

        id
    }

    pub fn add_link(&self, a: NodeId, b: NodeId) {
        self.add_link_with_config(a, b, 20.0, 100_000_000, 0.0);
    }

    fn add_link_with_config(
        &self,
        a: NodeId,
        b: NodeId,
        latency_ms: f64,
        bandwidth_bps: u64,
        packet_loss_pct: f64,
    ) {
        let link_id = LinkId::new((a, b));

        // Don't add duplicate links
        if self.links.get_untracked().iter().any(|l| l.id == link_id) {
            return;
        }

        let packet_loss = if packet_loss_pct <= 0.0 {
            PacketLoss::None
        } else {
            PacketLoss::rate(packet_loss_pct / 100.0).unwrap_or(PacketLoss::None)
        };

        {
            let mut net = self.network.lock().unwrap();
            net.configure_link(a, b)
                .set_latency(Latency::new(Duration::from_millis(latency_ms as u64)))
                .set_bandwidth(Bandwidth::new(bandwidth_bps))
                .set_packet_loss(packet_loss)
                .apply();
        }

        self.links.update(|links| {
            links.push(LinkMeta {
                id: link_id,
                from: a,
                to: b,
                latency_ms,
                bandwidth_bps,
                packet_loss_pct,
            });
        });
    }

    pub fn update_link(&self, link_id: LinkId, latency_ms: f64, bandwidth_bps: u64, loss_pct: f64) {
        let (a, b) = link_id.into_nodes();

        let packet_loss = if loss_pct <= 0.0 {
            PacketLoss::None
        } else {
            PacketLoss::rate(loss_pct / 100.0).unwrap_or(PacketLoss::None)
        };

        {
            let mut net = self.network.lock().unwrap();
            net.configure_link(a, b)
                .set_latency(Latency::new(Duration::from_millis(latency_ms as u64)))
                .set_bandwidth(Bandwidth::new(bandwidth_bps))
                .set_packet_loss(packet_loss)
                .apply();
        }

        self.links.update(|links| {
            if let Some(link) = links.iter_mut().find(|l| l.id == link_id) {
                link.latency_ms = latency_ms;
                link.bandwidth_bps = bandwidth_bps;
                link.packet_loss_pct = loss_pct;
            }
        });
    }

    pub fn send_packet(&self, from: NodeId, to: NodeId) {
        let size = self.packet_size.get_untracked();
        let sim_time = self.sim_time.get_untracked();

        let est_latency = self
            .links
            .get_untracked()
            .iter()
            .find(|l| l.id == LinkId::new((from, to)))
            .map(|l| Duration::from_millis(l.latency_ms as u64))
            .unwrap_or(Duration::from_millis(50));

        let result = {
            let mut net = self.network.lock().unwrap();
            let packet = Packet::builder(net.packet_id_generator())
                .from(from)
                .to(to)
                .data(SimMessage {
                    size,
                    flow_id: None,
                })
                .build();

            match packet {
                Ok(pkt) => {
                    let id = pkt.id();
                    match net.send(pkt) {
                        Ok(()) => Ok(id),
                        Err(e) => Err(format!("{e}")),
                    }
                }
                Err(e) => Err(format!("{e}")),
            }
        };

        if let Ok(id) = result {
            self.in_flight.update(|packets| {
                packets.push(InFlightPacket {
                    id,
                    from,
                    to,
                    sent_at: sim_time,
                    est_latency,
                    flow_id: None,
                });
            });
        }
    }

    pub fn step(&self) {
        // Pump active flows before advancing the network.
        self.pump_flows();

        let step_duration = Duration::from_micros((self.step_ms.get_untracked() * 1000.0) as u64);

        let mut delivered: Vec<(PacketId, NodeId, NodeId, u64, Option<FlowId>)> = Vec::new();
        let mut corrupted: Vec<TransitSnapshot> = Vec::new();
        let mut nc = HashMap::new();
        let mut lc = HashMap::new();

        // Carry over previously dropped packets so they persist in the table.
        let mut transit_snapshots: Vec<TransitSnapshot> = self
            .transit_table
            .get_untracked()
            .into_iter()
            .filter(|s| s.status == TransitStatus::Dropped)
            .collect();

        {
            let mut net = self.network.lock().unwrap();
            net.advance_with_report(
                step_duration,
                |pkt| {
                    let id = pkt.id();
                    let from = pkt.from();
                    let to = pkt.to();
                    let msg = pkt.into_inner();
                    delivered.push((id, from, to, msg.size, msg.flow_id));
                },
                |transit| {
                    corrupted.push(TransitSnapshot {
                        packet_id: transit.packet_id(),
                        from: transit.from(),
                        to: transit.to(),
                        bytes_total: transit.bytes_size(),
                        upload_pending: transit.upload_pending(),
                        link_pending: transit.link_pending(),
                        download_pending: transit.download_pending(),
                        status: TransitStatus::Dropped,
                    });
                },
            );

            // Snapshot congestion state while still holding the lock.
            for node_meta in self.nodes.get_untracked().iter() {
                if let Some(node) = net.node(node_meta.id) {
                    nc.insert(
                        node_meta.id,
                        NodeCongestion {
                            upload_buffer_pct: buf_pct(
                                node.upload_buffer_used(),
                                node.upload_buffer_max(),
                            ),
                            upload_bw_pct: util_pct(
                                node.upload_channel_used(),
                                node.upload_channel_remaining(),
                            ),
                            download_buffer_pct: buf_pct(
                                node.download_buffer_used(),
                                node.download_buffer_max(),
                            ),
                            download_bw_pct: util_pct(
                                node.download_channel_used(),
                                node.download_channel_remaining(),
                            ),
                        },
                    );
                }
            }

            for link_meta in self.links.get_untracked().iter() {
                if let Some(link) = net.link(link_meta.id) {
                    lc.insert(
                        link_meta.id,
                        LinkCongestion {
                            forward_pct: util_pct(
                                link.forward_channel_used(),
                                link.forward_channel_remaining(),
                            ),
                            reverse_pct: util_pct(
                                link.reverse_channel_used(),
                                link.reverse_channel_remaining(),
                            ),
                        },
                    );
                }
            }

            // Snapshot all active transits.
            for transit in net.transits() {
                transit_snapshots.push(TransitSnapshot {
                    packet_id: transit.packet_id(),
                    from: transit.from(),
                    to: transit.to(),
                    bytes_total: transit.bytes_size(),
                    upload_pending: transit.upload_pending(),
                    link_pending: transit.link_pending(),
                    download_pending: transit.download_pending(),
                    status: TransitStatus::Active,
                });
            }
        }

        self.node_congestion.set(nc);
        self.link_congestion.set(lc);

        let new_sim_time = self.sim_time.get_untracked() + step_duration;
        self.sim_time.set(new_sim_time);

        // Add newly corrupted transits (with their actual final byte state).
        transit_snapshots.extend(corrupted);

        // Add delivered packets to the table.
        let delivered_ids: HashSet<PacketId> = delivered.iter().map(|(id, ..)| *id).collect();
        for &(id, from, to, size, _) in &delivered {
            transit_snapshots.push(TransitSnapshot {
                packet_id: id,
                from,
                to,
                bytes_total: size,
                upload_pending: 0,
                link_pending: 0,
                download_pending: size,
                status: TransitStatus::Delivered,
            });
        }

        self.transit_table.set(transit_snapshots);

        if !delivered.is_empty() {
            self.in_flight.update(|packets| {
                packets.retain(|p| !delivered_ids.contains(&p.id));
            });

            // Attribute delivered bytes to flows and detect completions.
            self.flows.update(|flows| {
                for &(_, _, _, size, flow_id) in &delivered {
                    if let Some(fid) = flow_id
                        && let Some(flow) = flows.iter_mut().find(|f| f.id == fid)
                    {
                        flow.bytes_delivered += size;
                        if flow.completed() && flow.active {
                            flow.active = false;
                        }
                    }
                }
            });
        }
    }

    // -----------------------------------------------------------------------
    // Traffic flow engine
    // -----------------------------------------------------------------------

    pub fn start_flow(
        &self,
        from: NodeId,
        to: NodeId,
        total_bytes: u64,
        chunk_size: u64,
    ) -> FlowId {
        let id = {
            let mut counter = self.flow_counter.lock().unwrap();
            *counter += 1;
            *counter
        };

        self.flows.update(|flows| {
            flows.push(TrafficFlow {
                id,
                from,
                to,
                total_bytes,
                bytes_sent: 0,
                bytes_delivered: 0,
                chunk_size,
                active: true,
                started_at: None,
                finished_sending_at: None,
            });
        });

        // Auto-play when a flow starts.
        self.playing.set(true);

        id
    }

    pub fn stop_flow(&self, flow_id: FlowId) {
        let sim_time = self.sim_time.get_untracked();
        self.flows.update(|flows| {
            if let Some(flow) = flows.iter_mut().find(|f| f.id == flow_id) {
                flow.active = false;
                if flow.finished_sending_at.is_none() {
                    flow.finished_sending_at = Some(sim_time);
                }
            }
        });
    }

    /// Returns all flows that match a given link (both directions).
    pub fn flows_for_link(&self, link_id: LinkId) -> Vec<TrafficFlow> {
        self.flows
            .get()
            .iter()
            .filter(|f| LinkId::new((f.from, f.to)) == link_id)
            .cloned()
            .collect()
    }

    /// Inject one chunk per active flow into the network.
    fn pump_flows(&self) {
        let sim_time = self.sim_time.get_untracked();

        // Snapshot active flows that still have bytes to send.
        let active: Vec<(FlowId, NodeId, NodeId, u64, u64)> = self
            .flows
            .get_untracked()
            .iter()
            .filter(|f| f.active && f.bytes_sent < f.total_bytes)
            .map(|f| {
                (
                    f.id,
                    f.from,
                    f.to,
                    f.total_bytes - f.bytes_sent,
                    f.chunk_size,
                )
            })
            .collect();

        for (flow_id, from, to, remaining, max_chunk) in active {
            let available = {
                let net = self.network.lock().unwrap();
                let ul_available = net
                    .node(from)
                    .map(|n| n.upload_buffer_max().saturating_sub(n.upload_buffer_used()))
                    .unwrap_or(0);
                let dl_available = net
                    .node(to)
                    .map(|n| n.download_buffer_max().saturating_sub(n.download_buffer_used()))
                    .unwrap_or(0);
                ul_available.min(dl_available)
            };

            if available == 0 {
                continue;
            }

            let chunk = remaining.min(available).min(max_chunk);
            if chunk == 0 {
                continue;
            }

            let est_latency = self
                .links
                .get_untracked()
                .iter()
                .find(|l| l.id == LinkId::new((from, to)))
                .map(|l| Duration::from_millis(l.latency_ms as u64))
                .unwrap_or(Duration::from_millis(50));

            let result = {
                let mut net = self.network.lock().unwrap();
                let packet = Packet::builder(net.packet_id_generator())
                    .from(from)
                    .to(to)
                    .data(SimMessage {
                        size: chunk,
                        flow_id: Some(flow_id),
                    })
                    .build();

                match packet {
                    Ok(pkt) => {
                        let id = pkt.id();
                        match net.send(pkt) {
                            Ok(()) => Ok((id, chunk)),
                            Err(_) => Err(()),
                        }
                    }
                    Err(_) => Err(()),
                }
            };

            if let Ok((pkt_id, sent_bytes)) = result {
                self.in_flight.update(|packets| {
                    packets.push(InFlightPacket {
                        id: pkt_id,
                        from,
                        to,
                        sent_at: sim_time,
                        est_latency,
                        flow_id: Some(flow_id),
                    });
                });

                self.flows.update(|flows| {
                    if let Some(flow) = flows.iter_mut().find(|f| f.id == flow_id) {
                        if flow.started_at.is_none() {
                            flow.started_at = Some(sim_time);
                        }
                        flow.bytes_sent += sent_bytes;
                        if flow.bytes_sent >= flow.total_bytes && flow.finished_sending_at.is_none()
                        {
                            flow.finished_sending_at = Some(sim_time);
                        }
                    }
                });
            }
        }
    }

    pub fn node_label(&self, id: NodeId) -> String {
        self.nodes
            .get_untracked()
            .iter()
            .find(|n| n.id == id)
            .map(|n| n.label.clone())
            .unwrap_or_else(|| format!("{id}"))
    }

    pub fn find_node_at(&self, x: f64, y: f64, _radius: f64) -> Option<NodeId> {
        let half_w = 60.0;
        let half_h = 40.0;
        self.nodes.get_untracked().iter().find_map(|n| {
            if (n.x - half_w..=n.x + half_w).contains(&x)
                && (n.y - half_h..=n.y + half_h).contains(&y)
            {
                Some(n.id)
            } else {
                None
            }
        })
    }

    pub fn find_link_at(&self, x: f64, y: f64, threshold: f64) -> Option<LinkId> {
        let nodes = self.nodes.get_untracked();
        self.links.get_untracked().iter().find_map(|link| {
            let from_pos = nodes.iter().find(|n| n.id == link.from)?;
            let to_pos = nodes.iter().find(|n| n.id == link.to)?;
            let dist = point_to_line_distance(x, y, from_pos.x, from_pos.y, to_pos.x, to_pos.y);
            if dist <= threshold {
                Some(link.id)
            } else {
                None
            }
        })
    }

    pub fn node_stats(&self, id: NodeId) -> Option<NodeStats> {
        let net = self.network.lock().unwrap();
        let node = net.node(id)?;
        Some(NodeStats {
            upload_bandwidth_bps: node.upload_bandwidth().bits_per_sec(),
            download_bandwidth_bps: node.download_bandwidth().bits_per_sec(),
            upload_buffer_max: node.upload_buffer_max(),
            upload_buffer_used: node.upload_buffer_used(),
            download_buffer_max: node.download_buffer_max(),
            download_buffer_used: node.download_buffer_used(),
        })
    }

    pub fn update_node(&self, id: NodeId, ul_bw: u64, dl_bw: u64, ul_buf: u64, dl_buf: u64) {
        let mut net = self.network.lock().unwrap();
        net.configure_node(id)
            .set_upload_bandwidth(Bandwidth::new(ul_bw))
            .set_download_bandwidth(Bandwidth::new(dl_bw))
            .set_upload_buffer(ul_buf)
            .set_download_buffer(dl_buf)
            .apply();
    }
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{bytes} B")
    }
}

/// Parse a human-friendly byte size string.
///
/// Accepts formats: `"1024"`, `"1KB"`, `"100MB"`, `"1GB"`, `"500B"`.
/// Case-insensitive, optional space between number and unit.
pub fn parse_bytes(input: &str) -> Result<u64, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("empty input".to_string());
    }

    let num_end = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());
    let (num_str, unit_str) = s.split_at(num_end);
    let unit_str = unit_str.trim().to_lowercase();

    let num: f64 = num_str
        .parse()
        .map_err(|_| format!("invalid number: {num_str}"))?;

    let multiplier: u64 = match unit_str.as_str() {
        "" | "b" | "bytes" => 1,
        "kb" => 1_000,
        "mb" => 1_000_000,
        "gb" => 1_000_000_000,
        "tb" => 1_000_000_000_000,
        "kib" => 1_024,
        "mib" => 1_048_576,
        "gib" => 1_073_741_824,
        other => return Err(format!("unknown unit: {other}")),
    };

    Ok((num * multiplier as f64) as u64)
}

pub fn format_bps(bps: u64) -> String {
    Bandwidth::new(bps).to_string()
}

/// Buffer fill as a percentage.
fn buf_pct(used: u64, max: u64) -> f64 {
    if max == 0 {
        0.0
    } else {
        (used as f64 / max as f64 * 100.0).min(100.0)
    }
}

/// Bandwidth utilization as a percentage: `used / (used + remaining) * 100`.
///
/// For idle channels (never updated this round) both `used` and `remaining`
/// are zero, so this correctly returns 0% instead of 100%.
fn util_pct(used: u64, remaining: u64) -> f64 {
    let total = used + remaining;
    if total == 0 {
        0.0
    } else {
        (used as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
    }
}

fn point_to_line_distance(px: f64, py: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;

    if len_sq == 0.0 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }

    let t = ((px - x1) * dx + (py - y1) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    let proj_x = x1 + t * dx;
    let proj_y = y1 + t * dy;

    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}
