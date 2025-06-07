use bevy::{prelude::*, window::CursorGrabMode};

use crate::{
    camera::FlyCam,
    get_semitransparent_panel_height, get_semitransparent_panel_width,
    types::RENDER_DISTANCE,
    ui::{MainMenuButton, PlayButton, QuitButton, ResumeButton, create_button, setup_ingame_ui},
};

// Resource to preserve camera state when pausing.
#[derive(Resource)]
pub struct PreservedCameraState
{
    pub transform: Transform,
    pub flycam: FlyCam,
}

#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum GameState
{
    #[default]
    MainMenu,
    InGame,
    Paused,
}

// Components to mark UI elements for different states.
#[derive(Component)]
pub struct MainMenuUI;

#[derive(Component)]
pub struct InGameUI;

#[derive(Component)]
pub struct PauseMenuUI;

// Components to mark the 3D camera and lighting for the game.
#[derive(Component)]
pub struct GameCamera;

#[derive(Component)]
pub struct GameLighting;

// Component to mark the global UI camera.
#[derive(Component)]
pub struct UICamera;

pub fn on_enter_main_menu(
    mut commands: Commands,
    mut window: Single<&mut Window>,
    mut clear_color: ResMut<ClearColor>,
    assets: Res<AssetServer>,
    in_game_query: Query<Entity, With<InGameUI>>,
    camera_query: Query<Entity, With<GameCamera>>,
    lighting_query: Query<Entity, With<GameLighting>>,
    pause_menu_query: Query<Entity, With<PauseMenuUI>>,
)
{
    // Clean up any existing game elements.
    for entity in &in_game_query
    {
        commands.entity(entity).despawn();
    }

    // Despawn game camera.
    for entity in &camera_query
    {
        commands.entity(entity).despawn();
    }

    // Despawn game lighting.
    for entity in &lighting_query
    {
        commands.entity(entity).despawn();
    }

    // Despawn pause menu if it exists.
    for entity in &pause_menu_query
    {
        commands.entity(entity).despawn();
    }

    // Remove preserved camera state.
    commands.remove_resource::<PreservedCameraState>();

    // Show cursor and unlock it.
    window.cursor_options.visible = true;
    window.cursor_options.grab_mode = CursorGrabMode::None;

    // Set background to dark gray for main menu.
    clear_color.0 = Color::srgb(0.2, 0.2, 0.2);

    // Spawn main menu UI.
    commands
        .spawn((
            MainMenuUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn(create_button("Play", PlayButton, &assets));
            parent.spawn(create_button("Quit", QuitButton, &assets));
        });
}

pub fn on_exit_main_menu(mut commands: Commands, main_menu_query: Query<Entity, With<MainMenuUI>>)
{
    // Despawn all main menu UI elements
    for entity in &main_menu_query
    {
        commands.entity(entity).despawn();
    }
}

pub fn on_enter_in_game(
    mut commands: Commands,
    mut window: Single<&mut Window>,
    mut clear_color: ResMut<ClearColor>,
    assets: Res<AssetServer>,
    existing_camera_query: Query<Entity, With<GameCamera>>,
    existing_ui_query: Query<Entity, With<InGameUI>>,
)
{
    // Hide cursor and lock it for FPS controls.
    window.cursor_options.visible = false;
    window.cursor_options.grab_mode = CursorGrabMode::Locked;

    // Change background to blue sky.
    clear_color.0 = Color::srgb(0.53, 0.81, 0.92);

    // Only spawn game elements if they don't already exist.
    if existing_camera_query.is_empty()
    {
        // Spawn ambient light for the game.
        commands.spawn((
            GameLighting,
            AmbientLight {
                color: Color::WHITE,
                brightness: 100.0,
                affects_lightmapped_meshes: true,
            },
        ));

        // Spawn 3D camera for the game.
        commands.spawn((
            GameCamera,
            Camera3d::default(),
            Transform::from_xyz(30.0, 100.0, 80.0).looking_at(Vec3::ZERO, Vec3::Y),
            FlyCam { speed: 12.0, sprint_mult: 20.0, sensitivity: 0.002, yaw: 0.0, pitch: 0.0 },
            Camera { order: 0, ..default() },
            DistanceFog {
                color: Color::srgb(0.53, 0.81, 0.92),
                falloff: FogFalloff::Linear {
                    start: (RENDER_DISTANCE - 2) as f32 * 16.0,
                    end: RENDER_DISTANCE as f32 * 16.0,
                },
                ..Default::default()
            },
        ));

        // Spawn main light (the sun) for the game.
        commands.spawn((
            GameLighting,
            DirectionalLight { shadows_enabled: true, illuminance: 10000.0, ..default() },
            Transform::from_rotation(Quat::from_euler(
                EulerRot::ZYX,
                0.0,
                std::f32::consts::FRAC_PI_4,
                -std::f32::consts::FRAC_PI_4,
            )),
        ));
    }

    // Only setup in-game UI if it doesn't already exist.
    if existing_ui_query.is_empty()
    {
        // Setup in-game UI (crosshair and FPS counter).
        setup_ingame_ui(commands, assets);
    }
}

