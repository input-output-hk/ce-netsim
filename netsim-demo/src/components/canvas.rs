use crate::simulation::{Mode, Playground};
use leptos::prelude::*;
use netsim_core::LinkId;
use wasm_bindgen::JsCast;

/// Returns a hex colour for the given utilization percentage.
fn gauge_color(pct: f64) -> &'static str {
    if pct > 80.0 {
        "#EF4444" // red
    } else if pct > 50.0 {
        "#F59E0B" // amber
    } else {
        "#06FF89" // green
    }
}

#[component]
pub fn Canvas() -> impl IntoView {
    let pg = use_context::<Playground>().unwrap();

    let on_click = {
        let pg = pg.clone();
        move |ev: web_sys::MouseEvent| {
            // Get bounding rect from the event target (the SVG element)
            let target = ev
                .current_target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok());

            let Some(el) = target else { return };
            let rect = el.get_bounding_client_rect();

            // Convert mouse coords to SVG viewBox space (0..1000 x 0..600)
            let svg_x = (ev.client_x() as f64 - rect.x()) / rect.width() * 1000.0;
            let svg_y = (ev.client_y() as f64 - rect.y()) / rect.height() * 600.0;

            let mode = pg.mode.get_untracked();
            match mode {
                Mode::AddNode => {
                    pg.add_node(svg_x, svg_y);
                }
                Mode::AddLink(first) => {
                    if let Some(node_id) = pg.find_node_at(svg_x, svg_y, 25.0) {
                        match first {
                            None => {
                                pg.mode.set(Mode::AddLink(Some(node_id)));
                            }
                            Some(from) => {
                                if from != node_id {
                                    pg.add_link(from, node_id);
                                }
                                pg.mode.set(Mode::AddLink(None));
                            }
                        }
                    }
                }
                Mode::SendPacket(first) => {
                    if let Some(node_id) = pg.find_node_at(svg_x, svg_y, 25.0) {
                        match first {
                            None => {
                                pg.mode.set(Mode::SendPacket(Some(node_id)));
                            }
                            Some(from) => {
                                if from != node_id {
                                    pg.send_packet(from, node_id);
                                }
                                pg.mode.set(Mode::SendPacket(None));
                            }
                        }
                    }
                }
                Mode::Select => {
                    if let Some(node_id) = pg.find_node_at(svg_x, svg_y, 25.0) {
                        pg.selected_node.set(Some(node_id));
                        pg.selected_link.set(None);
                    } else if let Some(link_id) = pg.find_link_at(svg_x, svg_y, 15.0) {
                        pg.selected_link.set(Some(link_id));
                        pg.selected_node.set(None);
                    } else {
                        pg.selected_node.set(None);
                        pg.selected_link.set(None);
                    }
                }
            }
        }
    };

    let hint_text = move || match pg.mode.get() {
        Mode::Select => "Click a node or link to inspect",
        Mode::AddNode => "Click on the canvas to place a node",
        Mode::AddLink(None) => "Click the first node",
        Mode::AddLink(Some(_)) => "Click the second node to complete the link",
        Mode::SendPacket(None) => "Click the source node",
        Mode::SendPacket(Some(_)) => "Click the destination node",
    };

    view! {
        <div class="canvas-area">
            <svg
                viewBox="0 0 1000 600"
                preserveAspectRatio="xMidYMid meet"
                on:click=on_click
            >
                <LinkLines />
                <FlowStreams />
                <NodeCards />
                <PacketDots />
            </svg>
            <div class="canvas-hint">{hint_text}</div>
        </div>
    }
}

