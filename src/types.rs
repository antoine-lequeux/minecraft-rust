use std::ops::{Add, Sub};

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

// Chunk size data.
pub const CHUNK_SIZE: u16 = 16;
pub const CHUNK_HEIGHT: u16 = 256;
pub const TOTAL: usize = (CHUNK_SIZE as usize).pow(2) * CHUNK_HEIGHT as usize;

// How many chunks should be loaded in each direction.
pub const RENDER_DISTANCE: i32 = 64;

// How many chunk loading tasks can be running at the same time.
pub const MAX_CONCURRENT_LOADS: usize = 64;

// Block types are hard-coded but should be loaded from a file later.
#[repr(u16)]
#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, Serialize_repr, Deserialize_repr)]
pub enum BlockType
{
    Air = 0,
    Grass = 1,
    Dirt = 2,
    Stone = 3,
    Sand = 4,
    Clay = 5,
    Gravel = 6,
    OakLog = 7,
    OakLeaves = 8,
    Water = 9,
}

// A struct representing the horizontal position of a chunk. It can serve as an
// ID for a chunk.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct ChunkPos
{
    pub x: i32,
    pub y: i32,
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
