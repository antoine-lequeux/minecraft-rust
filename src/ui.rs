use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
};

// Marker struct to help identify the FPS UI component, since there may be many
// Text components.
#[derive(Component)]
pub struct FpsText
{
    pub timer: Timer,
}

// This systems periodically updates the FPS text in the UI.
pub fn text_update_system(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<(&mut TextSpan, &mut FpsText)>,
    time: Res<Time>,
)
{
    for (mut span, mut fps_text) in &mut query
    {
        // Only update the counter if the timer period has just ended.
        if fps_text.timer.tick(time.delta()).just_finished()
        {
            // Get the FPS diagnostic.
            if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS)
            {
                // Get the smoothed FPS value.
                if let Some(value) = fps.smoothed()
                {
                    // Update the text.
                    **span = format!("{value:.0}");
                }
            }
        }
    }
}

// Setup function for creating UI elements like FPS counter and crosshair.
pub fn setup_ui(mut commands: Commands, assets: Res<AssetServer>)
{
    // 2D camera for the UI.
    commands.spawn((Camera2d::default(), Camera { order: 1, ..default() }));

    // Crosshair.
    commands.spawn((
        Sprite::from_image(assets.load("textures/crosshair.png")),
        Transform::from_translation(Transform::IDENTITY.translation + Vec3::new(0.0, 0.0, 1.0)),
    ));

    // FPS counter.
    commands
        .spawn((
            // Create a Text with multiple possible spans.
            Text::new("FPS: "),
            TextFont { font: assets.load("fonts/minecraft.otf"), font_size: 30.0, ..default() },
        ))
        .with_child((
            // Create a TextSpan that will be updated with the FPS value.
            TextSpan::default(),
            TextFont {
                font: assets.load("fonts/minecraft.otf"),
                font_size: 30.0,
                ..Default::default()
            },
            // Initialize the timer.
            FpsText { timer: Timer::from_seconds(0.5, TimerMode::Repeating) },
        ));
}
