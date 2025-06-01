use std::ops::{Add, Sub};

// Chunk size data.
pub const CHUNK_SIZE: u16 = 16;
pub const CHUNK_HEIGHT: u16 = 256;
pub const TOTAL: usize = (CHUNK_SIZE as usize).pow(2) * CHUNK_HEIGHT as usize;

// How many chunks should be loaded in each direction.
pub const RENDER_DISTANCE: i32 = 32;

// How many chunk loading tasks can be running at the same time.
pub const MAX_CONCURRENT_LOADS: usize = 16;

// Block types are hard-coded but should be loaded from a file later.
#[repr(u16)]
#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub enum BlockType
{
    Air,
    Grass,
    Dirt,
    Stone,
    Sand,
    Clay,
    Gravel,
    OakLog,
    OakLeaves,
    Water,
}

// A struct representing the horizontal position of a chunk. It can serve as an
// ID for a chunk.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
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
