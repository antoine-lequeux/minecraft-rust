use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};

use crate::{
    blocks::BlockList,
    chunks::{Chunk, Map},
    types::{BlockType, CHUNK_HEIGHT, CHUNK_SIZE, ChunkPos},
};

// The faces of each block, used for mesh generation.
// We keep this to get normal vectors, but we'll generate the quads dynamically.
pub const FACES: &[(IVec3, [[f32; 3]; 4], [[f32; 2]; 4])] = &[
    // +X (0)
    (
        IVec3::new(1, 0, 0),
        [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]],
        [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
    ),
    // -X (1)
    (
        IVec3::new(-1, 0, 0),
        [[0., 0., 1.], [0., 1., 1.], [0., 1., 0.], [0., 0., 0.]],
        [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
    ),
    // +Y (top) (2)
    (
        IVec3::new(0, 1, 0),
        [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]],
        [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
    ),
    // -Y (bottom) (3)
    (
        IVec3::new(0, -1, 0),
        [[0., 0., 1.], [0., 0., 0.], [1., 0., 0.], [1., 0., 1.]],
        [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
    ),
    // +Z (4)
    (
        IVec3::new(0, 0, 1),
        [[1., 0., 1.], [1., 1., 1.], [0., 1., 1.], [0., 0., 1.]],
        [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
    ),
    // -Z (5)
    (
        IVec3::new(0, 0, -1),
        [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]],
        [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
    ),
];

// Helper function to check if a block is opaque at the given coordinates.
fn is_opaque(
    req_x: i32,
    req_y: i32,
    req_z: i32,
    chunk: &Chunk,
    block_list: &BlockList,
    neighbor_chunks: &HashMap<ChunkPos, Chunk>,
    cs: usize,
    cs_i32: i32,
    ch_i32: i32,
) -> bool
{
    if !(0 .. ch_i32).contains(&req_y)
    {
        return false;
    }
    let mut target_chunk_pos = chunk.pos;
    let mut req_x = req_x;
    let mut req_z = req_z;

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

    return false;
}

pub fn mesh_chunk(
    chunk: &Chunk,
    block_list: &BlockList,
    neighbor_chunks: &HashMap<ChunkPos, Chunk>,
) -> (Mesh, Mesh)
{
    let opaque_mesh = build_mesh_for(false, chunk, block_list, neighbor_chunks);
    let transparent_mesh = build_mesh_for(true, chunk, block_list, neighbor_chunks);
    return (opaque_mesh, transparent_mesh);
}

fn build_mesh_for(
    transparent_pass: bool,
    chunk: &Chunk,
    block_list: &BlockList,
    neighbor_chunks: &HashMap<ChunkPos, Chunk>,
) -> Mesh
{
    let mut pos = Vec::new();
    let mut norm = Vec::new();
    let mut uv = Vec::new();
    let mut color = Vec::new(); // Used to store layer_index
    let mut idx = Vec::new();

    let cs = CHUNK_SIZE as usize;
    let ch = CHUNK_HEIGHT as usize;
    let cs_i32 = CHUNK_SIZE as i32;
    let ch_i32 = CHUNK_HEIGHT as i32;

    let min_y = chunk.min_face_height.saturating_sub(1);
    let max_y = (chunk.max_face_height + 1).min(ch - 1);

    // X, Y, Z axes for greedy meshing
    // 0: +X, 1: -X
    // 2: +Y, 3: -Y
    // 4: +Z, 5: -Z

    for face_dir in 0 .. 6
    {
        let dir = FACES[face_dir].0;

        let (axis_u, axis_v, axis_main) = match face_dir
        {
            0 | 1 => (2, 1, 0), // X faces: U=Z, V=Y
            2 | 3 => (0, 2, 1), // Y faces: U=X, V=Z
            4 | 5 => (0, 1, 2), // Z faces: U=X, V=Y
            _ => unreachable!(),
        };

        let u_size = if axis_u == 1 { ch } else { cs };
        let v_size = if axis_v == 1 { ch } else { cs };
        let main_size = if axis_main == 1 { max_y + 1 } else { cs };
        let main_start = if axis_main == 1 { min_y } else { 0 };

        for m in main_start .. main_size
        {
            let mut mask = vec![vec![None; u_size]; v_size];

            // Build mask for this slice
            for v in 0 .. v_size
            {
                // If V is Y-axis, we can skip if outside min_y..max_y
                if axis_v == 1 && (v < min_y || v > max_y)
                {
                    continue;
                }

                for u in 0 .. u_size
                {
                    // If U is Y-axis, skip if outside min_y..max_y
                    if axis_u == 1 && (u < min_y || u > max_y)
                    {
                        continue;
                    }

                    let mut x = 0;
                    let mut y = 0;
                    let mut z = 0;

                    if axis_main == 0
                    {
                        x = m;
                    }
                    else if axis_main == 1
                    {
                        y = m;
                    }
                    else
                    {
                        z = m;
                    }
                    if axis_u == 0
                    {
                        x = u;
                    }
                    else if axis_u == 1
                    {
                        y = u;
                    }
                    else
                    {
                        z = u;
                    }
                    if axis_v == 0
                    {
                        x = v;
                    }
                    else if axis_v == 1
                    {
                        y = v;
                    }
                    else
                    {
                        z = v;
                    }

                    let b = chunk.blocks[y * cs * cs + z * cs + x];
                    if b == BlockType::Air
                    {
                        continue;
                    }

                    // Filter based on pass
                    let is_block_transparent =
                        block_list.data.get(&b).map_or(false, |bd| bd.transparent);
                    if transparent_pass != is_block_transparent
                    {
                        continue;
                    }

                    let nx = x as i32 + dir.x;
                    let ny = y as i32 + dir.y;
                    let nz = z as i32 + dir.z;

                    if !is_opaque(
                        nx,
                        ny,
                        nz,
                        chunk,
                        block_list,
                        neighbor_chunks,
                        cs,
                        cs_i32,
                        ch_i32,
                    )
                    {
                        let tex_idx = block_list.data[&b].faces[face_dir];
                        mask[v][u] = Some(tex_idx);
                    }
                }
            }

            // Greedy mesh the mask
            for v in 0 .. v_size
            {
                for u in 0 .. u_size
                {
                    if let Some(tex_idx) = mask[v][u]
                    {
                        // Find width
                        let mut w = 1;
                        while u + w < u_size && mask[v][u + w] == Some(tex_idx)
                        {
                            w += 1;
                        }

                        // Find height
                        let mut h = 1;
                        'height_loop: while v + h < v_size
                        {
                            for k in 0 .. w
                            {
                                if mask[v + h][u + k] != Some(tex_idx)
                                {
                                    break 'height_loop;
                                }
                            }
                            h += 1;
                        }

                        // Clear mask
                        for i in 0 .. h
                        {
                            for j in 0 .. w
                            {
                                mask[v + i][u + j] = None;
                            }
                        }

                        // Emit quad
                        let mut du = [0; 3];
                        let mut dv = [0; 3];
                        du[axis_u] = w as i32;
                        dv[axis_v] = h as i32;

                        let mut base_x = 0;
                        let mut base_y = 0;
                        let mut base_z = 0;
                        if axis_main == 0
                        {
                            base_x = m as i32;
                        }
                        else if axis_main == 1
                        {
                            base_y = m as i32;
                        }
                        else
                        {
                            base_z = m as i32;
                        }
                        if axis_u == 0
                        {
                            base_x = u as i32;
                        }
                        else if axis_u == 1
                        {
                            base_y = u as i32;
                        }
                        else
                        {
                            base_z = u as i32;
                        }
                        if axis_v == 0
                        {
                            base_x = v as i32;
                        }
                        else if axis_v == 1
                        {
                            base_y = v as i32;
                        }
                        else
                        {
                            base_z = v as i32;
                        }

                        // Push vertices for the quad
                        let base_index = pos.len() as u32;

                        let mut p0 = [base_x as f32, base_y as f32, base_z as f32];
                        let mut p1 = [
                            base_x as f32 + du[0] as f32,
                            base_y as f32 + du[1] as f32,
                            base_z as f32 + du[2] as f32,
                        ];
                        let mut p2 = [
                            base_x as f32 + du[0] as f32 + dv[0] as f32,
                            base_y as f32 + du[1] as f32 + dv[1] as f32,
                            base_z as f32 + du[2] as f32 + dv[2] as f32,
                        ];
                        let mut p3 = [
                            base_x as f32 + dv[0] as f32,
                            base_y as f32 + dv[1] as f32,
                            base_z as f32 + dv[2] as f32,
                        ];

                        // Depending on the face direction, we need to adjust the quad vertices to
                        // align with the voxel boundaries and normals
                        if dir.x == 1
                        {
                            p0[0] += 1.;
                            p1[0] += 1.;
                            p2[0] += 1.;
                            p3[0] += 1.;
                        }
                        if dir.y == 1
                        {
                            p0[1] += 1.;
                            p1[1] += 1.;
                            p2[1] += 1.;
                            p3[1] += 1.;
                        }
                        if dir.z == 1
                        {
                            p0[2] += 1.;
                            p1[2] += 1.;
                            p2[2] += 1.;
                            p3[2] += 1.;
                        }

                        // Winding order adjustment
                        let reverse = dir.x > 0 || dir.y > 0 || dir.z < 0;

                        if reverse
                        {
                            pos.push(p0);
                            pos.push(p3);
                            pos.push(p2);
                            pos.push(p1);
                        }
                        else
                        {
                            pos.push(p0);
                            pos.push(p1);
                            pos.push(p2);
                            pos.push(p3);
                        }

                        // Normal
                        for _ in 0 .. 4
                        {
                            norm.push([dir.x as f32, dir.y as f32, dir.z as f32]);
                        }

                        // UVs based on width and height
                        let uv_w = w as f32;
                        let uv_h = h as f32;

                        if reverse
                        {
                            uv.push([0.0, uv_h]);
                            uv.push([0.0, 0.0]);
                            uv.push([uv_w, 0.0]);
                            uv.push([uv_w, uv_h]);
                        }
                        else
                        {
                            uv.push([0.0, uv_h]);
                            uv.push([uv_w, uv_h]);
                            uv.push([uv_w, 0.0]);
                            uv.push([0.0, 0.0]);
                        }

                        // Vertex color stores texture index in R channel
                        let c = [tex_idx as f32 / 255.0, 1.0, 1.0, 1.0];
                        for _ in 0 .. 4
                        {
                            color.push(c);
                        }

                        // Indices
                        idx.extend_from_slice(&[
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
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, norm);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, color);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

pub fn remesh_changed_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    global_mat: Res<crate::voxel_material::GlobalMaterials>,
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
        let (opaque_mesh, transparent_mesh) = mesh_chunk(chunk, &*block_list, &neighbor_data);

        // Spawn one child per (texture, mesh).
        commands.entity(chunk_entity).with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(opaque_mesh)),
                MeshMaterial3d(global_mat.opaque.clone()),
                Visibility::default(),
            ));
            parent.spawn((
                Mesh3d(meshes.add(transparent_mesh)),
                MeshMaterial3d(global_mat.transparent.clone()),
                Visibility::default(),
            ));
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
                    chunk.remesh_flag = true;
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
