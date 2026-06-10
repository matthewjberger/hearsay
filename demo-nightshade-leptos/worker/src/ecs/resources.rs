use nightshade::prelude::Entity;

/// Example resource. Replace, rename, or add to this list as your game
/// grows. Resources are global per-app state that systems read and mutate.
#[derive(Default)]
pub struct ExampleState {
    pub cubes: Vec<Entity>,
}

/// The currently selected engine entity.
#[derive(Default)]
pub struct Selection {
    pub selected: Option<Entity>,
}

/// Whether a GPU pick is in flight.
#[derive(Default)]
pub struct Picking {
    pub pending: bool,
}
