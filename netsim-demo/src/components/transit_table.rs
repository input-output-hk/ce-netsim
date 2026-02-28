use crate::simulation::{Playground, TransitStatus, format_bytes};
use leptos::prelude::*;

/// Returns a hex colour for the given fill percentage.
fn gauge_color(pct: f64) -> &'static str {
    if pct > 80.0 {
        "#EF4444"
    } else if pct > 50.0 {
        "#F59E0B"
    } else {
        "#06FF89"
    }
}

#[component]
pub fn TransitTable() -> impl IntoView {
    let pg = use_context::<Playground>().unwrap();

    let rows = move || {
        let table = pg.transit_table.get();

        table
            .iter()
            .map(|snap| {
                let from_label = pg.node_label(snap.from);
                let to_label = pg.node_label(snap.to);
                let route = format!("{from_label} \u{2192} {to_label}");

                // Short packet ID (last 4 hex digits).
                let id_str = format!("{}", snap.packet_id);
                let id_short = if id_str.len() > 6 {
                    format!("..{}", &id_str[id_str.len() - 4..])
                } else {
                    id_str
                };

                let total = snap.bytes_total.max(1) as f64;
                let ul_pct = snap.upload_pending as f64 / total * 100.0;
                let lk_pct = snap.link_pending as f64 / total * 100.0;
                let dl_pct = snap.download_pending as f64 / total * 100.0;

                let ul_w = format!("{:.1}%", ul_pct.min(100.0));
                let lk_w = format!("{:.1}%", lk_pct.min(100.0));
                let dl_w = format!("{:.1}%", dl_pct.min(100.0));

                let ul_text = format_bytes(snap.upload_pending);
                let lk_text = format_bytes(snap.link_pending);
                let dl_text = format_bytes(snap.download_pending);

                let (status_class, status_label) = match snap.status {
                    TransitStatus::Active => ("transit-status transit-status-active", "ACTIVE"),
                    TransitStatus::Delivered => {
                        ("transit-status transit-status-delivered", "RECV")
                    }
                    TransitStatus::Dropped => ("transit-status transit-status-dropped", "DROP"),
                };

                let row_class = match snap.status {
                    TransitStatus::Active => "transit-row",
                    TransitStatus::Delivered => "transit-row transit-row-delivered",
                    TransitStatus::Dropped => "transit-row transit-row-dropped",
                };

                view! {
                    <div class=row_class>
                        <span class="transit-col transit-col-id">{id_short}</span>
                        <span class="transit-col transit-col-route">{route}</span>
                        <span class="transit-col transit-col-stage">
                            <span class="transit-stage-label">{"UL"}</span>
                            <span class="transit-gauge">
                                <span
                                    class="transit-gauge-fill"
                                    style:width=ul_w
                                    style:background=gauge_color(ul_pct)
                                />
                            </span>
                            <span class="transit-stage-bytes">{ul_text}</span>
                        </span>
                        <span class="transit-col transit-col-stage">
                            <span class="transit-stage-label">{"LK"}</span>
                            <span class="transit-gauge">
                                <span
                                    class="transit-gauge-fill"
                                    style:width=lk_w
                                    style:background=gauge_color(lk_pct)
                                />
                            </span>
                            <span class="transit-stage-bytes">{lk_text}</span>
                        </span>
                        <span class="transit-col transit-col-stage">
                            <span class="transit-stage-label">{"DL"}</span>
                            <span class="transit-gauge">
                                <span
                                    class="transit-gauge-fill"
                                    style:width=dl_w
                                    style:background=gauge_color(dl_pct)
                                />
                            </span>
                            <span class="transit-stage-bytes">{dl_text}</span>
                        </span>
                        <span class=status_class>{status_label}</span>
                    </div>
                }
            })
            .collect::<Vec<_>>()
    };

    view! {
        <div class="transit-table">
            <div class="transit-table-header">
                <span class="transit-col transit-col-id">{"#"}</span>
                <span class="transit-col transit-col-route">{"Route"}</span>
                <span class="transit-col transit-col-stage">{"Upload"}</span>
                <span class="transit-col transit-col-stage">{"Link"}</span>
                <span class="transit-col transit-col-stage">{"Download"}</span>
                <span class="transit-status">{"Status"}</span>
            </div>
            <div class="transit-table-body">
                {move || {
                    let items = rows();
                    if items.is_empty() {
                        view! {
                            <div class="transit-empty">
                                "No packets in transit. Send a packet or start a flow."
                            </div>
                        }
                            .into_any()
                    } else {
                        view! { <div>{items}</div> }.into_any()
                    }
                }}
            </div>
        </div>
    }
}
