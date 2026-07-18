use bevy::{
    input::{ButtonState, keyboard::KeyboardInput},
    prelude::*,
    window::CursorGrabMode,
};

use crate::{
    ButtonAction, Chunk, ChunkLoadState, ChunkMap, ChunkMeshQueue, ChunkMeshState, Map,
    camera::FlyCam,
    saves,
    types::RENDER_DISTANCE,
    ui::{create_button, setup_ingame_ui},
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
    NewGameMenu,
    LoadGameMenu,
    Settings,
    LoadingScreen,
    InGame,
    Paused,
}

#[derive(Resource)]
pub enum PendingGameLoad
{
    NewGame,
    LoadGame(String),
}

// This resource stacks the previous game states so we can navigate back.
#[derive(Resource)]
pub struct MenuStack(pub Vec<GameState>);

impl Default for MenuStack
{
    fn default() -> Self
    {
        return Self(vec![GameState::MainMenu]);
    }
}

// Components to mark UI elements for different states.

#[derive(Component)]
pub struct LoadingScreenUI;

#[derive(Component)]
pub struct LoadingProgressBar;

#[derive(Component)]
pub struct MainMenuUI;

#[derive(Component)]
pub struct NewGameUI;

#[derive(Component)]
pub struct LoadGameUI;

#[derive(Component)]
pub struct WorldNameText;

#[derive(Resource, Default)]
pub struct WorldNameInput(pub String);

#[derive(Component)]
pub struct SettingsUI;

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

const MENU_BACKGROUND_COLOR: Color = Color::srgb(0.2, 0.2, 0.2);

pub fn on_enter_main_menu(
    mut commands: Commands,
    mut window: Single<&mut Window>,
    mut clear_color: ResMut<ClearColor>,
    assets: Res<AssetServer>,
    in_game_query: Query<Entity, With<InGameUI>>,
    camera_query: Query<Entity, With<GameCamera>>,
    lighting_query: Query<Entity, With<GameLighting>>,
    chunk_query: Query<Entity, With<Chunk>>,
    mut chunk_map: ResMut<ChunkMap>,
    mut chunk_load_state: ResMut<ChunkLoadState>,
    mut chunk_mesh_state: ResMut<ChunkMeshState>,
    mut chunk_mesh_queue: ResMut<ChunkMeshQueue>,
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

    // Despawn all chunks.
    for entity in &chunk_query
    {
        commands.entity(entity).despawn();
    }

    // Reset world generation state.
    chunk_map.loaded_chunks.clear();
    chunk_load_state.tasks.clear();
    chunk_load_state.last_player_chunk = None;
    chunk_load_state.sorted_desired.clear();
    chunk_load_state.desired_set.clear();
    chunk_mesh_state.tasks.clear();
    chunk_mesh_queue.queue.clear();

    // Remove game map and preserved camera.
    commands.remove_resource::<Map>();
    commands.remove_resource::<PreservedCameraState>();

    // Show cursor and unlock it.
    window.cursor_options.visible = true;
    window.cursor_options.grab_mode = CursorGrabMode::None;

    // Set background to dark gray for main menu.
    clear_color.0 = MENU_BACKGROUND_COLOR;

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
            parent.spawn(create_button("Create a new World", ButtonAction::NewWorld, &assets));
            parent.spawn(create_button("Load a World", ButtonAction::LoadWorld, &assets));
            parent.spawn(create_button("Settings", ButtonAction::Settings, &assets));
            parent.spawn(create_button("Quit", ButtonAction::Quit, &assets));
        });
}

pub fn on_exit_main_menu(mut commands: Commands, main_menu_query: Query<Entity, With<MainMenuUI>>)
{
    for entity in &main_menu_query
    {
        commands.entity(entity).despawn();
    }
}

