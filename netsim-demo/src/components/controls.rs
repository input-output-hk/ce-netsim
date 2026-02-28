use crate::simulation::{Mode, NodeStats, Playground, format_bps, format_bytes, parse_bytes};
use leptos::{ev, prelude::*};
use netsim_core::{Bandwidth, Latency, LinkId, NodeId};

fn gauge_fill_class(pct: f64) -> &'static str {
    if pct >= 80.0 {
        "gauge-fill gauge-fill-critical"
    } else if pct >= 50.0 {
        "gauge-fill gauge-fill-warn"
    } else {
        "gauge-fill gauge-fill-ok"
    }
}

#[component]
pub fn Controls() -> impl IntoView {
    view! {
        <aside class="control-panel">
            <ModeSelector />
            <SimControls />
            <PropertiesPanel />
        </aside>
    }
}

#[component]
fn ModeSelector() -> impl IntoView {
    let pg = use_context::<Playground>().unwrap();

    let mode_label = move |m: &Mode| -> &'static str {
        match m {
            Mode::Select => "Select",
            Mode::AddNode => "Add Node",
            Mode::AddLink(_) => "Add Link",
            Mode::SendPacket(_) => "Send Packet",
        }
    };

    let is_active = move |target: &str| {
        let m = pg.mode.get();
        mode_label(&m) == target
    };

    let set_mode = move |target: &str| {
        let new_mode = match target {
            "Select" => Mode::Select,
            "Add Node" => Mode::AddNode,
            "Add Link" => Mode::AddLink(None),
            "Send Packet" => Mode::SendPacket(None),
            _ => Mode::Select,
        };
        pg.mode.set(new_mode);
    };

    let modes = ["Select", "Add Node", "Add Link", "Send Packet"];

    view! {
        <div class="control-section">
            <div class="control-section-title">"Mode"</div>
            <div class="mode-selector">
                {modes
                    .into_iter()
                    .map(|name| {
                        let name_owned = name.to_string();
                        let class = move || {
                            if is_active(name) {
                                "mode-btn mode-btn-active"
                            } else {
                                "mode-btn"
                            }
                        };
                        view! {
                            <button
                                class=class
                                on:click=move |_| set_mode(&name_owned)
                            >
                                {name}
                            </button>
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>
        </div>
    }
}

#[component]
fn SimControls() -> impl IntoView {
    let pg = use_context::<Playground>().unwrap();

    let on_step = {
        let pg = pg.clone();
        move |_: ev::MouseEvent| {
            pg.step();
        }
    };

    let on_play_pause = {
        let pg = pg.clone();
        move |_: ev::MouseEvent| {
            pg.playing.update(|p| *p = !*p);
        }
    };

    let on_reset = {
        let pg = pg.clone();
        move |_: ev::MouseEvent| {
            pg.reset();
        }
    };

    let play_label = move || {
        if pg.playing.get() { "Pause" } else { "Play" }
    };

    let step_ms_value = move || format!("{:.0}", pg.step_ms.get());

    let on_step_change = {
        let pg = pg.clone();
        move |ev: ev::Event| {
            let value = event_target_value(&ev);
            if let Ok(v) = value.parse::<f64>() {
                pg.step_ms.set(v.max(0.1));
            }
        }
    };

    // Packet size: text input with human-friendly parsing
    let (size_text, set_size_text) = signal(format_bytes(pg.packet_size.get_untracked()));
    let (size_err, set_size_err) = signal(String::new());

    let on_size_input = move |ev: ev::Event| {
        let val = event_target_value(&ev);
        set_size_text.set(val.clone());
        match parse_bytes(&val) {
            Ok(v) => {
                pg.packet_size.set(v.max(1));
                set_size_err.set(String::new());
            }
            Err(e) => set_size_err.set(e),
        }
    };

    let set_preset = move |bytes: u64| {
        pg.packet_size.set(bytes);
        set_size_text.set(format_bytes(bytes));
        set_size_err.set(String::new());
    };

    let size_input_class = move || {
        if size_err.get().is_empty() {
            "param-input"
        } else {
            "param-input param-input-error"
        }
    };

    view! {
        <div class="control-section">
            <div class="control-section-title">"Simulation"</div>

            <div class="param-group">
                <span class="param-label">"Step Duration (ms)"</span>
                <input
                    type="number"
                    class="param-input"
                    min="0.1"
                    step="1"
                    prop:value=step_ms_value
                    on:change=on_step_change
                />
            </div>

            <div class="sim-controls">
                <button class="btn btn-secondary" on:click=on_step>"Step"</button>
                <button class="btn btn-primary" on:click=on_play_pause>{play_label}</button>
            </div>

            <button class="btn btn-secondary btn-full" style="margin-top: var(--space-2)" on:click=on_reset>"Reset"</button>
        </div>

        <div class="control-section">
            <div class="control-section-title">"Packet Size"</div>
            <div class="param-group">
                <input
                    type="text"
                    class=size_input_class
                    placeholder="e.g. 1KB, 100MB, 1GB"
                    prop:value=move || size_text.get()
                    on:change=on_size_input
                />
                <span class="param-value">
                    {move || format_bytes(pg.packet_size.get())}
                </span>
                {move || {
                    let err = size_err.get();
                    if err.is_empty() {
                        view! { <span /> }.into_any()
                    } else {
                        view! { <span class="param-error">{err}</span> }.into_any()
                    }
                }}
            </div>
            <div class="preset-buttons">
                <button class="btn btn-small" on:click=move |_| set_preset(1_024)>"1 KB"</button>
                <button class="btn btn-small" on:click=move |_| set_preset(1_000_000)>"1 MB"</button>
                <button class="btn btn-small" on:click=move |_| set_preset(100_000_000)>"100 MB"</button>
                <button class="btn btn-small" on:click=move |_| set_preset(1_000_000_000)>"1 GB"</button>
            </div>
        </div>
    }
}

#[component]
fn PropertiesPanel() -> impl IntoView {
    let pg = use_context::<Playground>().unwrap();

    let pg_node = pg.clone();
    let selected_node_info = move || {
        let node_id = pg_node.selected_node.get()?;
        let nodes = pg_node.nodes.get();
        let node = nodes.iter().find(|n| n.id == node_id)?;
        let stats = pg_node.node_stats(node_id)?;
        Some((node_id, node.label.clone(), stats))
    };

    let pg_link = pg.clone();
    let selected_link_info = move || {
        let link_id = pg_link.selected_link.get()?;
        let links = pg_link.links.get();
        let link = links.iter().find(|l| l.id == link_id)?;
        Some((
            link_id,
            pg_link.node_label(link.from),
            pg_link.node_label(link.to),
            link.latency_ms,
            link.bandwidth_bps,
            link.packet_loss_pct,
        ))
    };

    view! {
        <div class="control-section">
            <div class="control-section-title">"Properties"</div>

            {move || {
                if let Some((node_id, label, stats)) = selected_node_info() {
                    view! {
                        <NodeProperties node_id=node_id label=label stats=stats />
                    }
                    .into_any()
                } else if let Some((
                    link_id,
                    from_label,
                    to_label,
                    latency_ms,
                    bandwidth_bps,
                    loss_pct,
                )) = selected_link_info()
                {
                    view! {
                        <LinkProperties
                            link_id=link_id
                            from_label=from_label
                            to_label=to_label
                            latency_ms=latency_ms
                            bandwidth_bps=bandwidth_bps
                            loss_pct=loss_pct
                        />
                    }
                    .into_any()
                } else {
                    view! {
                        <div class="no-selection">"Select a node or link to inspect"</div>
                    }
                    .into_any()
                }
            }}
        </div>
    }
}

#[component]
fn NodeProperties(node_id: NodeId, label: String, stats: NodeStats) -> impl IntoView {
    let pg = use_context::<Playground>().unwrap();

    // Text signals initialized with human-readable values
    let (ul_bw_text, set_ul_bw_text) = signal(format_bps(stats.upload_bandwidth_bps));
    let (dl_bw_text, set_dl_bw_text) = signal(format_bps(stats.download_bandwidth_bps));
    let (ul_buf_text, set_ul_buf_text) = signal(format_bytes(stats.upload_buffer_max));
    let (dl_buf_text, set_dl_buf_text) = signal(format_bytes(stats.download_buffer_max));
    let (error, set_error) = signal(String::new());

    // Live buffer usage — re-reads node_stats on every sim step
    let pg_ul = pg.clone();
    let live_ul = move || {
        let _ = pg_ul.sim_time.get(); // subscribe to sim ticks
        pg_ul.node_stats(node_id).map_or((0.0, 0, 0), |s| {
            let pct = if s.upload_buffer_max == 0 {
                0.0
            } else {
                (s.upload_buffer_used as f64 / s.upload_buffer_max as f64 * 100.0).min(100.0)
            };
            (pct, s.upload_buffer_used, s.upload_buffer_max)
        })
    };

    let pg_dl = pg.clone();
    let live_dl = move || {
        let _ = pg_dl.sim_time.get();
        pg_dl.node_stats(node_id).map_or((0.0, 0, 0), |s| {
            let pct = if s.download_buffer_max == 0 {
                0.0
            } else {
                (s.download_buffer_used as f64 / s.download_buffer_max as f64 * 100.0).min(100.0)
            };
            (pct, s.download_buffer_used, s.download_buffer_max)
        })
    };

    let on_apply = {
        let pg = pg.clone();
        move |_: ev::MouseEvent| {
            let ul_bw = match ul_bw_text.get_untracked().parse::<Bandwidth>() {
                Ok(bw) => bw.bits_per_sec(),
                Err(e) => {
                    set_error.set(format!("Upload BW: {e}"));
                    return;
                }
            };
            let dl_bw = match dl_bw_text.get_untracked().parse::<Bandwidth>() {
                Ok(bw) => bw.bits_per_sec(),
                Err(e) => {
                    set_error.set(format!("Download BW: {e}"));
                    return;
                }
            };
            let ul_buf = match parse_bytes(&ul_buf_text.get_untracked()) {
                Ok(v) => v,
                Err(e) => {
                    set_error.set(format!("Upload Buffer: {e}"));
                    return;
                }
            };
            let dl_buf = match parse_bytes(&dl_buf_text.get_untracked()) {
                Ok(v) => v,
                Err(e) => {
                    set_error.set(format!("Download Buffer: {e}"));
                    return;
                }
            };
            set_error.set(String::new());
            pg.update_node(node_id, ul_bw, dl_bw, ul_buf, dl_buf);
        }
    };

    view! {
        <div>
            <div class="info-block" style="margin-bottom: var(--space-3)">
                <div class="info-row">
                    <span class="info-row-label">"Node"</span>
                    <span class="info-row-value">{label}</span>
                </div>
            </div>

            <div class="param-group">
                <span class="param-label">"Upload Bandwidth"</span>
                <input
                    type="text"
                    class="param-input"
                    placeholder="e.g. 100mbps, 1gbps"
                    prop:value=move || ul_bw_text.get()
                    on:input=move |ev: ev::Event| set_ul_bw_text.set(event_target_value(&ev))
                />
            </div>

            <div class="param-group">
                <span class="param-label">"Download Bandwidth"</span>
                <input
                    type="text"
                    class="param-input"
                    placeholder="e.g. 100mbps, 1gbps"
                    prop:value=move || dl_bw_text.get()
                    on:input=move |ev: ev::Event| set_dl_bw_text.set(event_target_value(&ev))
                />
            </div>

            <div class="param-group">
                <span class="param-label">"Upload Buffer"</span>
                <input
                    type="text"
                    class="param-input"
                    placeholder="e.g. 1MB, 100MB, 1GB"
                    prop:value=move || ul_buf_text.get()
                    on:input=move |ev: ev::Event| set_ul_buf_text.set(event_target_value(&ev))
                />
                {
                    let live_ul_class = live_ul.clone();
                    let live_ul_style = live_ul.clone();
                    view! {
                        <div class="gauge-bar">
                            <div
                                class=move || gauge_fill_class(live_ul_class().0)
                                style=move || format!("width: {:.1}%", live_ul_style().0)
                            ></div>
                        </div>
                        <span class="param-readonly">
                            {move || {
                                let (pct, used, _max) = live_ul();
                                format!("{} used ({pct:.1}%)", format_bytes(used))
                            }}
                        </span>
                    }
                }
            </div>

            <div class="param-group">
                <span class="param-label">"Download Buffer"</span>
                <input
                    type="text"
                    class="param-input"
                    placeholder="e.g. 1MB, 100MB, 1GB"
                    prop:value=move || dl_buf_text.get()
                    on:input=move |ev: ev::Event| set_dl_buf_text.set(event_target_value(&ev))
                />
                {
                    let live_dl_class = live_dl.clone();
                    let live_dl_style = live_dl.clone();
                    view! {
                        <div class="gauge-bar">
                            <div
                                class=move || gauge_fill_class(live_dl_class().0)
                                style=move || format!("width: {:.1}%", live_dl_style().0)
                            ></div>
                        </div>
                        <span class="param-readonly">
                            {move || {
                                let (pct, used, _max) = live_dl();
                                format!("{} used ({pct:.1}%)", format_bytes(used))
                            }}
                        </span>
                    }
                }
            </div>

            {move || {
                let err = error.get();
                if err.is_empty() {
                    view! { <span /> }.into_any()
                } else {
                    view! { <span class="param-error">{err}</span> }.into_any()
                }
            }}

            <button class="btn btn-accent btn-full" on:click=on_apply>"Apply Changes"</button>
        </div>
    }
}

#[component]
fn LinkProperties(
    link_id: LinkId,
    from_label: String,
    to_label: String,
    latency_ms: f64,
    bandwidth_bps: u64,
    loss_pct: f64,
) -> impl IntoView {
    let pg = use_context::<Playground>().unwrap();

    // Clone labels for the TrafficSection (view macro consumes the originals).
    let from_label_flow = from_label.clone();
    let to_label_flow = to_label.clone();

    // Text signals initialized with human-readable values
    let (lat_text, set_lat_text) = signal(format!("{latency_ms:.0}ms"));
    let (bw_text, set_bw_text) = signal(format_bps(bandwidth_bps));
    let (loss, set_loss) = signal(loss_pct);
    let (error, set_error) = signal(String::new());

    let on_loss_change = move |ev: ev::Event| {
        if let Ok(v) = event_target_value(&ev).parse::<f64>() {
            set_loss.set(v.clamp(0.0, 100.0));
        }
    };

    let on_apply = {
        let pg = pg.clone();
        move |_: ev::MouseEvent| {
            let latency = match lat_text.get_untracked().parse::<Latency>() {
                Ok(lat) => lat.into_duration().as_millis() as f64,
                Err(e) => {
                    set_error.set(format!("Latency: {e}"));
                    return;
                }
            };
            let bandwidth = match bw_text.get_untracked().parse::<Bandwidth>() {
                Ok(bw) => bw.bits_per_sec(),
                Err(e) => {
                    set_error.set(format!("Bandwidth: {e}"));
                    return;
                }
            };
            set_error.set(String::new());
            pg.update_link(link_id, latency, bandwidth, loss.get_untracked());
        }
    };

    view! {
        <div>
            <div class="info-block" style="margin-bottom: var(--space-3)">
                <div class="info-row">
                    <span class="info-row-label">"Link"</span>
                    <span class="info-row-value">{from_label}" → "{to_label}</span>
                </div>
            </div>

            <div class="param-group">
                <span class="param-label">"Latency"</span>
                <input
                    type="text"
                    class="param-input"
                    placeholder="e.g. 50ms, 1s, 200ms"
                    prop:value=move || lat_text.get()
                    on:input=move |ev: ev::Event| set_lat_text.set(event_target_value(&ev))
                />
            </div>

            <div class="param-group">
                <span class="param-label">"Bandwidth"</span>
                <input
                    type="text"
                    class="param-input"
                    placeholder="e.g. 100mbps, 1gbps"
                    prop:value=move || bw_text.get()
                    on:input=move |ev: ev::Event| set_bw_text.set(event_target_value(&ev))
                />
            </div>

            <div class="param-group">
                <div style="display: flex; justify-content: space-between">
                    <span class="param-label">"Packet Loss"</span>
                    <span class="param-value">{move || format!("{:.1}%", loss.get())}</span>
                </div>
                <input
                    type="range"
                    class="param-input"
                    min="0" max="100" step="0.5"
                    prop:value=move || format!("{:.1}", loss.get())
                    on:input=on_loss_change
                />
            </div>

            {move || {
                let err = error.get();
                if err.is_empty() {
                    view! { <span /> }.into_any()
                } else {
                    view! { <span class="param-error">{err}</span> }.into_any()
                }
            }}

            <button class="btn btn-accent btn-full" on:click=on_apply>"Apply Changes"</button>

            <TrafficSection link_id=link_id from_label=from_label_flow to_label=to_label_flow />
        </div>
    }
}

#[component]
fn TrafficSection(link_id: LinkId, from_label: String, to_label: String) -> impl IntoView {
    let pg = use_context::<Playground>().unwrap();

    let (direction_reversed, set_direction_reversed) = signal(false);
    let (size_text, set_size_text) = signal("500MB".to_string());
    let (size_err, set_size_err) = signal(String::new());

    let from_l = from_label.clone();
    let to_l = to_label.clone();
    let direction_label = move || {
        if direction_reversed.get() {
            format!("{to_l} \u{2192} {from_l}")
        } else {
            format!("{from_l} \u{2192} {to_l}")
        }
    };

    let pg_flows = pg.clone();
    let active_flows = move || {
        let _ = pg_flows.sim_time.get();
        pg_flows.flows_for_link(link_id)
    };

    let on_start = {
        let pg = pg.clone();
        move |_: ev::MouseEvent| {
            let total = match parse_bytes(&size_text.get_untracked()) {
                Ok(v) if v > 0 => v,
                Ok(_) => {
                    set_size_err.set("size must be > 0".to_string());
                    return;
                }
                Err(e) => {
                    set_size_err.set(e);
                    return;
                }
            };
            set_size_err.set(String::new());

            let links = pg.links.get_untracked();
            let Some(lm) = links.iter().find(|l| l.id == link_id) else {
                return;
            };
            let (sender, receiver) = if direction_reversed.get_untracked() {
                (lm.to, lm.from)
            } else {
                (lm.from, lm.to)
            };

            let chunk_size = pg.packet_size.get_untracked();
            pg.start_flow(sender, receiver, total, chunk_size);
        }
    };

    let size_input_class = move || {
        if size_err.get().is_empty() {
            "param-input"
        } else {
            "param-input param-input-error"
        }
    };

    view! {
        <div style="margin-top: var(--space-3); padding-top: var(--space-3); border-top: 1px solid var(--color-border)">
            <div class="control-section-title">"Traffic Flow"</div>

            <div class="param-group">
                <span class="param-label">"Direction"</span>
                <button
                    class="btn btn-small btn-full"
                    on:click=move |_| set_direction_reversed.update(|d| *d = !*d)
                >
                    {direction_label}
                </button>
            </div>

            <div class="param-group">
                <span class="param-label">"Total Size"</span>
                <input
                    type="text"
                    class=size_input_class
                    placeholder="e.g. 100MB, 1GB"
                    prop:value=move || size_text.get()
                    on:input=move |ev: ev::Event| set_size_text.set(event_target_value(&ev))
                />
                {move || {
                    let err = size_err.get();
                    if err.is_empty() {
                        view! { <span /> }.into_any()
                    } else {
                        view! { <span class="param-error">{err}</span> }.into_any()
                    }
                }}
            </div>

            <div class="preset-buttons" style="margin-bottom: var(--space-3)">
                <button class="btn btn-small" on:click=move |_| set_size_text.set("100MB".to_string())>"100 MB"</button>
                <button class="btn btn-small" on:click=move |_| set_size_text.set("500MB".to_string())>"500 MB"</button>
                <button class="btn btn-small" on:click=move |_| set_size_text.set("1GB".to_string())>"1 GB"</button>
            </div>

            <button class="btn btn-primary btn-full" on:click=on_start>"Start Flow"</button>

            {move || {
                let flows = active_flows();
                if flows.is_empty() {
                    view! { <div /> }.into_any()
                } else {
                    view! {
                        <div style="margin-top: var(--space-3)">
                            {flows
                                .iter()
                                .map(|flow| {
                                    let pct = flow.progress() * 100.0;
                                    let delivered = format_bytes(flow.bytes_delivered);
                                    let total = format_bytes(flow.total_bytes);
                                    let label = if flow.active {
                                        "Active"
                                    } else if flow.completed() {
                                        "Done"
                                    } else {
                                        "Stopped"
                                    };
                                    let flow_id = flow.id;
                                    let is_active = flow.active;
                                    let pg_stop = pg.clone();
                                    view! {
                                        <div class="info-block" style="margin-bottom: var(--space-2)">
                                            <div style="display: flex; justify-content: space-between; margin-bottom: var(--space-1)">
                                                <span class="param-label">{label}</span>
                                                <span class="param-value">{format!("{pct:.1}%")}</span>
                                            </div>
                                            <div class="gauge-bar">
                                                <div
                                                    class=if is_active { "gauge-fill gauge-fill-ok" } else { "gauge-fill gauge-fill-warn" }
                                                    style=format!("width: {pct:.1}%")
                                                ></div>
                                            </div>
                                            <span class="param-readonly">{format!("{delivered} / {total}")}</span>
                                            {if is_active {
                                                view! {
                                                    <button
                                                        class="btn btn-small"
                                                        style="margin-top: var(--space-1)"
                                                        on:click=move |_| pg_stop.stop_flow(flow_id)
                                                    >"Stop"</button>
                                                }.into_any()
                                            } else {
                                                view! { <span /> }.into_any()
                                            }}
                                        </div>
                                    }
                                })
                                .collect::<Vec<_>>()}
                        </div>
                    }
                    .into_any()
                }
            }}
        </div>
    }
}
