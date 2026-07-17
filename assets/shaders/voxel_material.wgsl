#import bevy_pbr::{
    forward_io::VertexOutput,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}

@group(2) @binding(100)
var atlas_texture: texture_2d<f32>;
@group(2) @binding(101)
var atlas_sampler: sampler;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {

    var pbr_input = pbr_input_from_standard_material(in, is_front);
    
    let layer_index = round(in.color.r * 255.0);
    
    let sub_uv = fract(in.uv);
    
    let v_offset = layer_index / 11.0;
    let final_uv = vec2<f32>(sub_uv.x, sub_uv.y / 11.0 + v_offset);
    
    let tex_color = textureSample(atlas_texture, atlas_sampler, final_uv);
    
    pbr_input.material.base_color = tex_color;
    
    if (tex_color.a < 0.5) {
        discard;
    }
    
    var out_color = apply_pbr_lighting(pbr_input);
    out_color = main_pass_post_lighting_processing(pbr_input, out_color);
    return out_color;
}