pub fn on_enter_new_game(mut commands: Commands, assets: Res<AssetServer>, mut menu_stack: ResMut<MenuStack>)
{
    if menu_stack.0.last() != Some(&GameState::NewGameMenu)
    {
        menu_stack.0.push(GameState::NewGameMenu);
    }

    commands.insert_resource(WorldNameInput(String::new()));

    commands
        .spawn((
            NewGameUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(MENU_BACKGROUND_COLOR),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Enter World Name:"),
                TextFont { font: assets.load("fonts/minecraft.otf"), font_size: 40.0, ..default() },
                TextColor(Color::WHITE),
            ));

            parent.spawn((
                WorldNameText,
                Text::new("_"),
                TextFont { font: assets.load("fonts/minecraft.otf"), font_size: 40.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 0.0)),
            ));

            parent.spawn(create_button("Create", ButtonAction::CreateGame, &assets));
            parent.spawn(create_button("Back", ButtonAction::Back, &assets));
        });
}

pub fn handle_world_name_input(
    mut evr_kbd: EventReader<KeyboardInput>,
    mut input: ResMut<WorldNameInput>,
    mut text_query: Query<&mut Text, With<WorldNameText>>,
)
{
    for ev in evr_kbd.read()
    {
        if ev.state == ButtonState::Pressed
        {
            if let bevy::input::keyboard::Key::Character(ref c) = ev.logical_key
            {
                if c.len() == 1 && input.0.len() < 20
                {
                    input.0.push_str(c);
                }
            }
            else if ev.key_code == KeyCode::Backspace
            {
                input.0.pop();
            }
            else if ev.key_code == KeyCode::Space
            {
                if input.0.len() < 20
                {
                    input.0.push(' ');
                }
            }
        }
    }

    if let Ok(mut text) = text_query.single_mut()
    {
        text.0 = input.0.clone();
        if text.0.is_empty()
        {
            text.0 = "_".to_string();
        }
    }
}

pub fn on_exit_new_game(mut commands: Commands, query: Query<Entity, With<NewGameUI>>)
{
    for entity in &query
    {
        commands.entity(entity).despawn();
    }
}

pub fn on_enter_load_game(mut commands: Commands, assets: Res<AssetServer>, mut menu_stack: ResMut<MenuStack>)
{
    if menu_stack.0.last() != Some(&GameState::LoadGameMenu)
    {
        menu_stack.0.push(GameState::LoadGameMenu);
    }

    let saves = saves::get_saves();

    commands
        .spawn((
            LoadGameUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(MENU_BACKGROUND_COLOR),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Select a World"),
                TextFont { font: assets.load("fonts/minecraft.otf"), font_size: 40.0, ..default() },
                TextColor(Color::WHITE),
            ));

            for save in saves
            {
                parent.spawn(create_button(
                    &save.world_name,
                    ButtonAction::LoadSpecificGame(save.file_name.clone()),
                    &assets,
                ));
            }

            parent.spawn(create_button("Back", ButtonAction::Back, &assets));
        });
}

pub fn on_exit_load_game(mut commands: Commands, query: Query<Entity, With<LoadGameUI>>)
{
    for entity in &query
    {
        commands.entity(entity).despawn();
    }
}

pub fn on_enter_settings(
    mut commands: Commands,
    mut window: Single<&mut Window>,
    assets: Res<AssetServer>,
    mut menu_stack: ResMut<MenuStack>,
)
{
    if menu_stack.0.last() != Some(&GameState::Settings)
    {
        menu_stack.0.push(GameState::Settings);
    }

    // Show cursor and unlock it.
    window.cursor_options.visible = true;
    window.cursor_options.grab_mode = CursorGrabMode::None;

    // Spawn settings menu UI with full grey overlay.
    commands
        .spawn((
            SettingsUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(MENU_BACKGROUND_COLOR),
        ))
        .with_children(|parent| {
            parent.spawn(create_button("Back", ButtonAction::Back, &assets));
        });
}

pub fn on_exit_settings(mut commands: Commands, settings_menu_query: Query<Entity, With<SettingsUI>>)
{
    // Despawn all settings menu UI elements.
    for entity in &settings_menu_query
    {
        commands.entity(entity).despawn();
    }
}

