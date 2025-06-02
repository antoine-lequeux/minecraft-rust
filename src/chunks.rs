use std::collections::HashMap;

use bevy::prelude::*;
use noise::{Fbm, MultiFractal, NoiseFn, Seedable};

use crate::types::{BlockType, CHUNK_HEIGHT, CHUNK_SIZE, ChunkPos, TOTAL};

// The Chunk component.
#[derive(Component, Clone)]
pub struct Chunk
{
    pub pos: ChunkPos,
    pub blocks: Box<[BlockType; TOTAL]>,
}

// Each chunk is always loaded using the seed, instead of being saved.
// But we then need to store the modifications that were applied to each chunk,
// else they will be lost when the player is too far.
#[derive(Clone)]
pub struct Modification
{
    pub index: usize,
    pub new: BlockType,
}

// The Map resource. It stores the seed for this world, and a list of
// modifications that were applied to each chunk.
#[derive(Resource, Default)]
pub struct Map
{
    pub seed: u64,
    pub modified: HashMap<ChunkPos, Vec<Modification>>,
}

impl Map
{
    pub fn new(seed: u64) -> Self
    {
        Map { seed, modified: HashMap::new() }
    }
}

// Helper function to generate a single column of blocks at (x, z).
fn generate_column(
    fbm: &Fbm,
    pos: ChunkPos,
    x: usize,
    z: usize,
    blocks: &mut [BlockType; TOTAL],
    terrain_heights: &mut [[usize; CHUNK_SIZE as usize]; CHUNK_SIZE as usize],
)
{
    let cs = CHUNK_SIZE as usize;
    let cs_sq = cs * cs;
    let freq = 0.01;
    let scale = 0.2;
    let offset = -0.45;
    let world_x = pos.x * CHUNK_SIZE as i32 + x as i32;
    let world_z = pos.y * CHUNK_SIZE as i32 + z as i32;
    let raw = fbm.get([world_x as f64 * freq, world_z as f64 * freq]);
    let height_val = raw * scale + offset;
    let mut terrain_height = ((height_val + 1.0) / 2.0 * CHUNK_HEIGHT as f64) as isize;
    if terrain_height < 0
    {
        terrain_height = 0;
    }
    if terrain_height as usize > CHUNK_HEIGHT as usize
    {
        terrain_height = CHUNK_HEIGHT as isize;
    }
    let terrain_height = terrain_height as usize;
    terrain_heights[z][x] = terrain_height;
    let column_offset = z * cs + x;
    let mut idx = column_offset;
    for y in 0 .. (CHUNK_HEIGHT as usize)
    {
        let is_underwater_surface =
            y < terrain_height && y >= terrain_height.saturating_sub(2) && y <= 64 && y >= 60;
        blocks[idx] = if y < terrain_height
        {
            if is_underwater_surface
            {
                BlockType::Clay
            }
            else if y < terrain_height.saturating_sub(4)
            {
                BlockType::Stone
            }
            else
            {
                BlockType::Dirt
            }
        }
        else if y == terrain_height
        {
            if y <= 64
            {
                BlockType::Clay
            }
            else if terrain_height <= 66 && terrain_height > 64
            {
                BlockType::Sand
            }
            else
            {
                BlockType::Grass
            }
        }
        else if y <= 64
        {
            BlockType::Water
        }
        else
        {
            BlockType::Air
        };
        idx += cs_sq;
    }
}

// This function generates a chunk using the seed and position.
// It uses a noise function to generate terrain height, and fills the chunk.
pub fn load_raw_chunk(seed: u64, pos: ChunkPos) -> Chunk
{
    let mut blocks = [BlockType::Air; TOTAL];
    let mut terrain_heights = [[0_usize; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

    // Initialize the noise function.
    let fbm = create_fbm(seed);

    // Constants for the noise function.
    let cs = CHUNK_SIZE as usize;

    // Iterate over the chunk's x and z coordinates.
    for z in 0 .. cs
    {
        for x in 0 .. cs
        {
            generate_column(&fbm, pos, x, z, &mut blocks, &mut terrain_heights);
        }
    }

    Chunk { pos, blocks: blocks.into() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChunkFace
{
    North,
    South,
    East,
    West,
}

// Load only a specific face of the chunk.
pub fn load_chunk_face(seed: u64, pos: ChunkPos, face: ChunkFace) -> Chunk
{
    let mut blocks = [BlockType::Air; TOTAL];
    let mut terrain_heights = [[0_usize; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
    // Initialize the noise function.
    let fbm = create_fbm(seed);
    let cs = CHUNK_SIZE as usize;
    match face
    {
        ChunkFace::North =>
        {
            let z = 0;
            for x in 0 .. cs
            {
                generate_column(&fbm, pos, x, z, &mut blocks, &mut terrain_heights);
            }
        },
        ChunkFace::South =>
        {
            let z = cs - 1;
            for x in 0 .. cs
            {
                generate_column(&fbm, pos, x, z, &mut blocks, &mut terrain_heights);
            }
        },
        ChunkFace::West =>
        {
            let x = 0;
            for z in 0 .. cs
            {
                generate_column(&fbm, pos, x, z, &mut blocks, &mut terrain_heights);
            }
        },
        ChunkFace::East =>
        {
            let x = cs - 1;
            for z in 0 .. cs
            {
                generate_column(&fbm, pos, x, z, &mut blocks, &mut terrain_heights);
            }
        },
    }
    Chunk { pos, blocks: blocks.into() }
}

// Helper function to create a noise function for terrain generation.
fn create_fbm(seed: u64) -> Fbm
{
    Fbm::new()
        .set_seed(seed as u32)
        .set_octaves(2)
        .set_persistence(0.4)
        .set_lacunarity(2.0)
}

// This function applies any saved modifications to a chunk after it is loaded.
pub fn apply_modifications(chunk: &mut Chunk, modifications: &[Modification])
{
    for modification in modifications
    {
        // Skip dummy modifications used only for triggering remeshing.
        if modification.index == usize::MAX
        {
            continue;
        }
        chunk.blocks[modification.index] = modification.new;
    }
}

pub fn count_chunks(query: Query<(), With<Chunk>>)
{
    println!("Loaded chunks: {}", query.iter().count());
}
