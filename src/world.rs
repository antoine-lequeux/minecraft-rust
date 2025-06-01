use std::collections::HashMap;

use bevy::{
    math::ops::sqrt,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
};

use crate::{
    blocks::BlockList,
    camera::FlyCam,
    chunks::{Chunk, Map, apply_modifications, load_raw_chunk},
    meshing::mesh_chunk,
    types::{CHUNK_HEIGHT, CHUNK_SIZE, ChunkPos, MAX_CONCURRENT_LOADS, RENDER_DISTANCE},
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
}

// Tracks the meshing state of chunks.
#[derive(Resource, Default)]
pub struct ChunkMeshState
{
    pub tasks: HashMap<ChunkPos, Task<(ChunkPos, HashMap<Handle<Image>, Mesh>)>>,
}

// This system manages the loading and unloading of chunks based on their
// position.
pub fn manage_chunk_loading(
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
    let fov_cos = (120.0f32.to_radians() / 2.0).cos();
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

    // Sort desired chunks by distance from player for priority loading.
    // Closer chunks will be loaded first.
    desired.sort_by(|a, b| {
        let dist_a = sqrt(
            ((a.x - player_chunk.x) * (a.x - player_chunk.x)
                + (a.y - player_chunk.y) * (a.y - player_chunk.y)) as f32,
        );
        let dist_b = sqrt(
            ((b.x - player_chunk.x) * (b.x - player_chunk.x)
                + (b.y - player_chunk.y) * (b.y - player_chunk.y)) as f32,
        );
        dist_a
            .partial_cmp(&dist_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

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

// This system processes completed chunk loading tasks.
pub fn process_chunk_tasks(
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
pub fn process_chunk_mesh_tasks(
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
