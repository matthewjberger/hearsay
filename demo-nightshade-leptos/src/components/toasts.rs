use leptos::prelude::*;

use crate::state::DemoState;

/// The toast stack, bottom right, auto-dismissed by `DemoState::push_toast`.
#[component]
pub fn Toasts(state: DemoState) -> impl IntoView {
    view! {
        <div class="toast-stack">
            <For
                each=move || state.toasts.get()
                key=|toast| toast.id
                children=|toast| {
                    view! { <div class=toast.kind.class()>{toast.text.clone()}</div> }
                }
            />
        </div>
    }
}
