use crate::simulation::Playground;
use leptos::prelude::*;

#[component]
pub fn Header() -> impl IntoView {
    let pg = use_context::<Playground>().unwrap();

    let sim_time_display = move || {
        let t = pg.sim_time.get();
        let total_ms = t.as_millis();
        if total_ms < 1000 {
            format!("{total_ms}ms")
        } else {
            let secs = total_ms as f64 / 1000.0;
            format!("{secs:.2}s")
        }
    };

    let in_transit = move || pg.in_flight.get().len();

    let pg_flows = pg.clone();
    let active_flow_count = move || pg_flows.flows.get().iter().filter(|f| f.active).count();

    view! {
        <header class="header-bar">
            <div class="header-left">
                <img src="marque-white.png" alt="IOG" class="header-logo" />
                <span class="header-title">"NetSim Playground"</span>
            </div>
            <div class="header-stats">
                <div class="header-stat">
                    <span class="header-stat-label">"Sim Time"</span>
                    <span class="header-stat-value">{sim_time_display}</span>
                </div>
                <div class="header-stat">
                    <span class="header-stat-label">"In Transit"</span>
                    <span class="header-stat-value">{in_transit}</span>
                </div>
                <div class="header-stat">
                    <span class="header-stat-label">"Flows"</span>
                    <span class="header-stat-value">{active_flow_count}</span>
                </div>
            </div>
        </header>
    }
}
