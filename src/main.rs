use std::{cmp, collections::HashMap};

use bevy::{
    asset::RenderAssetUsages,
    input::mouse::MouseMotion,
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
    tasks::{AsyncComputeTaskPool, Task},
    window::{CursorGrabMode, PrimaryWindow},
};

// Chunk size data.
const CHUNK_SIZE: u16 = 16;
const CHUNK_HEIGHT: u16 = 256;
const TOTAL: usize = (CHUNK_SIZE as usize).pow(2) * CHUNK_HEIGHT as usize;

// How many chunks should be loaded in each direction.
const RENDER_DISTANCE: i32 = 16;

// Block types are hard-coded but should be loaded from a file later.
#[repr(u16)]
#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
enum BlockType
{
    Air,
    Grass,
    Dirt,
    Stone,
}

// A block only has a color, but later it will have a texture instead, and
// possibly other fields (light emission, full/half...).
#[derive(Clone)]
struct Block
{
    color: Color,
}

// A simple way to associate blocks to chunks without copying each fields.
#[derive(Resource, Default, Clone)]
struct BlockList
{
    data: HashMap<BlockType, Block>,
}

// A struct representing the horizontal position of a chunk. It can serve as an
// ID for a chunk.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct ChunkPos
{
    x: i32,
    y: i32,
}

// The Chunk component.
#[derive(Component, Clone)]
struct Chunk
{
    pos: ChunkPos,
    blocks: Box<[BlockType; TOTAL]>,
}

// Each chunk is always loaded using the seed, instead of being saved.
// But we then need to store the modifications that were applied to each chunk,
// else they will be lost when the player is too far.
#[derive(Clone)]
struct Modification
{
    index: usize,
    new: BlockType,
}

// The Map resource. It stores the seed for this world, and a list of
// modifications that were applied to each chunk.
#[derive(Resource, Default)]
struct Map
{
    seed: u64,
    modified: HashMap<ChunkPos, Vec<Modification>>,
}

// The Chunkmap resource stores the chunks that are currently loaded.
// It allows the program to load chunks that are in range and unload those that
// are far away.
#[derive(Resource, Default)]
struct ChunkMap(HashMap<ChunkPos, Entity>);

// Tracks the loading state of chunks.
#[derive(Resource, Default)]
struct ChunkLoadState
{
    tasks: HashMap<ChunkPos, Task<(ChunkPos, Chunk)>>,
}

// Tracks the meshing state of chunks.
#[derive(Resource, Default)]
struct ChunkMeshState
{
    tasks: HashMap<ChunkPos, Task<(ChunkPos, Mesh)>>,
}

// This system is called when the game launches. The data is hard-coded but
// should be read from a file eventually.
fn load_block_types(mut list: ResMut<BlockList>)
{
    list.data
        .insert(BlockType::Air, Block { color: Color::NONE });

    list.data
        .insert(BlockType::Grass, Block { color: Color::srgba_u8(116, 184, 22, 255) });

    list.data
        .insert(BlockType::Dirt, Block { color: Color::srgba_u8(147, 85, 41, 255) });

    list.data
        .insert(BlockType::Stone, Block { color: Color::srgba_u8(134, 142, 150, 255) });
}

