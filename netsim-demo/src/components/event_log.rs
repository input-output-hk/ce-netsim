use crate::simulation::{LogEntryKind, Playground};
use leptos::prelude::*;

#[component]
pub fn EventLog() -> impl IntoView {
    let pg = use_context::<Playground>().unwrap();

    let entries = move || {
        let log = pg.log.get();
        log.iter()
            .rev()
            .map(|entry| {
                let time_ms = entry.sim_time.as_millis();
                let time_str = if time_ms < 1000 {
                    format!("{time_ms}ms")
                } else {
                    format!("{:.2}s", time_ms as f64 / 1000.0)
                };

                let (kind_class, kind_label) = match entry.kind {
                    LogEntryKind::Sent => ("log-kind log-kind-sent", "SENT"),
                    LogEntryKind::Delivered => ("log-kind log-kind-delivered", "RECV"),
                    LogEntryKind::Error => ("log-kind log-kind-error", "ERR"),
                };

                let route = format!("{} \u{2192} {}", entry.from_label, entry.to_label);
                let message = entry.message.clone();

                view! {
                    <div class="log-entry">
                        <span class="log-time">{time_str}</span>
                        <span class=kind_class>{kind_label}</span>
                        <span class="log-route">{route}</span>
                        <span class="log-message">{message}</span>
                    </div>
                }
            })
            .collect::<Vec<_>>()
    };

    view! {
        <div class="event-log">
            <div class="event-log-header">"Event Log"</div>
            <div class="event-log-body">
                {move || {
                    let items = entries();
                    if items.is_empty() {
                        view! { <div class="log-empty">"No events yet. Add nodes and send packets to see activity."</div> }.into_any()
                    } else {
                        view! { <div>{items}</div> }.into_any()
                    }
                }}
            </div>
        </div>
    }
}
