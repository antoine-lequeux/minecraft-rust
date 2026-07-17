use std::collections::HashMap;

use bevy::prelude::*;

use crate::types::BlockType;

// A block has a set of 6 textures, one per face. Later, it could have more
// data, for example light emission, or a shape...
#[derive(Clone)]
pub struct Block
{
    pub faces: [u8; 6],
    pub transparent: bool,
}

// A simple way to associate blocks to chunks without copying each fields.
#[derive(Resource, Default, Clone)]
pub struct BlockList
{
    pub data: HashMap<BlockType, Block>,
}

// Texture indices in the atlas:
// 0: grass_side
// 1: grass_top
// 2: dirt
// 3: stone
// 4: sand
// 5: clay
// 6: gravel
// 7: oak_log_inside
// 8: oak_log_outside
// 9: oak_leaves
// 10: water

// This system is called when the game launches.
pub fn load_block_types(mut list: ResMut<BlockList>)
{
    list.data
        .insert(BlockType::Air, Block { faces: [0, 0, 0, 0, 0, 0], transparent: true });

    list.data.insert(
        BlockType::Grass,
        Block {
            faces: [
                0, // +X: grass_side
                0, // -X: grass_side
                1, // +Y (top): grass_top
                2, // -Y (bottom): dirt
                0, // +Z: grass_side
                0, // -Z: grass_side
            ],
            transparent: false,
        },
    );

    list.data
        .insert(BlockType::Dirt, Block { faces: [2; 6], transparent: false });

    list.data
        .insert(BlockType::Stone, Block { faces: [3; 6], transparent: false });

    list.data
        .insert(BlockType::Sand, Block { faces: [4; 6], transparent: false });

    list.data
        .insert(BlockType::Clay, Block { faces: [5; 6], transparent: false });

    list.data
        .insert(BlockType::Gravel, Block { faces: [6; 6], transparent: false });

    list.data.insert(
        BlockType::OakLog,
        Block {
            faces: [
                8, // +X: outside
                8, // -X: outside
                7, // +Y (top): inside
                7, // -Y (bottom): inside
                8, // +Z: outside
                8, // -Z: outside
            ],
            transparent: false,
        },
    );

    list.data
        .insert(BlockType::OakLeaves, Block { faces: [9; 6], transparent: true });

    // Fake water texture.
    list.data
        .insert(BlockType::Water, Block { faces: [10; 6], transparent: false });
}
