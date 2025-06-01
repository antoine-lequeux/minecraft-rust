use bevy::{input::mouse::MouseMotion, prelude::*};

use crate::{
    chunks::{Chunk, Map, Modification},
    types::{BlockType, CHUNK_HEIGHT, CHUNK_SIZE, ChunkPos},
    world::ChunkMap,
};

// The FlyCam component represents the player camera.
#[derive(Component)]
pub struct FlyCam
{
    pub speed: f32,
    pub sprint_mult: f32,
    pub sensitivity: f32,
    pub yaw: f32,
    pub pitch: f32,
}

// This system manages keyboard input and moves the camera accordingly.
pub fn fly_camera_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<(&FlyCam, &mut Transform)>,
)
{
    let dt = time.delta().as_secs_f32();
    for (flycam, mut transform) in query.iter_mut()
    {
        // Create a direction vector for horizontal movement.
        let mut horiz = Vec3::ZERO;

        // Create a direction value for vertical movement.
        let mut vert = 0.0;

        // Forward / back.
        if keyboard.pressed(KeyCode::KeyW)
        {
            horiz.z -= 1.0;
        }
        if keyboard.pressed(KeyCode::KeyS)
        {
            horiz.z += 1.0;
        }

        // Left / right.
        if keyboard.pressed(KeyCode::KeyA)
        {
            horiz.x -= 1.0;
        }
        if keyboard.pressed(KeyCode::KeyD)
        {
            horiz.x += 1.0;
        }

        // Up / down.
        if keyboard.pressed(KeyCode::Space)
        {
            vert += 1.0;
        }
        if keyboard.pressed(KeyCode::ShiftLeft)
        {
            vert -= 1.0;
        }

        // If 'alt' is pressed, the camera moves faster horizontally.
        let speed_mult = if keyboard.pressed(KeyCode::AltLeft) { flycam.sprint_mult } else { 1.0 };

        // Total movement vector.
        let mut delta = Vec3::ZERO;

        // If there is a horizontal movement, 'delta' must be updated.
        if horiz != Vec3::ZERO
        {
            let dir = horiz.normalize();
            let yaw_quat = Quat::from_axis_angle(Vec3::Y, flycam.yaw);
            let world_horiz = yaw_quat * dir;
            delta += world_horiz * flycam.speed * dt * speed_mult;
        }

        // If there is a vertical movement, 'delta' must be updated.
        if vert != 0.0
        {
            delta.y += vert * flycam.speed * dt;
        }

        // 'delta' is applied to the camera translation.
        transform.translation += delta;
    }
}

// This system manages mouse input and pans the camera.
pub fn mouse_look(
    mut motion: EventReader<MouseMotion>,
    mut query: Query<(&mut FlyCam, &mut Transform)>,
)
{
    for ev in motion.read()
    {
        for (mut flycam, mut transform) in query.iter_mut()
        {
            // Get the yaw (Y‐axis) from X movement, and the pitch (X‐axis) from Y movement.
            flycam.yaw -= ev.delta.x * flycam.sensitivity;
            flycam.pitch -= ev.delta.y * flycam.sensitivity;
            // Clamp the pitch to [-90°, +90°].
            flycam.pitch = flycam
                .pitch
                .clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians());

            // Update the actual transform.rotation.
            transform.rotation = Quat::from_euler(EulerRot::YXZ, flycam.yaw, flycam.pitch, 0.0);
        }
    }
}

