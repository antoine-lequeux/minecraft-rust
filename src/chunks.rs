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

// This function generates a chunk using the seed and position.
// It uses a noise function to generate terrain height, and fills the chunk.
pub fn load_raw_chunk(seed: u64, pos: ChunkPos) -> Chunk
{
    let mut blocks = [BlockType::Air; TOTAL];
    let mut terrain_heights = [[0usize; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

    // Initialize the noise function.
    let fbm = Fbm::new()
        .set_seed(seed as u32)
        .set_octaves(2)
        .set_persistence(0.4)
        .set_lacunarity(2.0);

    // Constants for the noise function.
    let freq = 0.01;
    let scale = 0.2;
    let offset = -0.45; // Mean height of the terrain (-1.0 -> y = 0, 1.0 -> y = CHUNK_HEIGHT)
    let cs = CHUNK_SIZE as usize;
    let cs_sq = cs * cs;

    // Iterate over the chunk's x and z coordinates.
    for z in 0 .. cs
    {
        for x in 0 .. cs
        {
            // World coordinates of the block.
            let world_x = pos.x * CHUNK_SIZE as i32 + x as i32;
            let world_z = pos.y * CHUNK_SIZE as i32 + z as i32;

            // Get the raw height value from the noise function.
            let raw = fbm.get([world_x as f64 * freq, world_z as f64 * freq]);

            // Apply the scale and offset to the raw height value.
            let height_val = raw * scale + offset;

            // Go from [-1, 1] to [0, CHUNK_HEIGHT].
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

            // Store the terrain height for shore detection later.
            terrain_heights[z][x] = terrain_height;

            // Get the offset in the blocks array for this column.
            let column_offset = z * cs + x;

            // Initial value of idx.
            let mut idx = column_offset;

            // Iterate over the chunk's height.
            for y in 0 .. (CHUNK_HEIGHT as usize)
            {
                // Determine if this block is underwater and close to the surface.
                let is_underwater_surface = y < terrain_height
                    && y >= terrain_height.saturating_sub(2)
                    && y <= 64
                    && y >= 60;

                blocks[idx] = if y < terrain_height
                {
                    // Underground blocks.
                    if is_underwater_surface
                    {
                        // Clay blocks under water.
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
                // Above ground blocks.
                else if y == terrain_height
                {
                    if y <= 64
                    {
                        // Underwater floor is clay.
                        BlockType::Clay
                    }
                    else if terrain_height <= 66 && terrain_height > 64
                    {
                        // Blocks right above the water level are sand.
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

                // Add cs_sq to idx to move to the next block in the column.
                idx += cs_sq;
            }
        }
    }

    Chunk { pos, blocks: blocks.into() }
}

// This function applies any saved modifications to a chunk after it is loaded.
pub fn apply_modifications(chunk: &mut Chunk, modifications: &[Modification])
{
    for modification in modifications
    {
        chunk.blocks[modification.index] = modification.new;
    }
}

pub fn count_chunks(query: Query<(), With<Chunk>>)
{
    println!("Loaded chunks: {}", query.iter().count());
}