// This function creates a chunk based on its position and the world seed.
// For now, the generation is simple and creates a flat world, but procedural
// generation could be implemented in this function.
fn load_raw_chunk(_seed: u64, pos: ChunkPos) -> Chunk
{
    // The chunk is only air at first.
    let mut blocks = [BlockType::Air; TOTAL];

    // We iterate over the 3 dimensions.
    for z in 0 .. CHUNK_SIZE as usize
    {
        for x in 0 .. CHUNK_SIZE as usize
        {
            for y in 0 .. CHUNK_HEIGHT as usize
            {
                // 'idx' is the index of the current block in the chunk array.
                let idx = y * (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)
                    + z * (CHUNK_SIZE as usize)
                    + x;

                // The terrain will be generated with pyramidal hills to test mesh generation
                // and lighting.

                let distance_left = x;
                let distance_right = CHUNK_SIZE as usize - 1 - x;
                let distance_top = z;
                let distance_bottom = CHUNK_SIZE as usize - 1 - z;

                let terrain_height = cmp::min(
                    cmp::min(distance_left, distance_right),
                    cmp::min(distance_top, distance_bottom),
                ) + 60_usize;

                blocks[idx] = if y <= terrain_height
                {
                    BlockType::Stone
                }
                else if y <= terrain_height + 2
                {
                    BlockType::Dirt
                }
                else if y <= terrain_height + 3
                {
                    BlockType::Grass
                }
                else
                {
                    BlockType::Air
                };

                // The block type is determined by the height only (flat world).
                // blocks[idx] = match y
                // {
                //     0 ..= 60 => BlockType::Stone,
                //     61 ..= 62 => BlockType::Dirt,
                //     63 => BlockType::Grass,
                //     _ => BlockType::Air,
                // };
            }
        }
    }

    // The chunk is returned.
    return Chunk { pos, blocks: blocks.into() };
}

// This function applies any saved modifications to a chunk after it is loaded.
fn apply_modifications(chunk: &mut Chunk, modifications: &[Modification])
{
    for modification in modifications
    {
        chunk.blocks[modification.index] = modification.new;
    }
}

// This system manages the loading and unloading of chunks based on their
// position.
fn manage_chunk_loading(
    mut commands: Commands,
    map: Res<Map>,
    mut chunk_map: ResMut<ChunkMap>,
    mut chunk_state: ResMut<ChunkLoadState>,
    camera: Query<&Transform, With<FlyCam>>,
)
{
    // Get player chunk position.
    let player_chunk = match camera.single()
    {
        Ok(cam) =>
        {
            let cam_pos = cam.translation;
            let chunk_x = (cam_pos.x / CHUNK_SIZE as f32).floor() as i32;
            let chunk_y = (cam_pos.z / CHUNK_SIZE as f32).floor() as i32;
            ChunkPos { x: chunk_x, y: chunk_y }
        },
        // If there was a problem retrieving the camera data, we use a default position.
        Err(_) => ChunkPos { x: 0, y: 0 },
    };

    // The player's chunk's position will be used to determine the chunks that
    // should be loaded right now. This is where the render distance is useful.
    let mut desired = Vec::new();
    for dz in -RENDER_DISTANCE ..= RENDER_DISTANCE
    {
        for dx in -RENDER_DISTANCE ..= RENDER_DISTANCE
        {
            desired.push(ChunkPos { x: player_chunk.x + dx, y: player_chunk.y + dz });
        }
    }

    // Chunks that are currenlty loaded but are not wanted will be unloaded.
    for old_pos in chunk_map.0.keys().cloned().collect::<Vec<_>>()
    {
        if !desired.contains(&old_pos)
        {
            // Despawn the entity for the chunk and remove it from the map.
            if let Some(e) = chunk_map.0.remove(&old_pos)
            {
                commands.entity(e).despawn();
            }
            // Remove any loading tasks for this chunk.
            chunk_state.tasks.remove(&old_pos);
        }
    }

    // Chunks that are currently unloaded but are wanted will be loaded.
    for pos in desired
    {
        // Skip if chunk is already loaded or being loaded.
        if chunk_map.0.contains_key(&pos) || chunk_state.tasks.contains_key(&pos)
        {
            // Already loaded or being loaded.
            continue;
        }
        // Spawn async task to generate the chunk.
        let seed = map.seed;
        let modifications = map.modified.get(&pos).cloned().unwrap_or_default();
        let task_pool = AsyncComputeTaskPool::get();
        let task = task_pool.spawn(async move {
            // Generate the chunk and apply any modifications.
            let mut chunk = load_raw_chunk(seed, pos);
            apply_modifications(&mut chunk, &modifications);
            (pos, chunk)
        });
        // Insert the loading task into the state.
        chunk_state.tasks.insert(pos, task);
    }
}