// This system handles block interaction with the mouse.
// If a block is placed or destroyed, the chunk is marked as changed and will be
// remeshed.
pub fn block_interaction(
    buttons: Res<ButtonInput<MouseButton>>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    chunk_map: Res<ChunkMap>,
    mut map: ResMut<Map>,
    mut chunks: Query<&mut Chunk>,
)
{
    let request_destroy: bool = buttons.just_pressed(MouseButton::Left);
    let request_place: bool = buttons.just_pressed(MouseButton::Right);

    // If neither left nor right mouse button is pressed, we do nothing.
    if !(request_destroy || request_place)
    {
        return;
    }

    let Ok(cam_tf) = cams.single()
    else
    {
        println!("Camera transform is not valid.");
        return;
    };

    // Get the camera's origin and direction.
    let origin = cam_tf.translation();
    let dir = cam_tf.forward();

    // March a ray out to max_d in steps.
    let max_d = 8.0;
    let step = 0.01; // The step is small to prevent missing blocks when clicking on a corner.
    let mut t = 0.0;
    let mut last_air_pos: Option<(i32, i32, i32)> = None;
    let mut last_air_chunk = None;

    // Iterate over the ray from the camera origin in the direction of the camera.
    while t < max_d
    {
        // Calculate the current world position of the ray's end.
        let p = origin + dir * t;
        let bx = p.x.floor() as i32;
        let by = p.y.floor() as i32;
        let bz = p.z.floor() as i32;

        // Get the chunk in which the ray's end is.
        let cx = bx.div_euclid(CHUNK_SIZE as i32);
        let cz = bz.div_euclid(CHUNK_SIZE as i32);
        let cpos = ChunkPos { x: cx, y: cz };

        // Check if the chunk is loaded.
        if let Some(&entity) = chunk_map.loaded_chunks.get(&cpos)
        {
            if let Ok(mut chunk) = chunks.get_mut(entity)
            {
                // Get the local coordinates of the ray's end (in the chunk).
                let lx = bx - cx * CHUNK_SIZE as i32;
                let lz = bz - cz * CHUNK_SIZE as i32;
                if (0 .. CHUNK_SIZE as i32).contains(&lx)
                    && (0 .. CHUNK_HEIGHT as i32).contains(&by)
                    && (0 .. CHUNK_SIZE as i32).contains(&lz)
                {
                    // Calculate the index of the block in the chunk's blocks array.
                    let idx = (by as usize) * (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)
                        + (lz as usize) * (CHUNK_SIZE as usize)
                        + (lx as usize);

                    // Check if the block at this index is air.
                    let current_block = chunk.blocks[idx];
                    if current_block != BlockType::Air
                    {
                        if request_destroy
                        {
                            // Destroy the block.
                            chunk.blocks[idx] = BlockType::Air;
                            map.modified
                                .entry(cpos)
                                .or_default()
                                .push(Modification { index: idx, new: BlockType::Air });
                            return;
                        }
                        else if request_place && last_air_pos.is_some()
                        {
                            // Place a block at the last air position we found.
                            if let (Some((last_x, last_y, last_z)), Some(last_chunk_entity)) =
                                (last_air_pos, last_air_chunk)
                            {
                                // Get the chunk where we want to place the block.
                                if let Ok(mut target_chunk) = chunks.get_mut(last_chunk_entity)
                                {
                                    // Calculate the local coordinates in the target chunk.
                                    let last_cx = last_x.div_euclid(CHUNK_SIZE as i32);
                                    let last_cz = last_z.div_euclid(CHUNK_SIZE as i32);
                                    let last_lx = last_x - last_cx * CHUNK_SIZE as i32;
                                    let last_lz = last_z - last_cz * CHUNK_SIZE as i32;

                                    // Calculate the index in the target chunk's blocks array.
                                    let last_idx = (last_y as usize)
                                        * (CHUNK_SIZE as usize)
                                        * (CHUNK_SIZE as usize)
                                        + (last_lz as usize) * (CHUNK_SIZE as usize)
                                        + (last_lx as usize);

                                    // Place the block at the last air position.
                                    target_chunk.blocks[last_idx] = BlockType::Water;

                                    map.modified.entry(cpos).or_default().push(Modification {
                                        index: last_idx,
                                        new: BlockType::Water,
                                    });
                                }
                            }
                            return;
                        }
                        return;
                    }
                    else
                    {
                        // Keep track of the last air position for block placement.
                        last_air_pos = Some((bx, by, bz));
                        last_air_chunk = Some(entity);
                    }
                }
            }
        }
        t += step;
    }
}
