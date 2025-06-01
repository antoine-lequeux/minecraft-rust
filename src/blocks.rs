use std::collections::HashMap;

use bevy::prelude::*;

use crate::types::BlockType;

// A block has a set of 6 textures, one per face. Later, it could have more
// data, for example light emission, or a shape...
#[derive(Clone)]
pub struct Block
{
    pub faces: [Handle<Image>; 6],
    pub transparent: bool,
}

// A simple way to associate blocks to chunks without copying each fields.
#[derive(Resource, Default, Clone)]
pub struct BlockList
{
    pub data: HashMap<BlockType, Block>,
}

// The TextureHandles resource stores handles to all block textures.
#[derive(Resource)]
pub struct TextureHandles
{
    pub grass_side: Handle<Image>,
    pub grass_top: Handle<Image>,
    pub dirt: Handle<Image>,
    pub stone: Handle<Image>,
    pub sand: Handle<Image>,
    pub clay: Handle<Image>,
    pub gravel: Handle<Image>,
    pub oak_log_inside: Handle<Image>,
    pub oak_log_outside: Handle<Image>,
    pub oak_leaves: Handle<Image>,
}

// This system is called when the game launches. The data is hard-coded but
// should be read from a file eventually.
pub fn load_block_types(mut list: ResMut<BlockList>, textures: Res<TextureHandles>)
{
    list.data.insert(
        BlockType::Air,
        Block {
            faces: [
                Handle::default(), // +X
                Handle::default(), // -X
                Handle::default(), // +Y (top)
                Handle::default(), // -Y (bottom)
                Handle::default(), // +Z
                Handle::default(), // -Z
            ],
            transparent: true,
        },
    );

    list.data.insert(
        BlockType::Grass,
        Block {
            faces: [
                textures.grass_side.clone(), // +X
                textures.grass_side.clone(), // -X
                textures.grass_top.clone(),  // +Y (top)
                textures.dirt.clone(),       // -Y (bottom)
                textures.grass_side.clone(), // +Z
                textures.grass_side.clone(), // -Z
            ],
            transparent: false,
        },
    );

    list.data.insert(
        BlockType::Dirt,
        Block {
            faces: [
                textures.dirt.clone(), // +X
                textures.dirt.clone(), // -X
                textures.dirt.clone(), // +Y (top)
                textures.dirt.clone(), // -Y (bottom)
                textures.dirt.clone(), // +Z
                textures.dirt.clone(), // -Z
            ],
            transparent: false,
        },
    );

    list.data.insert(
        BlockType::Stone,
        Block {
            faces: [
                textures.stone.clone(), // +X
                textures.stone.clone(), // -X
                textures.stone.clone(), // +Y (top)
                textures.stone.clone(), // -Y (bottom)
                textures.stone.clone(), // +Z
                textures.stone.clone(), // -Z
            ],
            transparent: false,
        },
    );

    list.data.insert(
        BlockType::Sand,
        Block {
            faces: [
                textures.sand.clone(), // +X
                textures.sand.clone(), // -X
                textures.sand.clone(), // +Y (top)
                textures.sand.clone(), // -Y (bottom)
                textures.sand.clone(), // +Z
                textures.sand.clone(), // -Z
            ],
            transparent: false,
        },
    );

    list.data.insert(
        BlockType::Clay,
        Block {
            faces: [
                textures.clay.clone(), // +X
                textures.clay.clone(), // -X
                textures.clay.clone(), // +Y (top)
                textures.clay.clone(), // -Y (bottom)
                textures.clay.clone(), // +Z
                textures.clay.clone(), // -Z
            ],
            transparent: false,
        },
    );

    list.data.insert(
        BlockType::Gravel,
        Block {
            faces: [
                textures.gravel.clone(), // +X
                textures.gravel.clone(), // -X
                textures.gravel.clone(), // +Y (top)
                textures.gravel.clone(), // -Y (bottom)
                textures.gravel.clone(), // +Z
                textures.gravel.clone(), // -Z
            ],
            transparent: false,
        },
    );

    list.data.insert(
        BlockType::OakLog,
        Block {
            faces: [
                textures.oak_log_outside.clone(), // +X
                textures.oak_log_outside.clone(), // -X
                textures.oak_log_inside.clone(),  // +Y (top)
                textures.oak_log_inside.clone(),  // -Y (bottom)
                textures.oak_log_outside.clone(), // +Z
                textures.oak_log_outside.clone(), // -Z
            ],
            transparent: false,
        },
    );

    list.data.insert(
        BlockType::OakLeaves,
        Block {
            faces: [
                textures.oak_leaves.clone(), // +X
                textures.oak_leaves.clone(), // -X
                textures.oak_leaves.clone(), // +Y (top)
                textures.oak_leaves.clone(), // -Y (bottom)
                textures.oak_leaves.clone(), // +Z
                textures.oak_leaves.clone(), // -Z
            ],
            transparent: true,
        },
    );
}