// The faces of each block, used for mesh generation.
const FACES: &[(IVec3, [[f32; 3]; 4])] = &[
    // +X
    (IVec3::new(1, 0, 0), [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]]),
    // -X
    (IVec3::new(-1, 0, 0), [[0., 0., 1.], [0., 1., 1.], [0., 1., 0.], [0., 0., 0.]]),
    // +Y
    (IVec3::new(0, 1, 0), [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]]),
    // -Y
    (IVec3::new(0, -1, 0), [[0., 0., 1.], [0., 0., 0.], [1., 0., 0.], [1., 0., 1.]]),
    // +Z
    (IVec3::new(0, 0, 1), [[1., 0., 1.], [1., 1., 1.], [0., 1., 1.], [0., 0., 1.]]),
    // -Z
    (IVec3::new(0, 0, -1), [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]]),
];

// It would be too costly to create one mesh per block.
// With a render distance of 16 chunks, more that 70 million blocks could be
// loaded. Instead, we will create one mesh per chunk, and display only the
// exposed faces of this mesh.
fn mesh_chunk(chunk: &Chunk, block_list: &BlockList) -> Mesh
{
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    let mut idx_counter = 0u32;

    let cs = CHUNK_SIZE as usize;
    let ch = CHUNK_HEIGHT as usize;

    // This lambda takes the 3 coordinates of a block in a chunk and returns the
    // block type.
    let get = |x: i32, y: i32, z: i32| {
        if !(0 .. cs as i32).contains(&x)
            || !(0 .. ch as i32).contains(&y)
            || !(0 .. cs as i32).contains(&z)
        {
            return BlockType::Air;
        }
        let xi = x as usize;
        let yi = y as usize;
        let zi = z as usize;
        let idx = yi * cs * cs + zi * cs + xi;
        chunk.blocks[idx]
    };

    // Iterate over the 3 dimensions.
    for z in 0 .. cs
    {
        for y in 0 .. ch
        {
            for x in 0 .. cs
            {
                // Get the block type.
                let b = chunk.blocks[y * cs * cs + z * cs + x];

                if b == BlockType::Air
                {
                    // Air blocks will not be part of the mesh.
                    continue;
                }

                // Get the color of the block.
                let col = block_list.data[&b].color.to_linear().to_f32_array();

                // Iterate over the faces of the block.
                for &(dir, verts) in FACES
                {
                    let nx = x as i32 + dir.x;
                    let ny = y as i32 + dir.y;
                    let nz = z as i32 + dir.z;
                    if get(nx, ny, nz) != BlockType::Air
                    {
                        // This face is hidden by another block.
                        continue;
                    }

                    // Create a quad for this face.
                    for vert in verts
                    {
                        positions.push([
                            x as f32 + vert[0],
                            y as f32 + vert[1],
                            z as f32 + vert[2],
                        ]);
                        normals.push([dir.x as f32, dir.y as f32, dir.z as f32]);
                        colors.push(col);
                    }
                    // Create two triangles for this quad.
                    indices.extend_from_slice(&[
                        idx_counter,
                        idx_counter + 1,
                        idx_counter + 2,
                        idx_counter,
                        idx_counter + 2,
                        idx_counter + 3,
                    ]);
                    idx_counter += 4;
                }
            }
        }
    }

    // Create a new mesh.
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());

    // Populate the mesh with the data we calculated.
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));

    // Return the mesh.
    return mesh;
}

/// A handle to the neutral white material we want to reuse every time.
#[derive(Resource)]
struct BaseMaterial(pub Handle<StandardMaterial>);

/// Create the material once at startup.
fn setup_base_material(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>)
{
    let handle = materials.add(StandardMaterial { base_color: Color::WHITE, ..default() });
    commands.insert_resource(BaseMaterial(handle));
}

