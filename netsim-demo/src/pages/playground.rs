use crate::components::canvas::Canvas;
use crate::components::controls::Controls;
use crate::components::header::Header;
use crate::components::transit_table::TransitTable;
use crate::simulation::Playground;
use leptos::prelude::*;

#[component]
pub fn PlaygroundPage() -> impl IntoView {
    let pg = Playground::new();
    provide_context(pg.clone());

    // Single interval that ticks the simulation when playing
    let pg_tick = pg.clone();
    let _handle = set_interval_with_handle(
        move || {
            if pg_tick.playing.get_untracked() {
                pg_tick.step();
            }
        },
        std::time::Duration::from_millis(50),
    );

    view! {
        <div class="playground">
            <Header />
            <Canvas />
            <Controls />
            <TransitTable />
        </div>
    }
}
