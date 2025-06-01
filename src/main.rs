use bevy::{
    diagnostic::FrameTimeDiagnosticsPlugin,
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow},
};
use minecraft::*;

fn main()
{
    let mut app = App::new();

    // ImagePlugin is modified to use nearest filtering for pixelated textures.
    app.add_plugins((
        DefaultPlugins.set(ImagePlugin::default_nearest()),
        FrameTimeDiagnosticsPlugin::default(),
    ));

    app.init_resource::<BlockList>();
    app.init_resource::<Map>();
    app.init_resource::<ChunkMap>();
    app.init_resource::<ChunkLoadState>();
    app.init_resource::<ChunkMeshState>();
    app.add_systems(Startup, load_block_types.after(setup));
    app.add_systems(
        Update,
        (manage_chunk_loading, process_chunk_tasks, process_chunk_mesh_tasks).chain(),
    );
    app.add_systems(Update, fly_camera_movement);
    // app.add_systems(Update, count_chunks);
    app.add_systems(Update, mouse_look);
    app.add_systems(Update, (block_interaction, remesh_changed_chunks).chain());

    app.add_systems(Update, text_update_system);

    app.add_systems(Startup, setup);

    app.run();
}

// The setup system creates some global features.
fn setup(
    mut commands: Commands,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    assets: Res<AssetServer>,
)
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

    // Blue sky.
    commands.insert_resource(ClearColor(Color::srgb(0.53, 0.81, 0.92)));

    // Ambient light.
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 100.0,
        affects_lightmapped_meshes: true,
    });

    // 3D camera.
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(30.0, 100.0, 80.0).looking_at(Vec3::ZERO, Vec3::Y),
        FlyCam { speed: 12.0, sprint_mult: 2.0, sensitivity: 0.002, yaw: 0.0, pitch: 0.0 },
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

    // Main light (the sun).
    commands.spawn((
        DirectionalLight { shadows_enabled: true, illuminance: 10000.0, ..default() },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::ZYX,
            0.0,
            std::f32::consts::FRAC_PI_4,
            -std::f32::consts::FRAC_PI_4,
        )),
    ));

    // Setup UI elements
    setup_ui(commands, assets);

    // Lock cursor position.
    for mut window in windows.iter_mut()
    {
        window.cursor_options.visible = false;
        window.cursor_options.grab_mode = CursorGrabMode::Locked;
    }
}
