use std::{cmp, collections::HashMap};

use bevy::prelude::*;

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

// This function creates a chunk based on its position and the world seed.
// For now, the generation is simple and creates a flat world, but procedural
// generation could be implemented in this function.
pub fn load_raw_chunk(_seed: u64, pos: ChunkPos) -> Chunk
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
            }
        }
    }

    // The chunk is returned.
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
