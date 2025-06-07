use std::collections::HashMap;

use bevy::prelude::*;
use noise::{NoiseFn, OpenSimplex};

use crate::types::{BlockType, CHUNK_HEIGHT, CHUNK_SIZE, ChunkPos, TOTAL};

// The Chunk component.
#[derive(Component, Clone)]
pub struct Chunk
{
    pub pos: ChunkPos,
    pub blocks: Box<[BlockType; TOTAL]>,
    pub min_face_height: usize, // Minimum height where faces could be drawn.
    pub max_face_height: usize, // Maximum height where faces could be drawn.
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
    pub seed: u32,
    pub modified: HashMap<ChunkPos, Vec<Modification>>,
}

impl Map
{
    pub fn new(seed: u32) -> Self
    {
        return Map { seed, modified: HashMap::new() };
    }
}

// Configuration for terrain generation.
#[derive(Clone)]
pub struct TerrainConfig
{
    // Continental scale settings (large-scale terrain type distribution).
    pub continent_frequency: f64,   // How large the terrain regions are.
    pub continent_threshold: f64,   // Where to split flat vs. mountainous (-1 to 1).
    pub transition_smoothness: f64, // How smooth the transitions are (lower = smoother).

    // Mountain generation settings.
    pub mountain_base_frequency: f64, // Base frequency for mountain detail.
    pub mountain_octaves: i32,        // Number of octaves for mountain noise.
    pub mountain_persistence: f64,    // How much each octave contributes.
    pub mountain_lacunarity: f64,     // Frequency multiplier between octaves.
    pub mountain_amplitude: f64,      // Maximum height multiplier for mountains.
    pub mountain_base_height: f64,    // Base height for mountainous areas (0-1).

    // Plains generation settings.
    pub plains_base_frequency: f64, // Base frequency for plains detail.
    pub plains_octaves: i32,        // Number of octaves for plains noise.
    pub plains_persistence: f64,    // How much each octave contributes.
    pub plains_lacunarity: f64,     // Frequency multiplier between octaves.
    pub plains_amplitude: f64,      // Maximum height variation for plains.
    pub plains_base_height: f64,    // Base height for flat areas (0-1).

    // Water level settings.
    pub water_level: f64, // Water level as fraction of CHUNK_HEIGHT (0-1).
}

impl Default for TerrainConfig
{
    fn default() -> Self
    {
        return TerrainConfig {
            continent_frequency: 0.0008, // Very large scale terrain regions.
            continent_threshold: 0.1,    // Favor more mountains than plains.
            transition_smoothness: 4.0,  // Smooth transitions but mountains are well defined.

            // Mountain settings.
            mountain_base_frequency: 0.006, // Detailed mountain features.
            mountain_octaves: 6,            // Rich detail.
            mountain_persistence: 0.6,      // Maintain detail across octaves.
            mountain_lacunarity: 2.0,       // Standard frequency doubling.
            mountain_amplitude: 0.8,        // Mountains can reach 80% of max height.
            mountain_base_height: 0.4,      // Mountains start at 40% height.

            // Plains settings.
            plains_base_frequency: 0.01, // Gentle rolling hills.
            plains_octaves: 3,           // Less detail for smoother look.
            plains_persistence: 0.4,     // Subtle variations.
            plains_lacunarity: 2.0,      // Standard frequency doubling.
            plains_amplitude: 0.15,      // Only 15% height variation.
            plains_base_height: 0.2,     // Plains start low for water generation.

            water_level: 0.25, // Water at 25% of chunk height (64/256).
        };
    }
}

// Compute a smooth interpolation weight based on continental noise.
// Returns a value from 0 (plains) to 1 (mountains) with smooth transitions.
fn terrain_blend_weight(sample: f64, config: &TerrainConfig) -> f64
{
    // Apply threshold and smoothness.
    let shifted = (sample - config.continent_threshold) * config.transition_smoothness;

    // Use smoothstep function for nice transitions.
    let t = ((shifted + 1.0) * 0.5).clamp(0.0, 1.0);

    // Smoothstep: 3t² - 2t³ for smoother transitions.
    return t * t * (3.0 - 2.0 * t);
}

