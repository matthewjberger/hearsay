use crate::ecs::TemplateWorld;
use nightshade::prelude::*;
use protocol::WorkerMessage;

const SPIN_RADIANS_PER_SECOND: f32 = 0.8;
const RING_RADIUS: f32 = 3.0;
const GOLDEN_ANGLE_RADIANS: f32 = 2.399_963;

/// Example system. Each system is a free function that takes
/// `&mut TemplateWorld` for app-specific state and `&mut World` for the
/// engine's renderer-visible world. Add more files in `src/systems/` and
/// register them in `src/systems.rs` to grow your game.
///
/// This one spins every spawned cube and spawns another on Space.
pub fn tick(template_world: &mut TemplateWorld, world: &mut World) {
    let delta_time = world.resources.window.timing.delta_time;
    let spin = nalgebra_glm::quat_angle_axis(SPIN_RADIANS_PER_SECOND * delta_time, &Vec3::y());
    for index in 0..template_world.resources.example.cubes.len() {
        let cube = template_world.resources.example.cubes[index];
        if let Some(transform) = world.core.get_local_transform_mut(cube) {
            transform.rotation = spin * transform.rotation;
        }
        mark_local_transform_dirty(world, cube);
    }

    let events = std::mem::take(&mut world.resources.input.events);
    for event in events {
        if let AppEvent::Keyboard { key, state } = event
            && matches!((key, state), (KeyCode::Space, KeyState::Pressed))
        {
            spawn_cube(template_world, world);
        }
    }
}

/// Spawns a cube on a ring around the origin, names it, and reports the new
/// count to the page.
pub fn spawn_cube(template_world: &mut TemplateWorld, world: &mut World) {
    let count = template_world.resources.example.cubes.len() as u32;
    let position = if count == 0 {
        Vec3::new(0.0, 0.5, 0.0)
    } else {
        let angle = count as f32 * GOLDEN_ANGLE_RADIANS;
        Vec3::new(angle.cos() * RING_RADIUS, 0.5, angle.sin() * RING_RADIUS)
    };
    let cube = spawn_cube_at(world, position);
    world.core.set_name(cube, Name(format!("Cube {count}")));
    template_world.resources.example.cubes.push(cube);
    crate::post(&WorkerMessage::CubeCount { count: count + 1 });
}