pub fn on_enter_loading_screen(
    mut commands: Commands,
    mut window: Single<&mut Window>,
    mut clear_color: ResMut<ClearColor>,
    existing_camera_query: Query<Entity, With<GameCamera>>,
    mut menu_stack: ResMut<MenuStack>,
    pending_load: Option<Res<PendingGameLoad>>,
    world_name_input: Option<Res<WorldNameInput>>,
    existing_map: Option<Res<Map>>,
)
{
    if menu_stack.0.last() != Some(&GameState::LoadingScreen)
    {
        menu_stack.0.push(GameState::LoadingScreen);
    }

    // Hide cursor and lock it for FPS controls.
    window.cursor_options.visible = false;
    window.cursor_options.grab_mode = CursorGrabMode::Locked;

    // Change background to blue sky.
    clear_color.0 = Color::srgb(0.53, 0.81, 0.92);

    let mut start_pos = [0.0, 100.0, 0.0];
    let mut start_yaw = 0.0;
    let mut start_pitch = 0.0;

    if let Some(map) = &existing_map
    {
        start_pos = map.player_position;
        start_yaw = map.player_yaw;
        start_pitch = map.player_pitch;
    }

    if let Some(load) = pending_load
    {
        match *load
        {
            PendingGameLoad::NewGame =>
            {
                let name = world_name_input
                    .map(|r| r.0.clone())
                    .unwrap_or_else(|| "World".to_string());
                let final_name = if name.is_empty() { "World".to_string() } else { name };
                let seed = rand::random::<u32>();
                let file_name = saves::generate_new_save_file_name();
                let map = Map::new(seed, final_name, file_name);
                start_pos = map.player_position;
                start_yaw = map.player_yaw;
                start_pitch = map.player_pitch;
                commands.insert_resource(map);
            },
            PendingGameLoad::LoadGame(ref file_name) =>
            {
                if let Some(map) = saves::load_game(file_name)
                {
                    start_pos = map.player_position;
                    start_yaw = map.player_yaw;
                    start_pitch = map.player_pitch;
                    commands.insert_resource(map);
                }
                else
                {
                    // Fallback if load failed
                    let seed = rand::random::<u32>();
                    let map = Map::new(seed, "Error World".to_string(), "error".to_string());
                    start_pos = map.player_position;
                    start_yaw = map.player_yaw;
                    start_pitch = map.player_pitch;
                    commands.insert_resource(map);
                }
            },
        }
        commands.remove_resource::<PendingGameLoad>();
    }

    // Only spawn game elements if they don't already exist.
    if existing_camera_query.is_empty()
    {
        // Spawn ambient light for the game.
        commands.spawn((
            GameLighting,
            AmbientLight { color: Color::WHITE, brightness: 100.0, affects_lightmapped_meshes: true },
        ));

        // Spawn 3D camera for the game.
        commands.spawn((
            GameCamera,
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection {
                fov: std::f32::consts::FRAC_PI_3, // 60 degrees.
                ..default()
            }),
            Transform {
                translation: Vec3::new(start_pos[0], start_pos[1], start_pos[2]),
                rotation: Quat::from_euler(EulerRot::YXZ, start_yaw, start_pitch, 0.0),
                ..default()
            },
            FlyCam {
                speed: 15.0,
                sprint_mult: 5.0,
                sensitivity: 0.002,
                yaw: start_yaw,
                pitch: start_pitch,
            },
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
            DirectionalLight { shadows_enabled: false, illuminance: 10000.0, ..default() },
            Transform::from_rotation(Quat::from_euler(
                EulerRot::ZYX,
                0.0,
                std::f32::consts::FRAC_PI_4,
                -std::f32::consts::FRAC_PI_4,
            )),
        ));
    }

    // Spawn Loading Screen UI
    commands
        .spawn((
            LoadingScreenUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: Val::Px(400.0),
                        height: Val::Px(40.0),
                        border: UiRect::all(Val::Px(4.0)),
                        ..default()
                    },
                    BorderColor(Color::WHITE),
                ))
                .with_children(|outline| {
                    outline.spawn((
                        LoadingProgressBar,
                        Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
                        BackgroundColor(Color::srgb(0.0, 0.8, 0.0)),
                    ));
                });
        });
}

pub fn update_loading_screen_progress(
    mut next_state: ResMut<NextState<GameState>>,
    mut bar_query: Query<&mut Node, With<LoadingProgressBar>>,
    chunk_query: Query<(), (With<Chunk>, With<Children>)>,
)
{
    let rd = RENDER_DISTANCE as f32;
    let target = (rd * rd * 2.0).min(1000.0);

    let current_chunks = chunk_query.iter().count() as f32;
    let mut progress = current_chunks / target;
    if progress > 1.0
    {
        progress = 1.0;
    }

    for mut node in &mut bar_query
    {
        node.width = Val::Percent(progress * 100.0);
    }

    if progress >= 1.0
    {
        next_state.set(GameState::InGame);
    }
}

