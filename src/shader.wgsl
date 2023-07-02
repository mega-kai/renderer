struct Uniform {
    height_resolution: f32,
    texture_width: f32,
    texture_height: f32,
    window_width: f32,
    window_height: f32,
    utime: f32,
    mouse_x: f32,
    mouse_y: f32,
    cam_offset_x: f32,
    cam_offset_y: f32,
}

struct Sprite {
    pos_x: f32,
    pos_y: f32,
    width: f32,
    height: f32,

    tex_x: f32,
    tex_y: f32,

    depth: f32,
    origin: f32,
}

struct Animation {
    counter: f32,
    current_frame: u32,
}


struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}


@group(0) @binding(0) var<uniform> uniform_data: Uniform;
@group(0) @binding(1) var<storage, read_write> storage_array: array<Sprite>;
@group(0) @binding(2) var<storage, read_write> anim_storage_array: array<Animation>;
@group(0) @binding(3) var texture: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // todo depth sorting
    let sprite_index = vertex_index / 6u;
    let vertex_in_sprite_index = vertex_index % 6u;

    let current_sprite = storage_array[sprite_index];

    let pos_x = current_sprite.pos_x / uniform_data.height_resolution;
    let pos_y = current_sprite.pos_y / uniform_data.height_resolution;

    let width = current_sprite.width / uniform_data.height_resolution;
    let height = current_sprite.height / uniform_data.height_resolution;

    let ratio = uniform_data.window_height / uniform_data.window_width;

    var depth = current_sprite.depth;
    if current_sprite.depth < 0.5 && current_sprite.depth > 0.0 {
        depth = current_sprite.depth * 0.6;
    } else if current_sprite.depth < 1.0 && current_sprite.depth > 0.5 {
        // todo, shrink it so that it fits in background
        depth = (current_sprite.depth - 1.0) * 0.6 + 1.0;
    } else if current_sprite.depth == 0.5 {
        let normalized_origin_y = clamp((current_sprite.pos_y - current_sprite.origin) / uniform_data.height_resolution, -1.5, 1.5);
        depth = 0.2 * normalized_origin_y + 0.5;
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
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // todo might need some tuning to get this to sample lower to appear pixel perfect 
    var result = textureLoad(texture, vec2<i32>(in.tex_coords), 0);
    // result.x = abs(sin(uniform_data.utime));
    return result;
}