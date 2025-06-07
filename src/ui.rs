use bevy::{
    app::AppExit,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
};

use crate::gamestate::GameState;

const MAIN_FONT: &str = "fonts/minecraft.otf";

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
            TextFont { font: assets.load(MAIN_FONT), font_size: 30.0, ..default() },
        ))
        .with_child((
            // Create a TextSpan that will be updated with the FPS value.
            TextSpan::default(),
            TextFont { font: assets.load(MAIN_FONT), font_size: 30.0, ..Default::default() },
            // Initialize the timer.
            FpsText { timer: Timer::from_seconds(0.5, TimerMode::Repeating) },
        ));
}

// Helper function to create a color from u8 RGBA values.
const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color
{
    Color::srgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0)
}

const BUTTON_COLOR_BASE: Color = rgba(76, 86, 106, 255);
const BUTTON_COLOR_HOVER: Color = rgba(67, 76, 94, 255);
const BUTTON_COLOR_PRESSED: Color = rgba(59, 66, 82, 255);
const BUTTON_BORDER_COLOR: Color = Color::BLACK;
const BUTTON_FONT_COLOR: Color = Color::WHITE;

const BUTTON_FONT_SIZE: f32 = 30.0;
const BUTTON_WIDTH: f32 = 200.0;
const BUTTON_HEIGHT: f32 = 80.0;
const BUTTON_BORDER_SIZE: f32 = 5.0;

const BUTTON_CORNER_RADIUS: f32 = 5.0;

const BUTTON_PADDING: f32 = 10.0;

pub const fn get_semitransparent_panel_width(cols: u8) -> f32
{
    // Calculate the width of a semi-transparent panel based on the number of
    // columns.
    return (cols as f32 * BUTTON_WIDTH) + ((cols as f32 + 1.0) * BUTTON_PADDING);
}

pub const fn get_semitransparent_panel_height(rows: u8) -> f32
{
    // Calculate the height of a semi-transparent panel based on the number of
    // rows.
    return (rows as f32 * BUTTON_HEIGHT) + ((rows as f32 + 1.0) * BUTTON_PADDING);
}

// Marker components to identify which button was pressed.
#[derive(Component)]
pub struct PlayButton;

#[derive(Component)]
pub struct QuitButton;

#[derive(Component)]
pub struct ResumeButton;

#[derive(Component)]
pub struct MainMenuButton;

// Helper function to create a button with a specific marker component.
pub fn create_button<T: Component>(text: &str, marker: T, assets: &Res<AssetServer>)
-> impl Bundle
{
    (
        Button,
        marker,
        Node {
            width: Val::Px(BUTTON_WIDTH),
            height: Val::Px(BUTTON_HEIGHT),
            border: UiRect::all(Val::Px(BUTTON_BORDER_SIZE)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BorderColor(BUTTON_BORDER_COLOR),
        BorderRadius::all(Val::Px(BUTTON_CORNER_RADIUS)),
        BackgroundColor(BUTTON_COLOR_BASE),
        children![(
            Text::new(text),
            TextFont { font: assets.load(MAIN_FONT), font_size: BUTTON_FONT_SIZE, ..default() },
            TextColor(BUTTON_FONT_COLOR),
            TextShadow::default(),
        )],
    )
}

// Setup function for creating in-game UI elements like FPS counter and
// crosshair.
pub fn setup_ingame_ui(mut commands: Commands, assets: Res<AssetServer>)
{
    // Crosshair.
    commands.spawn((
        crate::gamestate::InGameUI,
        Sprite::from_image(assets.load("textures/crosshair.png")),
        Transform::from_translation(Transform::IDENTITY.translation + Vec3::new(0.0, 0.0, 1.0)),
    ));

    // FPS counter.
    commands
        .spawn((
            crate::gamestate::InGameUI,
            Text::new("FPS: "),
            TextFont { font: assets.load(MAIN_FONT), font_size: 30.0, ..default() },
        ))
        .with_child((
            TextSpan::default(),
            TextFont { font: assets.load(MAIN_FONT), font_size: 30.0, ..Default::default() },
            FpsText { timer: Timer::from_seconds(0.5, TimerMode::Repeating) },
        ));
}

pub fn button_system(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor, &Children),
        (Changed<Interaction>, With<Button>),
    >,
    play_button_query: Query<&Interaction, (With<PlayButton>, Changed<Interaction>)>,
    quit_button_query: Query<&Interaction, (With<QuitButton>, Changed<Interaction>)>,
    resume_button_query: Query<&Interaction, (With<ResumeButton>, Changed<Interaction>)>,
    main_menu_button_query: Query<&Interaction, (With<MainMenuButton>, Changed<Interaction>)>,
    mut next_state: ResMut<NextState<GameState>>,
    mut app_exit_events: EventWriter<AppExit>,
)
{
    // Handle visual button states.
    for (interaction, mut color, mut border_color, ..) in &mut interaction_query
    {
        match *interaction
        {
            Interaction::Pressed =>
            {
                *color = BUTTON_COLOR_PRESSED.into();
                border_color.0 = BUTTON_BORDER_COLOR;
            },
            Interaction::Hovered =>
            {
                *color = BUTTON_COLOR_HOVER.into();
                border_color.0 = BUTTON_BORDER_COLOR;
            },
            Interaction::None =>
            {
                *color = BUTTON_COLOR_BASE.into();
                border_color.0 = BUTTON_BORDER_COLOR;
            },
        }
    }

    // Handle Play button press (Main Menu -> In Game).
    for interaction in &play_button_query
    {
        if *interaction == Interaction::Pressed
        {
            next_state.set(GameState::InGame);
        }
    } // Handle Quit button press.
    for interaction in &quit_button_query
    {
        if *interaction == Interaction::Pressed
        {
            app_exit_events.write(AppExit::Success);
        }
    }

    // Handle Resume button press (Pause -> In Game).
    for interaction in &resume_button_query
    {
        if *interaction == Interaction::Pressed
        {
            next_state.set(GameState::InGame);
        }
    }

    // Handle Main Menu button press (Pause -> Main Menu).
    for interaction in &main_menu_button_query
    {
        if *interaction == Interaction::Pressed
        {
            next_state.set(GameState::MainMenu);
        }
    }
}
