use crate::prelude::*;

#[derive(Resource, Default)]
pub struct FpsCounter {
    fps: f32,
    frame_count: u32,
    timer: f32,
}

pub struct FpsPlugin;

impl Plugin for FpsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FpsCounter>()
            .add_systems(Update, update_fps)
            .add_systems(PreUpdate, display_fps.after(EguiPreUpdateSet::InitContexts));
    }
}

fn update_fps(time: Res<Time>, mut fps_counter: ResMut<FpsCounter>) {
    fps_counter.frame_count += 1;
    fps_counter.timer += time.delta_secs();
    if fps_counter.timer >= 0.5 {
        fps_counter.fps = fps_counter.frame_count as f32 / fps_counter.timer;
        fps_counter.frame_count = 0;
        fps_counter.timer = 0.0;
    }
}

fn display_fps(
    fps_counter: Res<FpsCounter>,
    mut contexts: Query<&mut EguiContext, With<bevy::window::PrimaryWindow>>,
) {
    let Ok(mut context) = contexts.get_single_mut() else {
        return;
    };
    context.get_mut().data_mut(|data| {
        data.insert_temp(egui::Id::new("fps_counter"), fps_counter.fps);
    });
}
