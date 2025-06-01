use std::{
    cmp,
    collections::HashMap,
    ops::{Add, Sub},
};

use bevy::{
    asset::RenderAssetUsages,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    input::mouse::MouseMotion,
    math::ops::sqrt,
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
const RENDER_DISTANCE: i32 = 32;

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

// A block has a set of 6 textures, one per face. Later, it could have more
// data, for example light emission, or a shape...
#[derive(Clone)]
pub struct Block
{
    faces: [Handle<Image>; 6],
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
pub struct ChunkPos
{
    x: i32,
    y: i32,
}

impl Add for ChunkPos
{
    type Output = ChunkPos;

    fn add(self, other: ChunkPos) -> ChunkPos
    {
        ChunkPos { x: self.x + other.x, y: self.y + other.y }
    }
}

impl Sub for ChunkPos
{
    type Output = ChunkPos;

    fn sub(self, other: ChunkPos) -> ChunkPos
    {
        ChunkPos { x: self.x - other.x, y: self.y - other.y }
    }
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
pub struct ChunkMap
{
    pub loaded_chunks: HashMap<ChunkPos, Entity>,
}

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
    tasks: HashMap<ChunkPos, Task<(ChunkPos, HashMap<Handle<Image>, Mesh>)>>,
}

// This system is called when the game launches. The data is hard-coded but
// should be read from a file eventually.
fn load_block_types(mut list: ResMut<BlockList>, textures: Res<TextureHandles>)
{
    list.data.insert(
        BlockType::Air,
        Block {
            faces: [
                Handle::default(), // +X
                Handle::default(), // -X
                Handle::default(), // +Y (top)
                Handle::default(), // -Y (bottom)
                Handle::default(), // +Z
                Handle::default(), // -Z
            ],
        },
    );

    list.data.insert(
        BlockType::Grass,
        Block {
            faces: [
                textures.grass_side.clone(), // +X
                textures.grass_side.clone(), // -X
                textures.grass_top.clone(),  // +Y (top)
                textures.dirt.clone(),       // -Y (bottom)
                textures.grass_side.clone(), // +Z
                textures.grass_side.clone(), // -Z
            ],
        },
    );

    list.data.insert(
        BlockType::Dirt,
        Block {
            faces: [
                textures.dirt.clone(), // +X
                textures.dirt.clone(), // -X
                textures.dirt.clone(), // +Y (top)
                textures.dirt.clone(), // -Y (bottom)
                textures.dirt.clone(), // +Z
                textures.dirt.clone(), // -Z
            ],
        },
    );

    list.data.insert(
        BlockType::Stone,
        Block {
            faces: [
                textures.stone.clone(), // +X
                textures.stone.clone(), // -X
                textures.stone.clone(), // +Y (top)
                textures.stone.clone(), // -Y (bottom)
                textures.stone.clone(), // +Z
                textures.stone.clone(), // -Z
            ],
        },
    );
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

fn count_chunks(query: Query<(), With<Chunk>>)
{
    println!("Loaded chunks: {}", query.iter().count());
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
    // Get player chunk position and camera info.
    let (player_chunk, cam_pos, cam_forward) = match camera.single()
    {
        Ok(cam) =>
        {
            let cam_pos = cam.translation;
            let chunk_x = (cam_pos.x / CHUNK_SIZE as f32).floor() as i32;
            let chunk_y = (cam_pos.z / CHUNK_SIZE as f32).floor() as i32;
            (ChunkPos { x: chunk_x, y: chunk_y }, cam_pos, cam.forward())
        },
        Err(_) => (ChunkPos { x: 0, y: 0 }, Vec3::ZERO, Dir3::Z),
    };

    // Camera FOV and culling parameters.
    let fov_cos = (220.0f32.to_radians() / 2.0).cos();
    let max_dist = (RENDER_DISTANCE as f32 + 0.5) * CHUNK_SIZE as f32;

    let cam_forward_xz = {
        let mut v = cam_forward.as_vec3();
        v.y = 0.0;
        if v.length_squared() > 0.0
        {
            v = v.normalize();
        }
        v
    };

    // A helper to determine if a chunk is visible from the camera pov.
    let is_chunk_visible = |chunk_pos: ChunkPos| {
        // Always show the chunk under the player and its neighbors in a radius of 2.
        if (chunk_pos.x - player_chunk.x).abs() <= 2 && (chunk_pos.y - player_chunk.y).abs() <= 2
        {
            return true;
        }
        let center = Vec3::new(
            (chunk_pos.x as f32 + 0.5) * CHUNK_SIZE as f32,
            CHUNK_HEIGHT as f32 / 2.0,
            (chunk_pos.y as f32 + 0.5) * CHUNK_SIZE as f32,
        );
        let to_center = center - cam_pos;
        let dist = to_center.length();
        if dist > max_dist
        {
            return false;
        }
        let mut dir = to_center;
        dir.y = 0.0;
        if dir.length_squared() == 0.0
        {
            return true;
        }
        let dir = dir.normalize();
        cam_forward_xz.dot(dir) > fov_cos
    };

    // The player's chunk's position will be used to determine the chunks that
    // should be loaded right now. This is where the render distance is useful.
    let mut desired = Vec::new();
    for dz in -RENDER_DISTANCE ..= RENDER_DISTANCE
    {
        for dx in -RENDER_DISTANCE ..= RENDER_DISTANCE
        {
            if sqrt((dx * dx + dz * dz) as f32) > RENDER_DISTANCE as f32
            {
                // Skip chunks that are too far, to create a circular rendered terrain.
                continue;
            }
            let pos = ChunkPos { x: player_chunk.x + dx, y: player_chunk.y + dz };
            if is_chunk_visible(pos)
            {
                desired.push(pos);
            }
        }
    }

    // Chunks that are currenlty loaded but are not wanted will be unloaded.
    for old_pos in chunk_map.loaded_chunks.keys().cloned().collect::<Vec<_>>()
    {
        if !desired.contains(&old_pos)
        {
            // Despawn the entity for the chunk and remove it from the map.
            if let Some(e) = chunk_map.loaded_chunks.remove(&old_pos)
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
        if chunk_map.loaded_chunks.contains_key(&pos) || chunk_state.tasks.contains_key(&pos)
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
const FACES: &[(IVec3, [[f32; 3]; 4], [[f32; 2]; 4])] = &[
    // +X
    (
        IVec3::new(1, 0, 0),
        [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]],
        [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
    ),
    // -X
    (
        IVec3::new(-1, 0, 0),
        [[0., 0., 1.], [0., 1., 1.], [0., 1., 0.], [0., 0., 0.]],
        [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
    ),
    // +Y (top)
    (
        IVec3::new(0, 1, 0),
        [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]],
        [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
    ),
    // -Y (bottom)
    (
        IVec3::new(0, -1, 0),
        [[0., 0., 1.], [0., 0., 0.], [1., 0., 0.], [1., 0., 1.]],
        [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
    ),
    // +Z
    (
        IVec3::new(0, 0, 1),
        [[1., 0., 1.], [1., 1., 1.], [0., 1., 1.], [0., 0., 1.]],
        [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
    ),
    // -Z
    (
        IVec3::new(0, 0, -1),
        [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]],
        [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
    ),
];

// It would be too costly to create one mesh per block.
// With a render distance of 16 chunks, more that 70 million blocks could be
// loaded. Instead, we will create one mesh per chunk, and display only the
// exposed faces of this mesh.
fn mesh_chunk(
    chunk: &Chunk,
    block_list: &BlockList,
    neighbor_chunks: &HashMap<ChunkPos, Chunk>,
) -> HashMap<Handle<Image>, Mesh>
{
    // For each texture, we store positions, normals, UVs and indices.
    let mut per_tex: HashMap<
        Handle<Image>,
        (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>),
    > = HashMap::new();

    let cs = CHUNK_SIZE as usize;
    let ch = CHUNK_HEIGHT as usize;
    let cs_i32 = CHUNK_SIZE as i32;
    let ch_i32 = CHUNK_HEIGHT as i32;

    // Helper closure to check if a block is solid at the given coordinates.
    let is_solid = |mut req_x: i32, req_y: i32, mut req_z: i32| -> bool {
        if !(0 .. ch_i32).contains(&req_y)
        {
            // If the y coordinate is below 0 or above the chunk height, it's not part of
            // the world.
            return false;
        }

        // Get the chunk in which the block is located.
        let mut target_chunk_pos = chunk.pos;

        // Determine if we need to look in a neighbor chunk based on x coordinate.
        if req_x < 0
        {
            target_chunk_pos.x -= 1;
            req_x += cs_i32;
        }
        else if req_x >= cs_i32
        {
            target_chunk_pos.x += 1;
            req_x -= cs_i32;
        }

        // Determine if we need to look in a neighbor chunk based on z coordinate.
        // Note: ChunkPos.y stores the world's Z coordinate for the chunk.
        if req_z < 0
        {
            target_chunk_pos.y -= 1;
            req_z += cs_i32;
        }
        else if req_z >= cs_i32
        {
            target_chunk_pos.y += 1;
            req_z -= cs_i32;
        }

        let chunk_data_to_use = if target_chunk_pos == chunk.pos
        {
            // Use the main chunk passed to mesh_chunk.
            Some(chunk)
        }
        else
        {
            // Get the chunk data from the neighbor_chunks map.
            neighbor_chunks.get(&target_chunk_pos) // Look up in the passed neighbor_chunks map
        };

        if let Some(selected_chunk) = chunk_data_to_use
        {
            // req_x, req_y, req_z are now local to selected_chunk.
            let idx = (req_y as usize) * cs * cs + (req_z as usize) * cs + (req_x as usize);
            return selected_chunk.blocks[idx] != BlockType::Air;
        }

        // If the neighbor chunk is not loaded yet, we assume the block is solid to hide
        // the face. This is a behaviour that will need to be fixed later.
        return true;
    };

    // Iterate over all blocks in the chunk.
    for z_local in 0 .. cs
    {
        for y_local in 0 .. ch
        {
            for x_local in 0 .. cs
            {
                // Get the block type at this position.
                let b = chunk.blocks[y_local * cs * cs + z_local * cs + x_local];
                if b == BlockType::Air
                {
                    // Skip air blocks.
                    continue;
                }
                // Get the texture handles for each face of this block.
                let face_tex = &block_list.data[&b].faces;

                // Iterate over each face (+X, -X, +Y, -Y, +Z, -Z).
                for (face_idx, &(dir, verts, base_uvs)) in FACES.iter().enumerate()
                {
                    // Determine the coordinates of the block to check in the neighboring space.
                    let neighbor_check_x = x_local as i32 + dir.x;
                    let neighbor_check_y = y_local as i32 + dir.y;
                    let neighbor_check_z = z_local as i32 + dir.z;

                    // Only add the face if the neighbor in that direction is air.
                    if is_solid(neighbor_check_x, neighbor_check_y, neighbor_check_z)
                    {
                        continue;
                    }

                    // Get the texture for this face.
                    let tex = &face_tex[face_idx];

                    // Get or create the mesh data for this texture.
                    let entry = per_tex.entry(tex.clone()).or_default();
                    let base_index = entry.0.len() as u32;

                    // Add the 4 vertices for this face.
                    for i in 0 .. 4
                    {
                        entry.0.push([
                            x_local as f32 + verts[i][0],
                            y_local as f32 + verts[i][1],
                            z_local as f32 + verts[i][2],
                        ]);
                        entry.1.push([dir.x as f32, dir.y as f32, dir.z as f32]);
                        entry.2.push(base_uvs[i]);
                    }

                    // Add the two triangles (6 indices) for this face.
                    entry.3.extend_from_slice(&[
                        base_index,
                        base_index + 1,
                        base_index + 2,
                        base_index,
                        base_index + 2,
                        base_index + 3,
                    ]);
                }
            }
        }
    }

    // Convert the per-texture mesh data into actual Mesh objects.
    per_tex
        .into_iter()
        .map(|(tex, (pos, norm, uv, idx))| {
            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, norm);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
            mesh.insert_indices(Indices::U32(idx));
            (tex, mesh)
        })
        .collect()
}

// This system processes completed chunk loading tasks.

// Helper function to get data for neighboring chunks.
fn get_neighbor_chunk_data(
    current_chunk_pos: ChunkPos,
    chunk_map: &ChunkMap,
    all_chunks_query: &Query<&Chunk>,
) -> HashMap<ChunkPos, Chunk>
{
    let mut neighbor_chunks_data = HashMap::new();
    let neighbor_offsets = [
        ChunkPos { x: -1, y: 0 },
        ChunkPos { x: 1, y: 0 },
        ChunkPos { x: 0, y: -1 },
        ChunkPos { x: 0, y: 1 },
    ];
    for offset in &neighbor_offsets
    {
        let neighbor_pos = current_chunk_pos + *offset;
        if let Some(entity) = chunk_map.loaded_chunks.get(&neighbor_pos)
        {
            if let Ok(chunk_component) = all_chunks_query.get(*entity)
            {
                neighbor_chunks_data.insert(neighbor_pos, chunk_component.clone());
            }
        }
    }
    neighbor_chunks_data
}

fn process_chunk_tasks(
    mut commands: Commands,
    mut chunk_map: ResMut<ChunkMap>,
    mut chunk_state: ResMut<ChunkLoadState>,
    mut mesh_state: ResMut<ChunkMeshState>,
    block_list: Res<BlockList>,
    all_chunks_query: Query<&Chunk>,
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
            chunk_map.loaded_chunks.insert(chunk_pos, e);
            let chunk_pos_copy = chunk_pos;
            let block_list = block_list.clone();
            let task_pool = AsyncComputeTaskPool::get();

            // Prepare neighbor data for mesh_chunk.
            let neighbor_chunks_data =
                get_neighbor_chunk_data(chunk_pos, &chunk_map, &all_chunks_query);

            let mesh_task = task_pool.spawn(async move {
                let meshes_by_tex = mesh_chunk(&chunk_clone, &block_list, &neighbor_chunks_data);
                (chunk_pos_copy, meshes_by_tex)
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
    mut materials: ResMut<Assets<StandardMaterial>>,
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
        if let Poll::Ready((chunk_pos, meshes_by_tex)) = std::pin::Pin::new(task).poll(&mut cx)
        {
            if let Some(&chunk_entity) = chunk_map.loaded_chunks.get(&chunk_pos)
            {
                for (tex_handle, mesh) in meshes_by_tex
                {
                    let mat_handle = materials.add(StandardMaterial {
                        base_color_texture: Some(tex_handle.clone()),
                        alpha_mode: AlphaMode::Mask(0.5),
                        ..default()
                    });
                    let mesh_handle = meshes.add(mesh);

                    commands.entity(chunk_entity).with_children(|c| {
                        c.spawn((
                            Mesh3d(mesh_handle),
                            MeshMaterial3d(mat_handle),
                            Visibility::default(),
                        ));
                    });
                }
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

// The TextureHandles resource stores handles to all block textures.
#[derive(Resource)]
struct TextureHandles
{
    grass_side: Handle<Image>,
    grass_top: Handle<Image>,
    dirt: Handle<Image>,
    stone: Handle<Image>,
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
                .clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians());

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
        if let Some(&entity) = chunk_map.loaded_chunks.get(&cpos)
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
    mut materials: ResMut<Assets<StandardMaterial>>,
    block_list: Res<BlockList>,
    query: Query<(Entity, &Chunk, Option<&Children>), Changed<Chunk>>,
    new_chunks: Query<Entity, Added<Chunk>>,
    chunk_map: Res<ChunkMap>,
    all_chunks_query: Query<&Chunk>,
)
{
    use std::collections::HashSet;

    // We only want to remesh chunks that were changed in this frame.
    let just_added: HashSet<_> = new_chunks.iter().collect();

    for (chunk_entity, chunk, children_opt) in &query
    {
        if just_added.contains(&chunk_entity)
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

        // Get neighboring chunks data.
        let neighbor_data = get_neighbor_chunk_data(chunk.pos, &chunk_map, &all_chunks_query);

        // Build the new meshes for the chunk, one per texture.
        let meshes_by_tex = mesh_chunk(chunk, &*block_list, &neighbor_data);

        // Spawn one child per (texture, mesh).
        commands.entity(chunk_entity).with_children(|parent| {
            for (tex_handle, mesh) in meshes_by_tex
            {
                let mesh_handle = meshes.add(mesh);

                // Standard material.
                let mat_handle = materials.add(StandardMaterial {
                    base_color_texture: Some(tex_handle.clone()),
                    alpha_mode: AlphaMode::Mask(0.5), // keep cut-out alpha
                    ..default()
                });

                // Spawn the mesh.
                parent.spawn((
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(mat_handle),
                    Visibility::default(),
                ));
            }
        });
    }
}

// Marker struct to help identify the FPS UI component, since there may be many
// Text components.
#[derive(Component)]
struct FpsText
{
    timer: Timer,
}

// This systems periodically updates the FPS text in the UI.
fn text_update_system(
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
