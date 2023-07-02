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
    /// if positive it loops all the time, if negative it only plays once
    duration: f32,
    looping: u32,
}

struct Animation {
    counter: f32,
    current_frame: u32,
}


struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) tex_width: i32,
    @location(2) tex_height: i32,
    @location(3) tex_x: i32,
    @location(4) tex_y: i32,
    @location(5) animated_x: u32,
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
            out.tex_coords = vec2<f32>(current_sprite.tex_x, current_sprite.tex_y);
        }
        case 1u: {
            out.position = vec4<f32>(ratio * pos_x, pos_y - height, depth, 1.0);
            out.tex_coords = vec2<f32>(current_sprite.tex_x, current_sprite.tex_y + current_sprite.height);
        }
        case 2u: {
            out.position = vec4<f32>(ratio * (pos_x + width), pos_y, depth, 1.0);
            out.tex_coords = vec2<f32>(current_sprite.tex_x + current_sprite.width, current_sprite.tex_y);
        }
        case 3u: {
            out.position = vec4<f32>(ratio * pos_x, pos_y - height, depth, 1.0);
            out.tex_coords = vec2<f32>(current_sprite.tex_x, current_sprite.tex_y + current_sprite.height);
        }
        case 4u: {
            out.position = vec4<f32>(ratio * (pos_x + width), pos_y - height, depth, 1.0);
            out.tex_coords = vec2<f32>(current_sprite.tex_x + current_sprite.width, current_sprite.tex_y + current_sprite.height);
        }
        case 5u: {
            out.position = vec4<f32>(ratio * (pos_x + width), pos_y, depth, 1.0);
            out.tex_coords = vec2<f32>(current_sprite.tex_x + current_sprite.width, current_sprite.tex_y);
        }
        default: {
            out.position = vec4<f32>();
            out.tex_coords = vec2<f32>();
        }
    }

    var animated_x = 0u;
    if current_sprite.frames > 1u {
        if anim_storage_array[sprite_index].counter >= current_sprite.duration {
            if anim_storage_array[sprite_index].current_frame < current_sprite.frames - 1u {
                anim_storage_array[sprite_index].current_frame = anim_storage_array[sprite_index].current_frame + 1u;
            } else {
                anim_storage_array[sprite_index].current_frame = 0u;
            }
            anim_storage_array[sprite_index].counter = 0.0;
        } else {
            anim_storage_array[sprite_index].counter += uniform_data.delta_time;
        }
        animated_x = anim_storage_array[sprite_index].current_frame * u32(current_sprite.tex_width);
    }

    out.tex_width = i32(current_sprite.tex_width);
    out.tex_height = i32(current_sprite.tex_height);
    out.tex_x = i32(current_sprite.tex_x);
    out.tex_y = i32(current_sprite.tex_y);
    out.animated_x = animated_x;
    return out;
}

    @fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var result: vec4<f32>;
    // result = textureLoad(texture, vec2<i32>(i32(in.tex_coords.x) + i32(in.animated_x), i32(in.tex_coords.y)), 0);
    result = textureLoad(texture, vec2<i32>(i32(in.tex_coords.x) % in.tex_width + in.tex_x + i32(in.animated_x), i32(in.tex_coords.y) % in.tex_height + in.tex_y), 0);
    
    // result.x = abs(sin(uniform_data.utime));
    return result;
}