#[component]
fn LinkLines() -> impl IntoView {
    let pg = use_context::<Playground>().unwrap();

    let links_view = move || {
        let links = pg.links.get();
        let nodes = pg.nodes.get();
        let selected = pg.selected_link.get();
        let flows = pg.flows.get();
        let congestion = pg.link_congestion.get();

        links
            .iter()
            .map(|link| {
                let from_pos = nodes.iter().find(|n| n.id == link.from);
                let to_pos = nodes.iter().find(|n| n.id == link.to);

                let (x1, y1, x2, y2) = match (from_pos, to_pos) {
                    (Some(f), Some(t)) => (f.x, f.y, t.x, t.y),
                    _ => return view! { <g /> }.into_any(),
                };

                let is_selected = selected == Some(link.id);
                let has_active_flow = flows
                    .iter()
                    .any(|f| f.active && LinkId::new((f.from, f.to)) == link.id);
                let class = if is_selected {
                    "link-line link-line-selected"
                } else if has_active_flow {
                    "link-line link-line-active"
                } else {
                    "link-line"
                };

                let mid_x = (x1 + x2) / 2.0;
                let mid_y = (y1 + y2) / 2.0 - 12.0;
                let label = format!("{}ms", link.latency_ms as u64);

                let cong = congestion.get(&link.id).cloned().unwrap_or_default();
                let gauge_x = mid_x - 25.0;
                let gauge_y_fwd = mid_y + 6.0;
                let gauge_y_rev = mid_y + 14.0;
                let fwd_w = format!("{:.1}", 50.0 * cong.forward_pct / 100.0);
                let rev_w = format!("{:.1}", 50.0 * cong.reverse_pct / 100.0);
                let arrow_x = (gauge_x - 10.0).to_string();

                view! {
                    <g>
                        <line
                            x1=x1.to_string()
                            y1=y1.to_string()
                            x2=x2.to_string()
                            y2=y2.to_string()
                            class=class
                        />
                        <text x=mid_x.to_string() y=mid_y.to_string() class="link-label">
                            {label}
                        </text>
                        <text x=arrow_x.clone() y=(gauge_y_fwd + 4.0).to_string() class="link-gauge-arrow">{"\u{25B8}"}</text>
                        <rect
                            x=gauge_x.to_string() y=gauge_y_fwd.to_string()
                            width="50" height="4" rx="2"
                            class="svg-gauge-bg"
                        />
                        <rect
                            x=gauge_x.to_string() y=gauge_y_fwd.to_string()
                            width=fwd_w height="4" rx="2"
                            fill=gauge_color(cong.forward_pct)
                        />
                        <text x=arrow_x y=(gauge_y_rev + 4.0).to_string() class="link-gauge-arrow">{"\u{25C2}"}</text>
                        <rect
                            x=gauge_x.to_string() y=gauge_y_rev.to_string()
                            width="50" height="4" rx="2"
                            class="svg-gauge-bg"
                        />
                        <rect
                            x=gauge_x.to_string() y=gauge_y_rev.to_string()
                            width=rev_w height="4" rx="2"
                            fill=gauge_color(cong.reverse_pct)
                        />
                    </g>
                }
                .into_any()
            })
            .collect::<Vec<_>>()
    };

    view! { {links_view} }
}

#[component]
fn NodeCards() -> impl IntoView {
    let pg = use_context::<Playground>().unwrap();

    let nodes_view = move || {
        let nodes = pg.nodes.get();
        let selected = pg.selected_node.get();
        let mode = pg.mode.get();
        let congestion = pg.node_congestion.get();

        let pending = match &mode {
            Mode::AddLink(Some(id)) | Mode::SendPacket(Some(id)) => Some(*id),
            _ => None,
        };

        nodes
            .iter()
            .map(|node| {
                let is_selected = selected == Some(node.id);
                let is_pending = pending == Some(node.id);

                let card_class = if is_selected {
                    "node-card node-card-selected"
                } else if is_pending {
                    "node-card node-card-pending"
                } else {
                    "node-card"
                };

                let cong = congestion.get(&node.id).cloned().unwrap_or_default();

                let tx = node.x - 60.0;
                let ty = node.y - 40.0;
                let transform = format!("translate({tx}, {ty})");
                let label = node.label.clone();

                let ul_buf_w = format!("{:.1}", 74.0 * cong.upload_buffer_pct / 100.0);
                let ul_bw_w = format!("{:.1}", 74.0 * cong.upload_bw_pct / 100.0);
                let dl_buf_w = format!("{:.1}", 74.0 * cong.download_buffer_pct / 100.0);
                let dl_bw_w = format!("{:.1}", 74.0 * cong.download_bw_pct / 100.0);

                view! {
                    <g transform=transform>
                        <rect width="120" height="80" rx="6" ry="6" class=card_class />
                        <text x="60" y="14" class="node-card-label">{label}</text>

                        <text x="4" y="28" class="node-gauge-label">{"UL Buf"}</text>
                        <rect x="42" y="21" width="74" height="8" rx="2" class="svg-gauge-bg" />
                        <rect x="42" y="21" width=ul_buf_w height="8" rx="2"
                            fill=gauge_color(cong.upload_buffer_pct) />

                        <text x="4" y="41" class="node-gauge-label">{"UL BW"}</text>
                        <rect x="42" y="34" width="74" height="8" rx="2" class="svg-gauge-bg" />
                        <rect x="42" y="34" width=ul_bw_w height="8" rx="2"
                            fill=gauge_color(cong.upload_bw_pct) />

                        <text x="4" y="54" class="node-gauge-label">{"DL Buf"}</text>
                        <rect x="42" y="47" width="74" height="8" rx="2" class="svg-gauge-bg" />
                        <rect x="42" y="47" width=dl_buf_w height="8" rx="2"
                            fill=gauge_color(cong.download_buffer_pct) />

                        <text x="4" y="67" class="node-gauge-label">{"DL BW"}</text>
                        <rect x="42" y="60" width="74" height="8" rx="2" class="svg-gauge-bg" />
                        <rect x="42" y="60" width=dl_bw_w height="8" rx="2"
                            fill=gauge_color(cong.download_bw_pct) />
                    </g>
                }
            })
            .collect::<Vec<_>>()
    };

    view! { {nodes_view} }
}