// This system processes completed chunk loading tasks.
fn process_chunk_tasks(
    mut commands: Commands,
    mut chunk_map: ResMut<ChunkMap>,
    mut chunk_state: ResMut<ChunkLoadState>,
    mut mesh_state: ResMut<ChunkMeshState>,
    block_list: Res<BlockList>,
)
{
    use std::task::{Context, Poll};

    use futures_util::task::noop_waker_ref;
    let mut completed = Vec::new();
    for (task_pos, task) in chunk_state.tasks.iter_mut()
    {
        let waker = noop_waker_ref();
        let mut cx = Context::from_waker(waker);
        if let Poll::Ready((chunk_pos, chunk)) = std::pin::Pin::new(task).poll(&mut cx)
        {
            let chunk_clone = chunk.clone();
            let e = commands
                .spawn((
                    Transform::from_translation(Vec3::new(
                        chunk_pos.x as f32 * CHUNK_SIZE as f32,
                        0.0,
                        chunk_pos.y as f32 * CHUNK_SIZE as f32,
                    )),
                    Visibility::default(),
                    chunk,
                ))
                .id();
            chunk_map.0.insert(chunk_pos, e);
            let chunk_pos_copy = chunk_pos;
            let block_list = block_list.clone();
            let task_pool = AsyncComputeTaskPool::get();
            let mesh_task = task_pool.spawn(async move {
                let mesh = mesh_chunk(&chunk_clone, &block_list);
                (chunk_pos_copy, mesh)
            });
            mesh_state.tasks.insert(chunk_pos, mesh_task);
            completed.push(*task_pos);
        }
    }
    for pos in completed
    {
        chunk_state.tasks.remove(&pos);
    }
}

// This system processes completed chunk meshing tasks.
fn process_chunk_mesh_tasks(
    mut commands: Commands,
    mut mesh_state: ResMut<ChunkMeshState>,
    mut meshes: ResMut<Assets<Mesh>>,
    base_mat: Res<BaseMaterial>,
    chunk_map: Res<ChunkMap>,
)
{
    use std::task::{Context, Poll};

    use futures_util::task::noop_waker_ref;
    let mut completed = Vec::new();
    for (_, task) in mesh_state.tasks.iter_mut()
    {
        let waker = noop_waker_ref();
        let mut cx = Context::from_waker(waker);
        if let Poll::Ready((chunk_pos, mesh)) = std::pin::Pin::new(task).poll(&mut cx)
        {
            if let Some(&entity) = chunk_map.0.get(&chunk_pos)
            {
                let mesh_handle = meshes.add(mesh);
                commands
                    .entity(entity)
                    .insert((Mesh3d(mesh_handle), MeshMaterial3d(base_mat.0.clone())));
            }
            completed.push(chunk_pos);
        }
    }
    for pos in completed
    {
        mesh_state.tasks.remove(&pos);
    }
}

fn main()
{
    let mut app = App::new();

    app.add_plugins(DefaultPlugins);

    app.init_resource::<BlockList>();
    app.init_resource::<Map>();
    app.init_resource::<ChunkMap>();
    app.init_resource::<ChunkLoadState>();
    app.init_resource::<ChunkMeshState>();

    app.add_systems(Startup, load_block_types);
    app.add_systems(
        Update,
        (manage_chunk_loading, process_chunk_tasks, process_chunk_mesh_tasks).chain(),
    );
    app.add_systems(Update, fly_camera_movement);
    app.add_systems(Update, mouse_look);
    app.add_systems(Update, (block_interaction, remesh_changed_chunks).chain());
    app.add_systems(Startup, setup_base_material);

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
    ));

    // 2D camera for the UI.
    commands.spawn((Camera2d::default(), Camera { order: 1, ..default() }));

    // Crosshair.
    commands.spawn((
        Sprite::from_image(assets.load("textures/crosshair.png")),
        Transform::from_translation(Transform::IDENTITY.translation + Vec3::new(0.0, 0.0, 1.0)),
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

    // Lock cursor position.
    for mut window in windows.iter_mut()
    {
        window.cursor_options.visible = false;
        window.cursor_options.grab_mode = CursorGrabMode::Locked;
    }
}

// The FlyCam component represents the player camera.
#[derive(Component)]
struct FlyCam
{
    speed: f32,
    sprint_mult: f32,
    sensitivity: f32,
    yaw: f32,
    pitch: f32,
}

// This system manages keyboard input and moves the camera accordingly.
fn fly_camera_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<(&FlyCam, &mut Transform)>,
)
{
    let dt = time.delta().as_secs_f32();
    for (flycam, mut transform) in query.iter_mut()
    {
        // Create a direction vector for horizontal movement.
        let mut horiz = Vec3::ZERO;

        // Create a direction value for vertical movement.
        let mut vert = 0.0;

        // Forward / back.
        if keyboard.pressed(KeyCode::KeyW)
        {
            horiz.z -= 1.0;
        }
        if keyboard.pressed(KeyCode::KeyS)
        {
            horiz.z += 1.0;
        }

        // Left / right.
        if keyboard.pressed(KeyCode::KeyA)
        {
            horiz.x -= 1.0;
        }
        if keyboard.pressed(KeyCode::KeyD)
        {
            horiz.x += 1.0;
        }

        // Up / down.
        if keyboard.pressed(KeyCode::Space)
        {
            vert += 1.0;
        }
        if keyboard.pressed(KeyCode::ShiftLeft)
        {
            vert -= 1.0;
        }

        // If 'alt' is pressed, the camera moves faster horizontally.
        let speed_mult = if keyboard.pressed(KeyCode::AltLeft) { flycam.sprint_mult } else { 1.0 };

        // Total movement vector.
        let mut delta = Vec3::ZERO;

        // If there is a horizontal movement, 'delta' must be updated.
        if horiz != Vec3::ZERO
        {
            let dir = horiz.normalize();
            let yaw_quat = Quat::from_axis_angle(Vec3::Y, flycam.yaw);
            let world_horiz = yaw_quat * dir;
            delta += world_horiz * flycam.speed * dt * speed_mult;
        }

        // If there is a vertical movement, 'delta' must be updated.
        if vert != 0.0
        {
            delta.y += vert * flycam.speed * dt;
        }

        // 'delta' is applied to the camera translation.
        transform.translation += delta;
    }
}