pub fn on_enter_pause_menu(
    mut commands: Commands,
    mut window: Single<&mut Window>,
    assets: Res<AssetServer>,
    camera_query: Query<(&Transform, &FlyCam), With<GameCamera>>,
    in_game_ui_query: Query<Entity, With<InGameUI>>,
)
{
    // Preserve camera state.
    if let Ok((transform, flycam)) = camera_query.single()
    {
        commands.insert_resource(PreservedCameraState {
            transform: *transform,
            flycam: flycam.clone(),
        });
    }

    // Hide in-game UI elements (crosshair, FPS counter).
    for entity in &in_game_ui_query
    {
        commands.entity(entity).insert(Visibility::Hidden);
    }

    // Show cursor for menu navigation (but keep the game world visible).
    window.cursor_options.visible = true;
    window.cursor_options.grab_mode = CursorGrabMode::None;

    // Spawn pause menu UI without a background color to keep game visible.
    commands
        .spawn((
            PauseMenuUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            // Add a semi-transparent panel behind the buttons for better visibility.
            parent
                .spawn((
                    Node {
                        width: Val::Px(get_semitransparent_panel_width(1)),
                        height: Val::Px(get_semitransparent_panel_height(2)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(10.0),
                        padding: UiRect::all(Val::Px(20.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|panel| {
                    panel.spawn(create_button("Resume", ResumeButton, &assets));
                    panel.spawn(create_button("Main Menu", MainMenuButton, &assets));
                });
        });
}

pub fn on_exit_pause_menu(
    mut commands: Commands,
    pause_menu_query: Query<Entity, With<PauseMenuUI>>,
    mut window: Single<&mut Window>,
    in_game_ui_query: Query<Entity, With<InGameUI>>,
    preserved_camera: Option<Res<PreservedCameraState>>,
    mut camera_query: Query<(&mut Transform, &mut FlyCam), With<GameCamera>>,
)
{
    // Restore camera state if we have it preserved.
    if let Some(preserved) = preserved_camera
    {
        if let Ok((mut transform, mut flycam)) = camera_query.single_mut()
        {
            *transform = preserved.transform;
            *flycam = preserved.flycam.clone();
        }
    }

    // Show in-game UI elements again (crosshair, FPS counter).
    for entity in &in_game_ui_query
    {
        commands.entity(entity).insert(Visibility::Visible);
    }

    // Hide cursor and lock it again.
    window.cursor_options.visible = false;
    window.cursor_options.grab_mode = CursorGrabMode::Locked;

    // Despawn all pause menu UI elements.
    for entity in &pause_menu_query
    {
        commands.entity(entity).despawn();
    }
}

// Keyboard input system for pause/resume.
pub fn keyboard_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    current_state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
)
{
    if keyboard.just_pressed(KeyCode::Escape)
    {
        match current_state.get()
        {
            GameState::InGame =>
            {
                next_state.set(GameState::Paused);
            },
            GameState::Paused =>
            {
                next_state.set(GameState::InGame);
            },
            _ =>
            {},
        }
    }
}