// Generate fractal Brownian motion (fBm) noise using multiple octaves of
// Simplex.
fn generate_fbm(
    simplex: &OpenSimplex,
    x: f64,
    z: f64,
    base_frequency: f64,
    octaves: i32,
    persistence: f64,
    lacunarity: f64,
) -> f64
{
    let mut amplitude = 1.0;
    let mut frequency = base_frequency;
    let mut value = 0.0;
    let mut max_value = 0.0; // For normalization.

    for _ in 0 .. octaves
    {
        value += simplex.get([x * frequency, z * frequency]) * amplitude;
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    // Normalize to [-1, 1].
    return value / max_value;
}

fn create_simplex(seed: u32) -> OpenSimplex
{
    return OpenSimplex::new(seed);
}

// Generate one column at (x,z) using the terrain settings.
fn generate_column(
    simplex: &OpenSimplex,
    pos: ChunkPos,
    x: usize,
    z: usize,
    blocks: &mut [BlockType; TOTAL],
    terrain_heights: &mut [[usize; CHUNK_SIZE as usize]; CHUNK_SIZE as usize],
    config: &TerrainConfig,
)
{
    let cs = CHUNK_SIZE as usize;
    let cs_sq = cs * cs;

    // World coordinates of this block column.
    let world_x = pos.x * CHUNK_SIZE as i32 + x as i32;
    let world_z = pos.y * CHUNK_SIZE as i32 + z as i32;
    let wx = world_x as f64;
    let wz = world_z as f64;

    // Sample continental noise to determine if this is a mountain or plains area.
    let raw_continent =
        simplex.get([wx * config.continent_frequency, wz * config.continent_frequency]);
    let blend_weight = terrain_blend_weight(raw_continent, config);

    // Generate mountain noise.
    let mountain_noise = generate_fbm(
        simplex,
        wx,
        wz,
        config.mountain_base_frequency,
        config.mountain_octaves,
        config.mountain_persistence,
        config.mountain_lacunarity,
    );

    // Convert mountain noise [-1,1] to real height.
    let mountain_height =
        config.mountain_base_height + ((mountain_noise + 1.0) * 0.5) * config.mountain_amplitude;

    // Generate plains terrain using gentler fBm.
    let plains_noise = generate_fbm(
        simplex,
        wx,
        wz,
        config.plains_base_frequency,
        config.plains_octaves,
        config.plains_persistence,
        config.plains_lacunarity,
    );

    // Convert plains noise [-1,1] to real height.
    let plains_height =
        config.plains_base_height + ((plains_noise + 1.0) * 0.5) * config.plains_amplitude;

    // Blend between mountain and plains terrain.
    let normalized_height = blend_weight * mountain_height + (1.0 - blend_weight) * plains_height;

    // Convert to world height.
    let world_height =
        (normalized_height * CHUNK_HEIGHT as f64).clamp(0.0, CHUNK_HEIGHT as f64 - 1.0);
    let terrain_height = world_height as usize;
    terrain_heights[z][x] = terrain_height;

    // Fill blocks based on height and water level.
    let water_level = (config.water_level * CHUNK_HEIGHT as f64) as usize;

    let column_offset = z * cs + x;
    let mut idx = column_offset;

    for y in 0 .. (CHUNK_HEIGHT as usize)
    {
        // Determine if this is an underwater surface (clay layer near water body).
        let is_underwater_surface = y < terrain_height
            && y >= terrain_height.saturating_sub(2)
            && y <= water_level
            && y >= water_level.saturating_sub(4);

        blocks[idx] = if y < terrain_height
        {
            // Below surface.
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
            // Surface layer, choose based on height and biome.
            if y <= water_level
            {
                BlockType::Clay // Underwater surface.
            }
            else if terrain_height <= water_level + 2 && terrain_height > water_level
            {
                BlockType::Sand // Beach/shore areas.
            }
            else
            {
                BlockType::Grass // Above water surface.
            }
        }
        else if y <= water_level
        {
            // Above terrain but below water level.
            BlockType::Water
        }
        else
        {
            // Above water level.
            BlockType::Air
        };

        idx += cs_sq;
    }
}

// This function generates a chunk using the seed and position.
// It uses the default settings of the simplex noise.
pub fn load_raw_chunk(seed: u32, pos: ChunkPos) -> Chunk
{
    let mut blocks = [BlockType::Air; TOTAL];
    let mut terrain_heights = [[0_usize; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
    let config = TerrainConfig::default();

    let simplex = create_simplex(seed);

    let cs = CHUNK_SIZE as usize;
    for z in 0 .. cs
    {
        for x in 0 .. cs
        {
            generate_column(&simplex, pos, x, z, &mut blocks, &mut terrain_heights, &config);
        }
    }

    let (min_face_height, max_face_height) = calculate_face_heights(&blocks);
    return Chunk { pos, blocks: blocks.into(), min_face_height, max_face_height };
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
pub fn load_chunk_face(seed: u32, pos: ChunkPos, face: ChunkFace) -> Chunk
{
    let mut blocks = [BlockType::Air; TOTAL];
    let mut terrain_heights = [[0_usize; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
    let config = TerrainConfig::default();
    let simplex = create_simplex(seed);

    let cs = CHUNK_SIZE as usize;
    match face
    {
        ChunkFace::North =>
        {
            let z = 0;
            for x in 0 .. cs
            {
                generate_column(&simplex, pos, x, z, &mut blocks, &mut terrain_heights, &config);
            }
        },
        ChunkFace::South =>
        {
            let z = cs - 1;
            for x in 0 .. cs
            {
                generate_column(&simplex, pos, x, z, &mut blocks, &mut terrain_heights, &config);
            }
        },
        ChunkFace::West =>
        {
            let x = 0;
            for z in 0 .. cs
            {
                generate_column(&simplex, pos, x, z, &mut blocks, &mut terrain_heights, &config);
            }
        },
        ChunkFace::East =>
        {
            let x = cs - 1;
            for z in 0 .. cs
            {
                generate_column(&simplex, pos, x, z, &mut blocks, &mut terrain_heights, &config);
            }
        },
    }

    let (min_face_height, max_face_height) = calculate_face_heights(&blocks);
    return Chunk { pos, blocks: blocks.into(), min_face_height, max_face_height };
}

// Helper function to calculate the minimal and maximal heights for face
// drawing.
pub fn calculate_face_heights(blocks: &[BlockType; TOTAL]) -> (usize, usize)
{
    let cs = CHUNK_SIZE as usize;
    let cs_sq = cs * cs;
    let ch = CHUNK_HEIGHT as usize;

    // Find the lowest layer with any non-opaque blocks.
    let mut min_height = CHUNK_HEIGHT as usize;
    // Find the highest layer with any opaque blocks.
    let mut max_height = 0;

    for y in 0 .. (CHUNK_HEIGHT as usize)
    {
        let mut has_transparent = false;
        let mut has_opaque = false;

        // Check all blocks in this layer.
        for z in 0 .. cs
        {
            for x in 0 .. cs
            {
                let idx = y * cs_sq + z * cs + x;
                let block_type = blocks[idx];

                // Use the block's transparency property from BlockList.
                if block_type == BlockType::Air
                {
                    has_transparent = true;
                }
                else
                {
                    has_opaque = true;
                }

                // Early exit if we found both types.
                if has_transparent && has_opaque
                {
                    break;
                }
            }
            if has_transparent && has_opaque
            {
                break;
            }
        }

        // Lowest layer with any non-opaque blocks.
        if min_height == ch && has_transparent
        {
            min_height = y;
        }

        // Highest layer with any opaque blocks.
        if has_opaque
        {
            max_height = y;
        }
    }

    // If no transparent blocks found, set min_height to max_height.
    if min_height == ch
    {
        min_height = max_height;
    }

    return (min_height, max_height);
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

    // Recalculate face heights after applying modifications.
    let (min_face_height, max_face_height) = calculate_face_heights(&chunk.blocks);
    chunk.min_face_height = min_face_height;
    chunk.max_face_height = max_face_height;
}

pub fn count_chunks(query: Query<(), With<Chunk>>)
{
    println!("Loaded chunks: {}", query.iter().count());
}