// This system manages mouse input and pans the camera.
fn mouse_look(mut motion: EventReader<MouseMotion>, mut query: Query<(&mut FlyCam, &mut Transform)>)
{
    for ev in motion.read()
    {
        for (mut flycam, mut transform) in query.iter_mut()
        {
            // Get the yaw (Y‐axis) from X movement, and the pitch (X‐axis) from Y movement.
            flycam.yaw -= ev.delta.x * flycam.sensitivity;
            flycam.pitch -= ev.delta.y * flycam.sensitivity;
            // Clamp the pitch to [-90°, +90°].
            flycam.pitch = flycam
                .pitch
                .clamp(-90.0_f32.to_radians(), 90.0_f32.to_radians());

            // Update the actual transform.rotation.
            transform.rotation = Quat::from_euler(EulerRot::YXZ, flycam.yaw, flycam.pitch, 0.0);
        }
    }
}

// This system handles block interaction with the mouse.
// If a block is placed or destroyed, the chunk is marked as changed and will be
// remeshed.
fn block_interaction(
    buttons: Res<ButtonInput<MouseButton>>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    chunk_map: Res<ChunkMap>,
    mut map: ResMut<Map>,
    mut chunks: Query<&mut Chunk>,
)
{
    let request_destroy: bool = buttons.just_pressed(MouseButton::Left);
    let request_place: bool = buttons.just_pressed(MouseButton::Right);

    // If neither left nor right mouse button is pressed, we do nothing.
    if !(request_destroy || request_place)
    {
        return;
    }

    let Ok(cam_tf) = cams.single()
    else
    {
        println!("Camera transform is not valid.");
        return;
    };

    // Get the camera's origin and direction.
    let origin = cam_tf.translation();
    let dir = cam_tf.forward();

    // March a ray out to max_d in steps.
    let max_d = 8.0;
    let step = 0.01; // The step is small to prevent missing blocks when clicking on a corner.
    let mut t = 0.0;
    let mut last_air_pos: Option<(i32, i32, i32)> = None;
    let mut last_air_chunk = None;

    // Iterate over the ray from the camera origin in the direction of the camera.
    while t < max_d
    {
        // Calculate the current world position of the ray's end.
        let p = origin + dir * t;
        let bx = p.x.floor() as i32;
        let by = p.y.floor() as i32;
        let bz = p.z.floor() as i32;

        // Get the chunk in which the ray's end is.
        let cx = bx.div_euclid(CHUNK_SIZE as i32);
        let cz = bz.div_euclid(CHUNK_SIZE as i32);
        let cpos = ChunkPos { x: cx, y: cz };

        // Check if the chunk is loaded.
        if let Some(&entity) = chunk_map.0.get(&cpos)
        {
            if let Ok(mut chunk) = chunks.get_mut(entity)
            {
                // Get the local coordinates of the ray's end (in the chunk).
                let lx = bx - cx * CHUNK_SIZE as i32;
                let lz = bz - cz * CHUNK_SIZE as i32;
                if (0 .. CHUNK_SIZE as i32).contains(&lx)
                    && (0 .. CHUNK_HEIGHT as i32).contains(&by)
                    && (0 .. CHUNK_SIZE as i32).contains(&lz)
                {
                    // Calculate the index of the block in the chunk's blocks array.
                    let idx = (by as usize) * (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)
                        + (lz as usize) * (CHUNK_SIZE as usize)
                        + (lx as usize);

                    // Check if the block at this index is air.
                    let current_block = chunk.blocks[idx];
                    if current_block != BlockType::Air
                    {
                        if request_destroy
                        {
                            // Destroy the block.
                            chunk.blocks[idx] = BlockType::Air;
                            map.modified
                                .entry(cpos)
                                .or_default()
                                .push(Modification { index: idx, new: BlockType::Air });
                            return;
                        }
                        else if request_place && last_air_pos.is_some()
                        {
                            // Place a block at the last air position we found.
                            if let (Some((last_x, last_y, last_z)), Some(last_chunk_entity)) =
                                (last_air_pos, last_air_chunk)
                            {
                                // Get the chunk where we want to place the block.
                                if let Ok(mut target_chunk) = chunks.get_mut(last_chunk_entity)
                                {
                                    // Calculate the local coordinates in the target chunk.
                                    let last_cx = last_x.div_euclid(CHUNK_SIZE as i32);
                                    let last_cz = last_z.div_euclid(CHUNK_SIZE as i32);
                                    let last_lx = last_x - last_cx * CHUNK_SIZE as i32;
                                    let last_lz = last_z - last_cz * CHUNK_SIZE as i32;

                                    // Calculate the index in the target chunk's blocks array.
                                    let last_idx = (last_y as usize)
                                        * (CHUNK_SIZE as usize)
                                        * (CHUNK_SIZE as usize)
                                        + (last_lz as usize) * (CHUNK_SIZE as usize)
                                        + (last_lx as usize);

                                    // Place the block at the last air position.
                                    target_chunk.blocks[last_idx] = BlockType::Grass;

                                    map.modified
                                        .entry(cpos)
                                        .or_default()
                                        .push(Modification { index: idx, new: BlockType::Grass });
                                }
                            }
                            return;
                        }
                        return;
                    }
                    else
                    {
                        // Keep track of the last air position for block placement.
                        last_air_pos = Some((bx, by, bz));
                        last_air_chunk = Some(entity);
                    }
                }
            }
        }
        t += step;
    }
}

// This system remeshes chunks that have changed since the last frame.
fn remesh_changed_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    block_list: Res<BlockList>,
    base_mat: Res<BaseMaterial>,
    query: Query<(Entity, &Chunk, Option<&Children>), Changed<Chunk>>,
    new_chunks: Query<Entity, Added<Chunk>>,
)
{
    // We only want to remesh chunks that were changed in this frame.
    let just_added: std::collections::HashSet<_> = new_chunks.iter().collect();

    for (entity, chunk, children_opt) in &query
    {
        if just_added.contains(&entity)
        {
            // This chunk was just added, so we don't need to remesh it.
            continue;
        }

        // If the chunk has children, we need to despawn them.
        if let Some(children) = children_opt
        {
            for &child in children
            {
                commands.entity(child).despawn();
            }
        }

        // Build the new mesh for the chunk.
        let new_mesh_handle = meshes.add(mesh_chunk(chunk, &*block_list));

        // Insert the new mesh and keep the base material.
        commands.entity(entity).insert((
            Mesh3d(new_mesh_handle),
            // Keep the base material for the mesh.
            MeshMaterial3d(base_mat.0.clone()),
        ));
    }
}
