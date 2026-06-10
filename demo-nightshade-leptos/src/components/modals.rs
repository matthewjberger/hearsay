use leptos::prelude::*;

use crate::state::{DemoState, ModalRequest};

fn resolve(state: DemoState, modal: &ModalRequest, confirmed: bool) {
    let panel = state
        .panels
        .get_untracked()
        .into_iter()
        .find(|panel| panel.id == modal.panel_id);
    if let Some(panel) = panel {
        panel.last_modal_result.set(Some(confirmed));
        panel.modal_open.set(false);
    }
    let modal_id = modal.id;
    state
        .modals
        .update(|modals| modals.retain(|modal| modal.id != modal_id));
}

/// The confirm modal stack. Each request renders a centered card over a
/// backdrop; the buttons resolve back to the requesting panel.
#[component]
pub fn Modals(state: DemoState) -> impl IntoView {
    view! {
        <For
            each=move || state.modals.get()
            key=|modal| modal.id
            children=move |modal| {
                let confirm_modal = modal.clone();
                let cancel_modal = modal.clone();
                view! {
                    <div class="modal-backdrop">
                        <div class="modal-card">
                            <div class="modal-title">{modal.title.clone()}</div>
                            <div class="modal-body">{modal.body.clone()}</div>
                            <div class="hearsay-button-row">
                                <button
                                    class="hud-button"
                                    on:click=move |_| resolve(state, &cancel_modal, false)
                                >
                                    {modal.cancel_text.clone()}
                                </button>
                                <button
                                    class="hud-button modal-confirm"
                                    on:click=move |_| resolve(state, &confirm_modal, true)
                                >
                                    {modal.confirm_text.clone()}
                                </button>
                            </div>
                        </div>
                    </div>
                }
            }
        />
    }
}
