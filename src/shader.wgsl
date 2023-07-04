struct Uniform {
    height_resolution: f32,
    texture_width: f32,
    texture_height: f32,
    window_width: f32,
    window_height: f32,
    utime: f32,
    last_utime: f32,
    delta_time: f32,
    global_offset_x: f32,
    global_offset_y: f32,
}

struct Sprite {
    pos_x: f32,
    pos_y: f32,
    width: f32,
    height: f32,

    tex_x: f32,
    tex_y: f32,
    tex_width: f32,
    tex_height: f32,

    depth: f32,
    origin: f32,

    frames: u32,
    duration: f32,
    paused: u32,
    reversed: u32,
    looping: u32,

    transparency: f32,

    anim_buffer_index: u32,

    flipped_x: u32,
    flipped_y: u32,
}

struct Animation {
    counter: f32,
    current_frame: u32,
    loop_paused: u32,
    started: u32,
}


struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) tex_width: i32,
    @location(2) tex_height: i32,
    @location(3) tex_x: i32,
    @location(4) tex_y: i32,
    @location(5) frame_offset_x: u32,
    @location(6) transparency: f32,

    @location(7) flipped_x: u32,
    @location(8) flipped_y: u32,
    // @location(9) thing: Sprite,

}


@group(0) @binding(0) var<uniform> uniform_data: Uniform;
@group(0) @binding(1) var<storage, read_write> storage_array: array<Sprite>;
@group(0) @binding(2) var<storage, read_write> anim_storage_array: array<Animation>;
@group(0) @binding(3) var texture: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let sprite_index = vertex_index / 6u;
    let vertex_in_sprite_index = vertex_index % 6u;

    let current_sprite = storage_array[sprite_index];

    let pos_x = (current_sprite.pos_x + uniform_data.global_offset_x) / uniform_data.height_resolution;
    let pos_y = (current_sprite.pos_y + uniform_data.global_offset_y) / uniform_data.height_resolution;

    let width = current_sprite.width / uniform_data.height_resolution;
    let height = current_sprite.height / uniform_data.height_resolution;

    let ratio = uniform_data.window_height / uniform_data.window_width;

    var depth = current_sprite.depth;
    if current_sprite.depth < 0.5 && current_sprite.depth > 0.0 {
        depth = current_sprite.depth * 0.6;
    } else if current_sprite.depth < 1.0 && current_sprite.depth > 0.5 {
        depth = (current_sprite.depth - 1.0) * 0.6 + 1.0;
    } else if current_sprite.depth == 0.5 {
        let normalized_origin_y = clamp((uniform_data.global_offset_y + current_sprite.pos_y - current_sprite.origin) / uniform_data.height_resolution, -2.0, 2.0);
        // the range is actually 0.4 to 0.6, both inclusive, it is extended to 2.0 instead clamping at the edge where abs(y) == 1.0, but 
        // sometimes sprites goes partially out of the screen, but you still wanna ysort them
        depth = 0.1 * normalized_origin_y + 0.5;
    } else {
        depth = 0.0;
    }

    var out: VertexOutput;
    switch vertex_in_sprite_index {
        case 0u: {
            out.position = vec4<f32>(ratio * pos_x, pos_y, depth, 1.0);
            out.tex_coords = vec2<f32>(0.0, 0.0);
        }
        case 1u: {
            out.position = vec4<f32>(ratio * pos_x, pos_y - height, depth, 1.0);
            out.tex_coords = vec2<f32>(0.0, current_sprite.height);
        }
        case 2u: {
            out.position = vec4<f32>(ratio * (pos_x + width), pos_y, depth, 1.0);
            out.tex_coords = vec2<f32>(current_sprite.width, 0.0);
        }
        case 3u: {
            out.position = vec4<f32>(ratio * pos_x, pos_y - height, depth, 1.0);
            out.tex_coords = vec2<f32>(0.0, current_sprite.height);
        }
        case 4u: {
            out.position = vec4<f32>(ratio * (pos_x + width), pos_y - height, depth, 1.0);
            out.tex_coords = vec2<f32>(current_sprite.width, current_sprite.height);
        }
        case 5u: {
            out.position = vec4<f32>(ratio * (pos_x + width), pos_y, depth, 1.0);
            out.tex_coords = vec2<f32>(current_sprite.width, 0.0);
        }
        default: {
            out.position = vec4<f32>();
            out.tex_coords = vec2<f32>();
        }
    }

    var frame_offset_x = 0u;
    if current_sprite.reversed != 0u && anim_storage_array[current_sprite.anim_buffer_index].started == 0u {
        anim_storage_array[current_sprite.anim_buffer_index].current_frame = (current_sprite.frames - 1u);
    }
    if current_sprite.frames > 1u && current_sprite.paused == 0u && (current_sprite.looping != 0u || anim_storage_array[current_sprite.anim_buffer_index].loop_paused == 0u) {
        anim_storage_array[current_sprite.anim_buffer_index].started = 1u;
        let duration = max(current_sprite.duration, 0.0);
        if anim_storage_array[current_sprite.anim_buffer_index].counter >= duration {
            if current_sprite.reversed == 0u {
                if anim_storage_array[current_sprite.anim_buffer_index].current_frame + 1u < current_sprite.frames {
                    if anim_storage_array[current_sprite.anim_buffer_index].current_frame + 1u == current_sprite.frames - 1u {
                        anim_storage_array[current_sprite.anim_buffer_index].loop_paused = 1u;
                    }
                    anim_storage_array[current_sprite.anim_buffer_index].current_frame += 1u;
                } else {
                    anim_storage_array[current_sprite.anim_buffer_index].current_frame = 0u;
                    anim_storage_array[current_sprite.anim_buffer_index].loop_paused = 0u;
                }
            } else {
                if anim_storage_array[current_sprite.anim_buffer_index].current_frame > 0u {
                    if anim_storage_array[current_sprite.anim_buffer_index].current_frame == 1u {
                        anim_storage_array[current_sprite.anim_buffer_index].loop_paused = 1u;
                    }
                    anim_storage_array[current_sprite.anim_buffer_index].current_frame -= 1u;
                } else {
                    anim_storage_array[current_sprite.anim_buffer_index].current_frame = current_sprite.frames - 1u;
                    anim_storage_array[current_sprite.anim_buffer_index].loop_paused = 0u;
                }
            }
            anim_storage_array[current_sprite.anim_buffer_index].counter = 0.0;
        } else {
            anim_storage_array[current_sprite.anim_buffer_index].counter += uniform_data.delta_time;
        }
    }
    frame_offset_x = anim_storage_array[current_sprite.anim_buffer_index].current_frame * u32(current_sprite.tex_width);

    out.tex_width = i32(current_sprite.tex_width);
    out.tex_height = i32(current_sprite.tex_height);
    out.tex_x = i32(current_sprite.tex_x);
    out.tex_y = i32(current_sprite.tex_y);
    out.frame_offset_x = frame_offset_x;
    out.transparency = clamp(current_sprite.transparency, 0.0, 1.0);
    out.flipped_x = current_sprite.flipped_x;
    out.flipped_y = current_sprite.flipped_y;

    return out;
}

    @fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var result: vec4<f32>;
    // result = textureLoad(texture, vec2<i32>(i32(in.tex_coords.x) + i32(in.frame_offset_x), i32(in.tex_coords.y)), 0);

    var repeated_unit_x = i32(in.tex_coords.x) % in.tex_width;
    var repeated_unit_y = i32(in.tex_coords.y) % in.tex_height;
    var base_x = in.tex_x;
    var base_y = in.tex_y;
    var frame_x = i32(in.frame_offset_x);
    var frame_y = 0;

    if in.flipped_x != 0u {
        repeated_unit_x = in.tex_width - 1 - repeated_unit_x;
    }

    if in.flipped_y != 0u {
        repeated_unit_y = in.tex_height - 1 - repeated_unit_y;
    }

    result = textureLoad(texture, vec2<i32>(base_x + frame_x + repeated_unit_x, base_y + frame_y + repeated_unit_y), 0);

    // set transparency
    result.w *= in.transparency;
    // result.x = abs(sin(uniform_data.utime));
    return result;
}