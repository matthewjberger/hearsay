use leptos::prelude::*;

use crate::state::DemoState;

/// A centered card with a spinner until the worker reports the renderer is
/// ready.
#[component]
pub fn Loader(state: DemoState) -> impl IntoView {
    view! {
        <Show when=move || !state.ready.get() fallback=|| ()>
            <div class="loader-overlay">
                <div class="loader-card">
                    <span class="spinner"></span>
                    "Starting the rendererâ€¦"
                </div>
            </div>
        </Show>
    }
}
