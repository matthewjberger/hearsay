use crate::ecs::TemplateWorld;
use crate::systems;
use nightshade::prelude::*;

/// The application root. Holds your user-side ECS world (`TemplateWorld`)
/// and forwards each `State` hook to system functions in `src/systems/`.
#[derive(Default)]
pub struct Template {
    pub template_world: TemplateWorld,
}

impl State for Template {
    fn initialize(&mut self, world: &mut World) {
        systems::setup::initialize(&mut self.template_world, world);
    }

    fn run_systems(&mut self, world: &mut World) {
        pan_orbit_camera_system(world);
        systems::picking::apply(&mut self.template_world, world);
        systems::example::tick(&mut self.template_world, world);
    }
}
