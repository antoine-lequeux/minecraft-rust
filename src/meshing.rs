use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};

use crate::{
    ChunkFace, Map, Modification,
    blocks::BlockList,
    chunks::{Chunk, apply_modifications, load_chunk_face},
    types::{BlockType, CHUNK_HEIGHT, CHUNK_SIZE, ChunkPos},
};

// The faces of each block, used for mesh generation.
pub const FACES: &[(IVec3, [[f32; 3]; 4], [[f32; 2]; 4])] = &[
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
pub fn mesh_chunk(
    chunk: &Chunk,
    block_list: &BlockList,
    neighbor_chunks: &HashMap<ChunkPos, Chunk>,
    seed: u64,
    modifications: &std::collections::HashMap<ChunkPos, Vec<Modification>>,
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

    // Cache for generated chunk faces: (ChunkPos, face) -> Chunk.
    let mut face_cache: HashMap<(ChunkPos, ChunkFace), Chunk> = HashMap::new();

    // Helper function to check if a block is opaque at the given coordinates.
    fn is_opaque(
        req_x: i32,
        req_y: i32,
        req_z: i32,
        chunk: &Chunk,
        block_list: &BlockList,
        neighbor_chunks: &HashMap<ChunkPos, Chunk>,
        seed: u64,
        cs: usize,
        cs_i32: i32,
        ch_i32: i32,
        face_cache: &mut HashMap<(ChunkPos, ChunkFace), Chunk>,
        modifications: &std::collections::HashMap<ChunkPos, Vec<Modification>>,
    ) -> bool
    {
        if !(0 .. ch_i32).contains(&req_y)
        {
            // If the y coordinate is below 0 or above the chunk height, it's not part of
            // the world.
            return false;
        }
        // Get the chunk in which the block is located.
        let mut target_chunk_pos = chunk.pos;
        // Determine if we need to look in a neighbor chunk based on x coordinate.
        let mut face: Option<ChunkFace> = None;
        let mut req_x = req_x;
        let mut req_z = req_z;
        if req_x < 0
        {
            target_chunk_pos.x -= 1;
            req_x += cs_i32;
            face = Some(ChunkFace::East);
        }
        else if req_x >= cs_i32
        {
            target_chunk_pos.x += 1;
            req_x -= cs_i32;
            face = Some(ChunkFace::West);
        }
        if req_z < 0
        {
            target_chunk_pos.y -= 1;
            req_z += cs_i32;
            face = Some(ChunkFace::South);
        }
        else if req_z >= cs_i32
        {
            target_chunk_pos.y += 1;
            req_z -= cs_i32;
            face = Some(ChunkFace::North);
        }
        let chunk_data_to_use = if target_chunk_pos == chunk.pos
        {
            Some(chunk)
        }
        else
        {
            neighbor_chunks.get(&target_chunk_pos)
        };
        if let Some(selected_chunk) = chunk_data_to_use
        {
            let idx = (req_y as usize) * cs * cs + (req_z as usize) * cs + (req_x as usize);
            return block_list
                .data
                .get(&selected_chunk.blocks[idx])
                .map_or(false, |block| !block.transparent);
        }
        // If the neighbor chunk is not loaded yet, we generate the useful face.
        if let Some(face_name) = face
        {
            // If the face is already cached, we use it.
            let cache_key = (target_chunk_pos, face_name);
            let temp_chunk = face_cache.entry(cache_key).or_insert_with(|| {
                let mut chunk_face = load_chunk_face(seed, target_chunk_pos, face_name);
                // If there are modifications for this chunk, apply them.
                if let Some(mods) = modifications.get(&target_chunk_pos)
                {
                    apply_modifications(&mut chunk_face, mods);
                }
                chunk_face
            });
            // Check the block type at the requested coordinates in the cached chunk.
            let idx = (req_y as usize) * cs * cs + (req_z as usize) * cs + (req_x as usize);
            return block_list
                .data
                .get(&temp_chunk.blocks[idx])
                .map_or(false, |block| !block.transparent);
        }
        return true;
    }

    // Use the chunk's face height bounds to optimize iteration.
    // The ranges is slightly expanded to ensure we cover the full height of the
    // faces.
    let min_y = chunk.min_face_height.saturating_sub(1);
    let max_y = (chunk.max_face_height + 1).min(ch - 1);

    // Iterate over all blocks in the chunk, but only in the relevant Y range.
    for z_local in 0 .. cs
    {
        for y_local in min_y ..= max_y
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
                    if is_opaque(
                        neighbor_check_x,
                        neighbor_check_y,
                        neighbor_check_z,
                        chunk,
                        block_list,
                        neighbor_chunks,
                        seed,
                        cs,
                        cs_i32,
                        ch_i32,
                        &mut face_cache,
                        modifications,
                    )
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

// This system remeshes chunks that have changed since the last frame.
pub fn remesh_changed_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    map: Res<Map>,
    block_list: Res<BlockList>,
    query: Query<(Entity, &Chunk, Option<&Children>), Changed<Chunk>>,
    new_chunks: Query<Entity, Added<Chunk>>,
    chunk_map: Res<crate::world::ChunkMap>,
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
        let neighbor_data =
            crate::world::get_neighbor_chunk_data(chunk.pos, &chunk_map, &all_chunks_query);

        // Build the new meshes for the chunk, one per texture.
        let meshes_by_tex =
            mesh_chunk(chunk, &*block_list, &neighbor_data, map.seed, &map.modified);

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

// This system triggers remeshing for chunks that have modifications requiring
// remesh (like when adjacent chunks have border blocks modified).
pub fn trigger_chunk_remeshing(
    mut map: ResMut<Map>,
    chunk_map: Res<crate::world::ChunkMap>,
    mut chunks: Query<&mut Chunk>,
)
{
    let mut chunks_to_clean = Vec::new();

    for (chunk_pos, modifications) in &map.modified
    {
        // Check if this chunk has any dummy modifications (used for triggering
        // remeshing).
        let has_remesh_trigger = modifications.iter().any(|m| m.index == usize::MAX);

        if has_remesh_trigger
        {
            // Find the chunk entity and trigger a change.
            if let Some(&chunk_entity) = chunk_map.loaded_chunks.get(chunk_pos)
            {
                if let Ok(mut chunk) = chunks.get_mut(chunk_entity)
                {
                    // Simply touch the chunk to trigger the Change<Chunk> detection.
                    // We clone the position to force a change without actually changing data.
                    chunk.pos = chunk.pos;
                }
            }

            // Remove the dummy modifications but keep real ones.
            chunks_to_clean.push(*chunk_pos);
        }
    }

    // Clean up dummy modifications.
    for chunk_pos in chunks_to_clean
    {
        if let Some(modifications) = map.modified.get_mut(&chunk_pos)
        {
            modifications.retain(|m| m.index != usize::MAX);

            // If no real modifications remain, remove the entry completely.
            if modifications.is_empty()
            {
                map.modified.remove(&chunk_pos);
            }
        }
    }
}
