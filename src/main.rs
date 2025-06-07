use bevy::{diagnostic::FrameTimeDiagnosticsPlugin, prelude::*, window::WindowResolution};
use minecraft::*;

fn main()
{
    let mut app = App::new();

    // ImagePlugin is modified to use nearest filtering for pixelated textures.
    app.add_plugins((
        DefaultPlugins.set(ImagePlugin::default_nearest()),
        FrameTimeDiagnosticsPlugin::default(),
    ));

    app.init_state::<GameState>();

    app.init_resource::<BlockList>();
    app.insert_resource(Map::new(0xDE1FA234));
    app.init_resource::<ChunkMap>();
    app.init_resource::<ChunkLoadState>();
    app.init_resource::<ChunkMeshState>();
    app.add_systems(Startup, load_block_types.after(setup)); // World systems - only run when in game (not when paused)
    app.add_systems(
        Update,
        (manage_chunk_loading, process_chunk_tasks, process_chunk_mesh_tasks)
            .chain()
            .run_if(in_state(GameState::InGame)),
    );
    app.add_systems(Update, fly_camera_movement.run_if(in_state(GameState::InGame)));
    // app.add_systems(Update, count_chunks);
    app.add_systems(Update, mouse_look.run_if(in_state(GameState::InGame)));
    app.add_systems(
        Update,
        (block_interaction, trigger_chunk_remeshing, remesh_changed_chunks)
            .chain()
            .run_if(in_state(GameState::InGame)),
    );
    app.add_systems(OnEnter(GameState::MainMenu), on_enter_main_menu);
    app.add_systems(OnExit(GameState::MainMenu), on_exit_main_menu);
    app.add_systems(OnEnter(GameState::InGame), on_enter_in_game);
    app.add_systems(OnEnter(GameState::Paused), on_enter_pause_menu);
    app.add_systems(OnExit(GameState::Paused), on_exit_pause_menu);

    app.add_systems(Update, text_update_system.run_if(in_state(GameState::InGame)));
    app.add_systems(Update, button_system);
    app.add_systems(Update, keyboard_input_system);

    app.add_systems(Startup, setup);

    app.run();
}

// The setup system creates some global features.
fn setup(mut commands: Commands, mut window: Single<&mut Window>, assets: Res<AssetServer>)
{
    // Load block textures.
    commands.insert_resource(TextureHandles {
        grass_side: assets.load("textures/grass_side.png"),
        grass_top: assets.load("textures/grass_top.png"),
        dirt: assets.load("textures/dirt.png"),
        stone: assets.load("textures/stone.png"),
        sand: assets.load("textures/sand.png"),
        clay: assets.load("textures/clay.png"),
        gravel: assets.load("textures/gravel.png"),
        oak_log_inside: assets.load("textures/oak_log_inside.png"),
        oak_log_outside: assets.load("textures/oak_log_outside.png"),
        oak_leaves: assets.load("textures/oak_leaves.png"),
        water: assets.load("textures/water.png"),
    });
    // Default background color for main menu (dark background).
    commands.insert_resource(ClearColor(Color::srgb(0.2, 0.2, 0.2)));

    // Setup global 2D camera for UI with a marker component.
    commands.spawn((UICamera, Camera2d::default(), Camera { order: 1, ..default() }));

    // Set the window resolution.
    window.resolution = WindowResolution::new(1920.0, 1080.0);
}
