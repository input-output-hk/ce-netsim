use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::{components::*, path};

// Modules
mod components;
mod pages;
mod simulation;

use crate::pages::playground::PlaygroundPage;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Html attr:lang="en" attr:dir="ltr" attr:data-theme="dark" />

        <Title text="NetSim Playground" />

        <Meta charset="UTF-8" />
        <Meta name="viewport" content="width=device-width, initial-scale=1.0" />

        <Router base=option_env!("LEPTOS_BASE_PATH").unwrap_or_default()>
            <Routes fallback=|| view! { "Not Found" }>
                <Route path=path!("/") view=PlaygroundPage />
            </Routes>
        </Router>
    }
}