/// Renders active flows as line segments on their links.
///
/// The **head** (leading edge) represents the first bytes sent and advances
/// from the sender toward the receiver at the speed dictated by link latency.
/// The **tail** (trailing edge) stays at the sender while bytes are still being
/// sent, then advances once all bytes have left. The line disappears when the
/// tail reaches the receiver — i.e. the transfer is fully complete.
#[component]
fn FlowStreams() -> impl IntoView {
    let pg = use_context::<Playground>().unwrap();

    let streams_view = move || {
        let flows = pg.flows.get();
        let nodes = pg.nodes.get();
        let links = pg.links.get();
        let sim_time = pg.sim_time.get();

        flows
            .iter()
            .filter_map(|flow| {
                let started_at = flow.started_at?;

                // Find the link and its latency.
                let link = links
                    .iter()
                    .find(|l| l.id == LinkId::new((flow.from, flow.to)))?;
                let latency_secs = (link.latency_ms / 1000.0).max(0.001);

                // Head: position of the first bytes along the link.
                let elapsed_head = sim_time.saturating_sub(started_at).as_secs_f64();
                let head_pos = (elapsed_head / latency_secs).min(1.0);

                // Tail: stays at sender while sending, then drains.
                let tail_pos = match flow.finished_sending_at {
                    Some(fin) => {
                        let elapsed_tail = sim_time.saturating_sub(fin).as_secs_f64();
                        (elapsed_tail / latency_secs).min(1.0)
                    }
                    None => 0.0,
                };

                // Nothing visible once the tail has reached the receiver.
                if tail_pos >= 1.0 {
                    return None;
                }

                // Node positions — direction matters (from → to).
                let from_node = nodes.iter().find(|n| n.id == flow.from)?;
                let to_node = nodes.iter().find(|n| n.id == flow.to)?;

                let x1 = from_node.x + (to_node.x - from_node.x) * tail_pos;
                let y1 = from_node.y + (to_node.y - from_node.y) * tail_pos;
                let x2 = from_node.x + (to_node.x - from_node.x) * head_pos;
                let y2 = from_node.y + (to_node.y - from_node.y) * head_pos;

                Some(view! {
                    <line
                        x1=x1.to_string()
                        y1=y1.to_string()
                        x2=x2.to_string()
                        y2=y2.to_string()
                        class="flow-stream"
                        stroke-linecap="round"
                    />
                })
            })
            .collect::<Vec<_>>()
    };

    view! { {streams_view} }
}

#[component]
fn PacketDots() -> impl IntoView {
    let pg = use_context::<Playground>().unwrap();

    let packets_view = move || {
        let packets = pg.in_flight.get();
        let nodes = pg.nodes.get();
        let sim_time = pg.sim_time.get();

        packets
            .iter()
            .filter(|pkt| pkt.flow_id.is_none()) // flow packets rendered by FlowStreams
            .filter_map(|pkt| {
                let from_pos = nodes.iter().find(|n| n.id == pkt.from)?;
                let to_pos = nodes.iter().find(|n| n.id == pkt.to)?;

                let elapsed = sim_time.saturating_sub(pkt.sent_at);
                let total = pkt.est_latency.as_secs_f64().max(0.001);
                let progress = (elapsed.as_secs_f64() / total).min(1.0);

                let x = from_pos.x + (to_pos.x - from_pos.x) * progress;
                let y = from_pos.y + (to_pos.y - from_pos.y) * progress;

                Some(view! {
                    <circle cx=x.to_string() cy=y.to_string() class="packet-dot" r="6" />
                })
            })
            .collect::<Vec<_>>()
    };

    view! { {packets_view} }
}
