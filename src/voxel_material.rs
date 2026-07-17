use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderRef},
};

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct VoxelMaterialExtension
{
    #[texture(100)]
    #[sampler(101)]
    pub atlas_texture: Handle<Image>,
}

impl MaterialExtension for VoxelMaterialExtension
{
    fn fragment_shader() -> ShaderRef
    {
        "shaders/voxel_material.wgsl".into()
    }
}

pub type VoxelMaterial = ExtendedMaterial<StandardMaterial, VoxelMaterialExtension>;

#[derive(Resource)]
pub struct GlobalMaterials
{
    pub opaque: Handle<VoxelMaterial>,
    pub transparent: Handle<VoxelMaterial>,
}
