use bevy::{
    diagnostic::FrameTimeDiagnosticsPlugin,
    prelude::*,
    render::{
        RenderPlugin,
        settings::{PowerPreference, RenderCreation, WgpuSettings},
    },
    window::WindowResolution,
};
use mimalloc::MiMalloc;
use minecraft::*;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main()
{
    let mut app = App::new();

    // ImagePlugin is modified to use nearest filtering for pixelated textures.
    app.add_plugins((
        DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    power_preference: PowerPreference::HighPerformance,
                    ..default()
                }),
                ..default()
            }),
        FrameTimeDiagnosticsPlugin::default(),
        MaterialPlugin::<VoxelMaterial>::default(),
    ));

    app.init_state::<GameState>();

    app.init_resource::<BlockList>();
    app.insert_resource(Map::new(0x12345678));
    app.init_resource::<MenuStack>();
    app.init_resource::<ChunkMap>();
    app.init_resource::<ChunkLoadState>();
    app.init_resource::<ChunkMeshState>();
    app.init_resource::<ChunkMeshQueue>();
    app.add_systems(Startup, load_block_types.after(setup)); // World systems - only run when in game (not when paused)
    app.add_systems(
        Update,
        (
            manage_chunk_loading,
            process_chunk_tasks,
            queue_chunk_meshes,
            process_chunk_mesh_tasks,
        )
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
    app.add_systems(OnEnter(GameState::Settings), on_enter_settings);
    app.add_systems(OnExit(GameState::Settings), on_exit_settings);
    app.add_systems(OnEnter(GameState::InGame), on_enter_in_game);
    app.add_systems(OnExit(GameState::InGame), on_exit_in_game);
    app.add_systems(OnEnter(GameState::Paused), on_enter_pause_menu);
    app.add_systems(OnExit(GameState::Paused), on_exit_pause_menu);

    app.add_systems(Update, text_update_system.run_if(in_state(GameState::InGame)));
    app.add_systems(Update, button_system);
    app.add_systems(Update, keyboard_input_system);

    app.add_systems(Startup, setup);

    app.run();
}

// Setup global resources for rendering chunks.
fn setup(
    mut commands: Commands,
    mut window: Single<&mut Window>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
)
{
    let atlas_handle = assets.load("textures/atlas.png");

    let voxel_mat_opaque = VoxelMaterial {
        base: StandardMaterial { alpha_mode: AlphaMode::Opaque, ..default() },
        extension: VoxelMaterialExtension { atlas_texture: atlas_handle.clone() },
    };

    let voxel_mat_transparent = VoxelMaterial {
        base: StandardMaterial { alpha_mode: AlphaMode::Mask(0.5), ..default() },
        extension: VoxelMaterialExtension { atlas_texture: atlas_handle },
    };

    commands.insert_resource(GlobalMaterials {
        opaque: materials.add(voxel_mat_opaque),
        transparent: materials.add(voxel_mat_transparent),
    });

    // Default background color for main menu (dark background).
    commands.insert_resource(ClearColor(Color::srgb(0.2, 0.2, 0.2)));

    // Setup global 2D camera for UI with a marker component.
    commands.spawn((UICamera, Camera2d::default(), Camera { order: 1, ..default() }));

    // Set the window resolution.
    window.resolution = WindowResolution::new(1920.0, 1080.0);
}
