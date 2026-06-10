use leptos::prelude::*;

use crate::hearsay_link::{self, HearsaySlot};
use crate::state::DemoState;

/// The Api window: compose a broker message on any topic and publish it as
/// text or binary, the browser counterpart of the bevy demo's Api composer.
#[component]
pub fn ApiPanel(link: HearsaySlot, state: DemoState) -> impl IntoView {
    let topic = RwSignal::new("template/text".to_string());
    let payload = RwSignal::new("{\"text\":\"hello\"}".to_string());

    let on_topic = move |event: leptos::ev::Event| topic.set(event_target_value(&event));
    let on_payload = move |event: leptos::ev::Event| payload.set(event_target_value(&event));
    let on_publish_text = move |_| {
        hearsay_link::publish_text(link, &topic.get_untracked(), &payload.get_untracked());
    };
    let on_publish_binary = move |_| {
        hearsay_link::publish_binary(
            link,
            &topic.get_untracked(),
            payload.get_untracked().into_bytes(),
        );
    };
    let on_close = move |_| state.api_visible.set(false);

    view! {
        <Show when=move || state.api_visible.get() fallback=|| ()>
            <div class="api-panel">
                <div class="panel-header">
                    <span class="panel-title">"Api"</span>
                    <button class="panel-close" on:click=on_close>"X"</button>
                </div>
                <div class="panel-dim">
                    "Publish a message on any topic through the broker."
                </div>
                <div class="panel-section">"Topic"</div>
                <input
                    class="hearsay-input"
                    type="text"
                    prop:value=move || topic.get()
                    on:input=on_topic
                />
                <div class="panel-section">"Payload"</div>
                <textarea
                    class="hearsay-input api-payload"
                    prop:value=move || payload.get()
                    on:input=on_payload
                ></textarea>
                <div class="hearsay-button-row">
                    <button
                        class="hud-button"
                        on:click=on_publish_text
                        disabled=move || !state.hearsay_connected.get()
                    >
                        "Publish Text"
                    </button>
                    <button
                        class="hud-button"
                        on:click=on_publish_binary
                        disabled=move || !state.hearsay_connected.get()
                    >
                        "Publish Binary"
                    </button>
                </div>
            </div>
        </Show>
    }
}
