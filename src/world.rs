use std::collections::HashMap;

use bevy::{
    math::ops::sqrt,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
};

use crate::{
    GlobalMaterials,
    blocks::BlockList,
    camera::FlyCam,
    chunks::{Chunk, Map, apply_modifications, load_raw_chunk},
    meshing::mesh_chunk,
    types::{CHUNK_SIZE, ChunkPos, MAX_CONCURRENT_LOADS, RENDER_DISTANCE},
};

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
pub struct ChunkLoadState
{
    pub tasks: HashMap<ChunkPos, Task<(ChunkPos, Chunk)>>,
    pub last_player_chunk: Option<ChunkPos>,
    pub sorted_desired: Vec<ChunkPos>,
    pub desired_set: std::collections::HashSet<ChunkPos>,
}

// Tracks the meshing state of chunks.
#[derive(Resource, Default)]
pub struct ChunkMeshState
{
    pub tasks: HashMap<ChunkPos, Task<(ChunkPos, Mesh, Mesh)>>,
}

// Tracks chunks that are loaded but waiting to be meshed (because they wait for neighbors).
#[derive(Resource, Default)]
pub struct ChunkMeshQueue
{
    pub queue: std::collections::HashSet<ChunkPos>,
}

// This system manages the loading and unloading of chunks based on their position.
pub fn manage_chunk_loading(
    mut commands: Commands,
    map: Res<Map>,
    mut chunk_map: ResMut<ChunkMap>,
    mut chunk_state: ResMut<ChunkLoadState>,
    camera: Query<&Transform, With<FlyCam>>,
)
{
    // Get player chunk position and camera info.
    let (player_chunk, _, _) = match camera.single()
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

    // The player's chunk's position will be used to determine the chunks that
    // should be loaded right now. This is where the render distance is useful.
    if chunk_state.last_player_chunk != Some(player_chunk)
    {
        let mut desired = Vec::new();
        let mut desired_set = std::collections::HashSet::new();
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
                desired.push(pos);
                desired_set.insert(pos);
            }
        }

        // Sort desired chunks by distance from player for priority loading.
        // Closer chunks will be loaded first.
        desired.sort_by(|a, b| {
            let dist_a = sqrt(
                ((a.x - player_chunk.x) * (a.x - player_chunk.x) + (a.y - player_chunk.y) * (a.y - player_chunk.y))
                    as f32,
            );
            let dist_b = sqrt(
                ((b.x - player_chunk.x) * (b.x - player_chunk.x) + (b.y - player_chunk.y) * (b.y - player_chunk.y))
                    as f32,
            );
            dist_a
                .partial_cmp(&dist_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        chunk_state.sorted_desired = desired;
        chunk_state.desired_set = desired_set;
        chunk_state.last_player_chunk = Some(player_chunk);
    }

    // Chunks that are currenlty loaded but are not wanted will be unloaded.
    for old_pos in chunk_map.loaded_chunks.keys().cloned().collect::<Vec<_>>()
    {
        if !chunk_state.desired_set.contains(&old_pos)
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
    for pos in chunk_state.sorted_desired.clone()
    {
        // Skip if chunk is already loaded or being loaded.
        if chunk_map.loaded_chunks.contains_key(&pos) || chunk_state.tasks.contains_key(&pos)
        {
            // Already loaded or being loaded.
            continue;
        }

        // Limit the number of concurrent loading tasks to prioritize closer chunks.
        if chunk_state.tasks.len() >= MAX_CONCURRENT_LOADS
        {
            break;
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

// Helper function to get data for neighboring chunks.
pub fn get_neighbor_chunk_data(
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
    for offset in neighbor_offsets
    {
        let neighbor_pos = current_chunk_pos + offset;
        if let Some(entity) = chunk_map.loaded_chunks.get(&neighbor_pos)
        {
            if let Ok(chunk_component) = all_chunks_query.get(*entity)
            {
                neighbor_chunks_data.insert(neighbor_pos, chunk_component.clone());
            }
        }
    }
    return neighbor_chunks_data;
}

// This system processes completed chunk loading tasks.
pub fn process_chunk_tasks(
    mut commands: Commands,
    mut chunk_map: ResMut<ChunkMap>,
    mut chunk_state: ResMut<ChunkLoadState>,
    mut mesh_queue: ResMut<ChunkMeshQueue>,
)
{
    use std::task::{Context, Poll};

    use futures_util::task::noop_waker_ref;

    let mut completed = Vec::new();
    for (&task_pos, task) in chunk_state.tasks.iter_mut()
    {
        let waker = noop_waker_ref();
        let mut cx = Context::from_waker(waker);
        if let Poll::Ready((chunk_pos, chunk)) = std::pin::Pin::new(task).poll(&mut cx)
        {
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
            mesh_queue.queue.insert(chunk_pos);
            completed.push(task_pos);
        }
    }
    for pos in completed
    {
        chunk_state.tasks.remove(&pos);
    }
}

// This system checks if chunks in the mesh queue have all 4 horizontal neighbors loaded.
// If so, it removes them from the queue and spawns a meshing task.
pub fn queue_chunk_meshes(
    mut mesh_queue: ResMut<ChunkMeshQueue>,
    mut mesh_state: ResMut<ChunkMeshState>,
    chunk_map: Res<ChunkMap>,
    block_list: Res<BlockList>,
    all_chunks_query: Query<&Chunk>,
)
{
    let mut to_mesh = Vec::new();

    // Check which chunks have all neighbors loaded.
    for &pos in mesh_queue.queue.iter()
    {
        let neighbor_offsets = [
            ChunkPos { x: -1, y: 0 },
            ChunkPos { x: 1, y: 0 },
            ChunkPos { x: 0, y: -1 },
            ChunkPos { x: 0, y: 1 },
        ];

        let mut all_loaded = true;
        for offset in neighbor_offsets
        {
            if !chunk_map.loaded_chunks.contains_key(&(pos + offset))
            {
                all_loaded = false;
                break;
            }
        }

        if all_loaded
        {
            to_mesh.push(pos);
        }
    }

    let task_pool = AsyncComputeTaskPool::get();

    // Spawn meshing tasks for chunks that have all neighbors.
    for pos in to_mesh
    {
        mesh_queue.queue.remove(&pos);

        if let Some(&entity) = chunk_map.loaded_chunks.get(&pos)
        {
            if let Ok(chunk_component) = all_chunks_query.get(entity)
            {
                let chunk_clone = chunk_component.clone();
                let block_list = block_list.clone();
                let chunk_pos_copy = pos;

                let neighbor_chunks_data = get_neighbor_chunk_data(pos, &chunk_map, &all_chunks_query);

                let mesh_task = task_pool.spawn(async move {
                    let (opaque_mesh, transparent_mesh) = mesh_chunk(&chunk_clone, &block_list, &neighbor_chunks_data);
                    (chunk_pos_copy, opaque_mesh, transparent_mesh)
                });

                mesh_state.tasks.insert(pos, mesh_task);
            }
        }
    }
}

// This system processes completed chunk meshing tasks.
pub fn process_chunk_mesh_tasks(
    mut commands: Commands,
    mut mesh_state: ResMut<ChunkMeshState>,
    mut meshes: ResMut<Assets<Mesh>>,
    global_mat: Res<GlobalMaterials>,
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
        if let Poll::Ready((chunk_pos, opaque_mesh, transparent_mesh)) = std::pin::Pin::new(task).poll(&mut cx)
        {
            if let Some(&chunk_entity) = chunk_map.loaded_chunks.get(&chunk_pos)
            {
                let opaque_handle = meshes.add(opaque_mesh);
                let transparent_handle = meshes.add(transparent_mesh);

                commands.entity(chunk_entity).with_children(|c| {
                    c.spawn((
                        Mesh3d(opaque_handle),
                        MeshMaterial3d(global_mat.opaque.clone()),
                        Visibility::default(),
                    ));
                    c.spawn((
                        Mesh3d(transparent_handle),
                        MeshMaterial3d(global_mat.transparent.clone()),
                        Visibility::default(),
                    ));
                });
            }
            completed.push(chunk_pos);
        }
    }
    for pos in completed
    {
        mesh_state.tasks.remove(&pos);
    }
}