pub fn on_exit_loading_screen(mut commands: Commands, loading_screen_query: Query<Entity, With<LoadingScreenUI>>)
{
    for entity in &loading_screen_query
    {
        commands.entity(entity).despawn();
    }
}

pub fn on_enter_in_game(
    commands: Commands,
    mut window: Single<&mut Window>,
    mut clear_color: ResMut<ClearColor>,
    assets: Res<AssetServer>,
    existing_ui_query: Query<Entity, With<InGameUI>>,
    mut menu_stack: ResMut<MenuStack>,
)
{
    if menu_stack.0.last() != Some(&GameState::InGame)
    {
        menu_stack.0.push(GameState::InGame);
    }

    // Hide cursor and lock it for FPS controls.
    window.cursor_options.visible = false;
    window.cursor_options.grab_mode = CursorGrabMode::Locked;

    // Change background to blue sky.
    clear_color.0 = Color::srgb(0.53, 0.81, 0.92);

    // Only setup in-game UI if it doesn't already exist.
    if existing_ui_query.is_empty()
    {
        setup_ingame_ui(commands, assets);
    }
}

pub fn save_on_exit(map: Option<ResMut<Map>>, camera_query: Query<(&Transform, &FlyCam), With<GameCamera>>)
{
    if let Some(mut map) = map
    {
        if let Ok((transform, flycam)) = camera_query.single()
        {
            map.player_position = transform.translation.to_array();
            map.player_yaw = flycam.yaw;
            map.player_pitch = flycam.pitch;
        }
        saves::save_game(&map);
    }
}

pub fn on_exit_in_game(
    mut window: Single<&mut Window>,
    mut commands: Commands,
    camera_query: Query<(&Transform, &FlyCam), With<GameCamera>>,
    map: Option<ResMut<Map>>,
)
{
    // Save the game
    if let Some(mut map) = map
    {
        if let Ok((transform, flycam)) = camera_query.single()
        {
            map.player_position = transform.translation.to_array();
            map.player_yaw = flycam.yaw;
            map.player_pitch = flycam.pitch;
        }
        saves::save_game(&map);
    }

    // Show cursor and unlock it for the menu.
    window.cursor_options.visible = true;
    window.cursor_options.grab_mode = CursorGrabMode::None;

    // Preserve the camera state so we can resume from the same spot.
    if let Ok((transform, flycam)) = camera_query.single()
    {
        commands.insert_resource(PreservedCameraState { transform: *transform, flycam: flycam.clone() });
    }
}

pub fn on_enter_paused(
    mut commands: Commands,
    mut window: Single<&mut Window>,
    assets: Res<AssetServer>,
    mut menu_stack: ResMut<MenuStack>,
)
{
    if menu_stack.0.last() != Some(&GameState::Paused)
    {
        menu_stack.0.push(GameState::Paused);
    }

    // Show cursor and unlock it for the menu.
    window.cursor_options.visible = true;
    window.cursor_options.grab_mode = CursorGrabMode::None;

    // Spawn pause menu UI.
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
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        ))
        .with_children(|parent| {
            parent.spawn(create_button("Resume", ButtonAction::Back, &assets));
            parent.spawn(create_button("Settings", ButtonAction::Settings, &assets));
            parent.spawn(create_button("Main Menu", ButtonAction::MainMenu, &assets));
        });
}

pub fn on_exit_paused(
    mut commands: Commands,
    pause_menu_query: Query<Entity, With<PauseMenuUI>>,
    mut window: Single<&mut Window>,
    next_state: Res<State<GameState>>,
)
{
    // Despawn pause menu elements.
    for entity in &pause_menu_query
    {
        commands.entity(entity).despawn();
    }

    // Only hide the cursor if we are returning to the game.
    if *next_state.get() == GameState::InGame
    {
        window.cursor_options.visible = false;
        window.cursor_options.grab_mode = CursorGrabMode::Locked;
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
