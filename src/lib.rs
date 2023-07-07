#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_mut,
    unused_assignments,
    unreachable_code,
    unused_labels
)]
#![feature(path_file_prefix, alloc_layout_extra)]

use std::{
    mem::{size_of, MaybeUninit},
    slice::from_raw_parts,
};

pub use ecs;
use rayon::{prelude::IntoParallelRefMutIterator, slice::ParallelSliceMut};
pub use winit;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Animation {
    counter: f32,
    pub current_frame: u32,
    pub loop_paused: u32,
    started: u32,
    pub cycles: u32,
    pub cycle_just_finished: u32,

    pub actual_depth: f32,
}
impl Animation {
    fn new_empty() -> Self {
        Self {
            counter: 0.0,
            current_frame: 0,
            loop_paused: 0,
            started: 0,
            cycles: 0,
            cycle_just_finished: 0,
            actual_depth: 0.0,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy)]
struct TextureDescription {
    tex_x: u32,
    tex_y: u32,
    tex_width: u32,
    tex_height: u32,
    frames: u32,
    origin: u32,
    looping: u32,
    frames_per_sec: u32,
}

/// specify the depth as 0.5 to enable y sorting
#[repr(C)]
#[derive(Debug)]
pub struct Sprite {
    pub pos_x: f32,
    pub pos_y: f32,

    /// if you don't want repeating patterns don't touch this
    pub width: f32,
    pub height: f32,

    tex_x: f32,
    tex_y: f32,

    tex_width: f32,
    tex_height: f32,

    pub base_depth: f32,
    origin: f32,

    frames: u32,

    pub duration: f32,
    pub paused: u32,
    pub reversed: u32,
    pub looping: u32,

    pub transparency: f32,

    anim_buffer_index: u32,

    pub flipped_x: u32,
    pub flipped_y: u32,
}
impl Sprite {
    fn new_empty() -> Self {
        Self {
            pos_x: 0.0,
            pos_y: 0.0,
            width: 0.0,
            height: 0.0,

            tex_x: 0.0,
            tex_y: 0.0,
            tex_height: 0.0,
            tex_width: 0.0,

            base_depth: 0.0,
            origin: 0.0,

            duration: 1.0,
            reversed: 0,
            paused: 0,
            frames: 0,
            looping: 0,

            transparency: 1.0,

            anim_buffer_index: 0,

            flipped_x: 0,
            flipped_y: 0,
        }
    }
}

// todo add texture functionality
pub struct SpriteMaster3000<'this> {
    map: std::collections::HashMap<String, TextureDescription>,
    occupied_indices: Vec<bool>,
    names: Vec<&'static str>,
    anim_data: Vec<Animation>,

    table: &'this mut ecs::Table,
    queue: &'this wgpu::Queue,
    buffer: &'this wgpu::Buffer,
}
impl<'this> SpriteMaster3000<'this> {
    fn new(
        mut current_dir: std::path::PathBuf,
        sprite_num: u32,
        table: *mut ecs::Table,
        queue: *const wgpu::Queue,
        buffer: *const wgpu::Buffer,
        anim_data: Vec<Animation>,
    ) -> Self {
        current_dir.push("res/texture.json");
        let val = std::fs::read(current_dir).expect("cannot open file from this directory, either file not exist or don't have the permission");
        let map: std::collections::HashMap<String, TextureDescription> =
            serde_json::from_slice(&val).expect("corrupt file format");
        Self {
            map,
            occupied_indices: vec![false; sprite_num as usize],
            table: unsafe { table.as_mut().unwrap() },
            queue: unsafe { queue.as_ref().unwrap() },
            buffer: unsafe { buffer.as_ref().unwrap() },
            anim_data,
            names: vec![""; sprite_num as usize],
            // collisions: vec![0; (sprite_num as f32 / 32.0).ceil() as usize],
        }
    }

    fn read_sprite<'a, 'b>(&self, sparse_index: usize) -> Result<&'b Sprite, &'static str> {
        Ok(self
            .table
            .read_single::<Sprite>(sparse_index)
            .ok_or("invalid index")?)
    }

    pub fn read_anim_data(&self, sparse_index: usize) -> Result<Animation, &'static str> {
        let sprite = self.read_sprite(sparse_index)?;
        Ok(self.anim_data[sprite.anim_buffer_index as usize])
    }

    pub fn get_name(&self, sparse_index: usize) -> Result<&'static str, &'static str> {
        let sprite = self.read_sprite(sparse_index)?;
        Ok(self.names[sprite.anim_buffer_index as usize])
    }

    fn request_index(&mut self) -> u32 {
        let mut buffer_index = None::<u32>;
        for each in 0..self.occupied_indices.len() {
            if self.occupied_indices[each] == false {
                self.occupied_indices[each] = true;
                buffer_index = Some(each as u32);
                break;
            }
        }
        buffer_index.expect("not enough space")
    }

    pub fn set_anim_data(
        &mut self,
        texture: &'static str,
        sprite: &mut Sprite,
    ) -> Result<(), &'static str> {
        let tex_data = self.map.get(texture).ok_or("wrong texture name")?;
        sprite.tex_x = tex_data.tex_x as f32;
        sprite.tex_y = tex_data.tex_y as f32;
        sprite.tex_width = tex_data.tex_width as f32;
        sprite.tex_height = tex_data.tex_height as f32;
        sprite.width = tex_data.tex_width as f32;
        sprite.height = tex_data.tex_height as f32;
        sprite.frames = tex_data.frames;
        sprite.looping = tex_data.looping;
        sprite.duration = 1.0 / tex_data.frames_per_sec.max(1) as f32;
        Ok(())
    }

    pub fn add_sprite<'a, 'b>(
        &'a mut self,
        texture: &'static str,
        pos: (f32, f32),
        depth: f32,
    ) -> Result<(usize, &'b mut Sprite), &'static str> {
        let mut buffer_index = self.request_index();
        let mut sprite = Sprite::new_empty();

        sprite.anim_buffer_index = buffer_index;
        self.names[buffer_index as usize] = texture;

        self.set_anim_data(texture, &mut sprite)?;

        sprite.base_depth = depth;
        sprite.pos_x = pos.0;
        sprite.pos_y = pos.1;

        Ok(self.table.insert_new(sprite))
    }

    pub fn insert_sprite<'a, 'b>(
        &'a mut self,
        sparse_index: usize,
        texture: &'static str,
        pos: (f32, f32),
        depth: f32,
    ) -> Result<&'b mut Sprite, &'static str> {
        let mut buffer_index = self.request_index();

        let mut sprite = Sprite::new_empty();
        sprite.anim_buffer_index = buffer_index;
        self.names[buffer_index as usize] = texture;

        self.set_anim_data(texture, &mut sprite)?;

        sprite.base_depth = depth;
        sprite.pos_x = pos.0;
        sprite.pos_y = pos.1;

        Ok(self.table.insert_at(sparse_index, sprite)?)
    }

    fn copy_sprite(
        &mut self,
        index_to_clone: usize,
        requested_index: u32,
    ) -> Result<Sprite, &'static str> {
        unsafe {
            let sprite_clone = self
                .table
                .read_single::<Sprite>(index_to_clone)
                .ok_or("index not valid")?;
            let mut uninit_sprite: MaybeUninit<Sprite> = MaybeUninit::uninit();
            std::ptr::copy(sprite_clone, uninit_sprite.as_mut_ptr(), 1);
            let mut sprite = uninit_sprite.assume_init();
            sprite.anim_buffer_index = requested_index;
            self.names[requested_index as usize] =
                self.names[sprite_clone.anim_buffer_index as usize];
            Ok(sprite)
        }
    }

    pub fn clone_insert<'a, 'b>(
        &'a mut self,
        index_to_clone: usize,
        index_to_insert: usize,
    ) -> Result<&'b mut Sprite, &'static str> {
        let mut buffer_index = self.request_index();
        let mut sprite = self.copy_sprite(index_to_clone, buffer_index)?;
        Ok(self.table.insert_at(index_to_insert, sprite)?)
    }

    pub fn clone_add<'a, 'b>(
        &'a mut self,
        index_to_clone: usize,
    ) -> Result<(usize, &'b mut Sprite), &'static str> {
        let mut buffer_index = self.request_index();
        let mut sprite = self.copy_sprite(index_to_clone, buffer_index)?;
        Ok(self.table.insert_new(sprite))
    }

    fn free_index(&mut self, anim_buffer_index: u32) {
        let index_size = anim_buffer_index as u64 * std::mem::size_of::<Animation>() as u64;
        // the problem is probably here, which is why it didn't
        self.queue.write_buffer(
            self.buffer,
            index_size,
            bytemuck::cast_slice(&[Animation::new_empty()]),
        );

        self.occupied_indices[anim_buffer_index as usize] = false;
        self.names[anim_buffer_index as usize] = "";
    }

    pub fn remove_sprite(&mut self, sparse_index: usize) -> Result<(), &'static str> {
        let sprite = self.table.remove::<Sprite>(sparse_index)?;
        self.free_index(sprite.anim_buffer_index);
        Ok(())
    }

    pub fn change_state(
        &mut self,
        sparse_index: usize,
        texture: &'static str,
    ) -> Result<(), &'static str> {
        let tex_data = *self.map.get(texture).ok_or("wrong texture name")?;
        let sprite = self
            .table
            .read_single::<Sprite>(sparse_index)
            .ok_or("invalid index")?;

        if tex_data.tex_x as f32 == sprite.tex_x
            && tex_data.tex_y as f32 == sprite.tex_y
            && tex_data.frames == sprite.frames
            && tex_data.tex_height as f32 == sprite.tex_height
            && tex_data.tex_width as f32 == sprite.tex_width
        {
            return Ok(());
        }

        let mut buffer_index = self.request_index();

        self.set_anim_data(texture, sprite)?;

        self.free_index(sprite.anim_buffer_index);

        self.names[sprite.anim_buffer_index as usize] = "";
        self.names[buffer_index as usize] = texture;

        sprite.anim_buffer_index = buffer_index;
        Ok(())
    }
}

// todo add more collision shapes
#[derive(Debug)]
pub struct CollisionRect {
    sparse_index: usize,

    pub pos_x: f32,
    pub pos_y: f32,

    pub width: f32,
    pub height: f32,
    pub actual_depth: f32,
    pub fuzzy_range: f32,
    pub channel: u32,
}
impl CollisionRect {
    fn new_empty() -> Self {
        Self {
            sparse_index: !0,
            pos_x: 0.0,
            pos_y: 0.0,
            width: 0.0,
            height: 0.0,
            actual_depth: 0.0,
            fuzzy_range: 0.0,
            channel: 0,
        }
    }

    pub fn sync_size_and_pos(&mut self, sprite: &Sprite) {
        self.pos_x = sprite.pos_x;
        self.pos_y = sprite.pos_y;
        self.width = sprite.width;
        self.height = sprite.height;
    }
}

pub struct CollisionManager<'this> {
    table: &'this mut ecs::Table,

    pub x_sorted_list: Vec<CollisionRect>,

    pub x_collided: Vec<(CollisionRect, CollisionRect)>,

    // sorted with id
    pub colliding_list: Vec<(usize, usize)>,
}
impl<'this> CollisionManager<'this> {
    fn new(table: *mut ecs::Table) -> Self {
        Self {
            table: unsafe { table.as_mut().unwrap() },
            x_sorted_list: vec![],
            x_collided: vec![],
            colliding_list: vec![],
        }
    }

    pub fn check_if_colliding(&self, id_1: usize, id_2: usize) -> bool {
        if self.colliding_list.binary_search(&(id_1, id_2)).is_ok()
            || self.colliding_list.binary_search(&(id_2, id_1)).is_ok()
        {
            true
        } else {
            false
        }
    }

    pub fn add_collision_rect(
        &mut self,
        pos: (f32, f32),
        size: (f32, f32),
        depth: f32,
        fuzzy_range: f32,
        channel: u32,
    ) -> usize {
        let collision_rect = CollisionRect {
            sparse_index: 0,
            pos_x: pos.0,
            pos_y: pos.1,
            width: size.0,
            height: size.1,
            actual_depth: depth,
            fuzzy_range,
            channel,
        };
        let (index, shape) = self.table.insert_new(collision_rect);
        shape.sparse_index = index;
        index
    }

    pub fn insert_collision_rect(
        &mut self,
        sparse_index: usize,
        pos: (f32, f32),
        size: (f32, f32),
        depth: f32,
        fuzzy_range: f32,
        channel: u32,
    ) -> Result<(), &'static str> {
        let collision_rect = CollisionRect {
            sparse_index,
            pos_x: pos.0,
            pos_y: pos.1,
            width: size.0,
            height: size.1,
            actual_depth: depth,
            fuzzy_range,
            channel,
        };
        self.table.insert_at(sparse_index, collision_rect)?;
        Ok(())
    }

    /// basically if you update the position data of some rect, the collision result will be available at the next frame
    fn update(&mut self, list_of_rects: &[CollisionRect]) {
        unsafe {
            self.x_collided.clear();
            self.colliding_list.clear();
            if self.x_sorted_list.capacity() < list_of_rects.len() {
                self.x_sorted_list.reserve(list_of_rects.len());
                self.x_sorted_list.set_len(list_of_rects.len());
            }

            std::ptr::copy(
                list_of_rects.as_ptr(),
                self.x_sorted_list.as_mut_ptr(),
                list_of_rects.len(),
            );
            self.x_sorted_list[0..list_of_rects.len()]
                .par_sort_unstable_by(|x, y| x.pos_x.total_cmp(&y.pos_x));

            for current_index in 0..list_of_rects.len() {
                for check_against in 1..list_of_rects.len() - current_index {
                    let check_against_index = check_against + current_index;
                    let mut current_rect_uninit: MaybeUninit<CollisionRect> = MaybeUninit::uninit();
                    let mut check_against_rect_uninit: MaybeUninit<CollisionRect> =
                        MaybeUninit::uninit();
                    std::ptr::copy(
                        &self.x_sorted_list[current_index],
                        current_rect_uninit.as_mut_ptr(),
                        1,
                    );
                    std::ptr::copy(
                        &self.x_sorted_list[check_against_index],
                        check_against_rect_uninit.as_mut_ptr(),
                        1,
                    );
                    let current_rect = current_rect_uninit.assume_init();
                    let check_against_rect = check_against_rect_uninit.assume_init();
                    if current_rect.pos_x + current_rect.width >= check_against_rect.pos_x {
                        self.x_collided.push((current_rect, check_against_rect));
                    } else {
                        // breaking the inner loop
                        break;
                    }
                }
            }
            for (first_rect, second_rect) in &self.x_collided {
                let y_check = (first_rect.pos_y - first_rect.height <= second_rect.pos_y
                    && first_rect.pos_y >= second_rect.pos_y)
                    || (second_rect.pos_y - second_rect.height <= first_rect.pos_y
                        && second_rect.pos_y >= first_rect.pos_y);
                let channel_check = first_rect.channel == second_rect.channel;
                let depth_check = (first_rect.actual_depth <= second_rect.actual_depth
                    && first_rect.actual_depth + first_rect.fuzzy_range
                        >= second_rect.actual_depth)
                    || (second_rect.actual_depth <= first_rect.actual_depth
                        && second_rect.actual_depth + second_rect.fuzzy_range
                            >= first_rect.actual_depth);
                if y_check && channel_check && depth_check {
                    self.colliding_list
                        .push((first_rect.sparse_index, second_rect.sparse_index));
                }
            }

            // so that binary search would actually work
            self.colliding_list.par_sort_unstable();
        }
    }
}

#[repr(C)]
#[derive(Debug, bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
pub struct Uniform {
    /// write
    pub height_resolution: f32,
    texture_width: f32,
    texture_height: f32,
    /// read
    pub window_width: f32,
    /// read
    pub window_height: f32,
    /// read
    pub utime: f32,
    /// read
    pub last_utime: f32,
    /// read
    pub delta_time: f32,
    /// write
    pub global_offset_x: f32,
    /// write
    pub global_offset_y: f32,
}

#[derive(Clone, Copy)]
pub enum RunningState {
    Running,
    Closed,
}

#[derive(Clone, Copy, Debug)]
/// read on state
pub struct MouseState {
    x: f32,
    y: f32,

    left: bool,
    left_clicked: bool,
    left_released: bool,

    middle: bool,
    middle_clicked: bool,
    middle_released: bool,

    right: bool,
    right_clicked: bool,
    right_released: bool,

    wheel_delta: f32,

    in_screen: bool,
    just_left: bool,
    just_entered: bool,
}
impl MouseState {
    /// reset after each tick
    fn reset(&mut self) {
        self.wheel_delta = 0.0;

        self.left_clicked = false;
        self.middle_clicked = false;
        self.right_clicked = false;

        self.left_released = false;
        self.middle_released = false;
        self.right_released = false;

        self.just_entered = false;
        self.just_left = false;
    }

    pub fn x(&self) -> f32 {
        self.x
    }
    pub fn y(&self) -> f32 {
        self.y
    }

    pub fn left_button_clicked(&self) -> bool {
        self.left_clicked
    }
    pub fn left_button_pressed(&self) -> bool {
        self.left
    }
    pub fn left_button_released(&self) -> bool {
        self.left_released
    }

    pub fn right_button_clicked(&self) -> bool {
        self.right_clicked
    }
    pub fn right_button_pressed(&self) -> bool {
        self.right
    }
    pub fn right_button_released(&self) -> bool {
        self.right_released
    }

    pub fn middle_button_clicked(&self) -> bool {
        self.middle_clicked
    }
    pub fn middle_button_pressed(&self) -> bool {
        self.middle
    }
    pub fn middle_button_released(&self) -> bool {
        self.middle_released
    }

    // wheel stuff
    pub fn wheel_delta(&self) -> f32 {
        self.wheel_delta
    }

    // cursor stuff
    pub fn in_screen(&self) -> bool {
        self.in_screen
    }
    pub fn just_left(&self) -> bool {
        self.just_left
    }
    pub fn just_entered(&self) -> bool {
        self.just_entered
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum KeyEvent {
    Pressed(winit::event::VirtualKeyCode),
    Released(winit::event::VirtualKeyCode),
}

pub struct KeyState {
    pressed: [Option<winit::event::VirtualKeyCode>; 32],
    events: [Option<KeyEvent>; 32],
}
impl KeyState {
    fn new() -> Self {
        KeyState {
            pressed: [None; 32],
            events: [None; 32],
        }
    }

    /// reset after each tick
    fn reset(&mut self) {
        self.pressed.sort_unstable_by(|x, y| y.cmp(x));
        self.events = [None; 32];
    }

    fn press(&mut self, code: winit::event::VirtualKeyCode) {
        if !self.pressed.contains(&Some(code)) {
            for each in self.pressed.iter_mut() {
                if each == &None {
                    *each = Some(code);
                    // register event here
                    for each in 0..self.events.len() {
                        if self.events[each] == None {
                            self.events[each] = Some(KeyEvent::Pressed(code));
                        }
                        break;
                    }
                    //
                    break;
                }
            }
        }
    }

    fn release(&mut self, code: winit::event::VirtualKeyCode) {
        let mut index_to_remove = None::<usize>;
        for (index, each) in self.pressed.iter().enumerate() {
            if each == &Some(code) {
                index_to_remove = Some(index);
                // register event here
                for each in 0..self.events.len() {
                    if self.events[each] == None {
                        self.events[each] = Some(KeyEvent::Released(code));
                        break;
                    }
                }
                //
                break;
            }
        }
        if index_to_remove.is_some() {
            self.pressed[index_to_remove.unwrap()] = None;
        }
    }

    pub fn is_pressed(&self, code: winit::event::VirtualKeyCode) -> bool {
        self.pressed.contains(&Some(code))
    }

    pub fn just_clicked(&self, key: winit::event::VirtualKeyCode) -> bool {
        for each in 0..self.events.len() {
            if self.events[each] == Some(KeyEvent::Pressed(key)) {
                return true;
            }
        }
        false
    }

    pub fn just_released(&self, key: winit::event::VirtualKeyCode) -> bool {
        for each in 0..self.events.len() {
            if self.events[each] == Some(KeyEvent::Released(key)) {
                return true;
            }
        }
        false
    }
}

/// max sprites must exceed 32
pub fn run(
    minimal_half_height_resolution: f32,
    max_sprites: u32,
    entry_point: fn(&mut ecs::Table),
    prep_func: fn(&mut ecs::Table),
    post_func: fn(&mut ecs::Table),
) {
    assert!(max_sprites >= 32);
    // utime
    let start_time = std::time::Instant::now();
    // window and event loop stuff
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();

    // wgpu prep stuff
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    let surface = unsafe { instance.create_surface(&window).unwrap() };
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptionsBase {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            features: adapter.features(),
            limits: adapter.limits(),
        },
        None,
    ))
    .unwrap();
    let mut surface_config = wgpu::SurfaceConfiguration {
        width: window.inner_size().width,
        height: window.inner_size().height,
        format: surface.get_capabilities(&adapter).formats[0],
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        alpha_mode: surface.get_capabilities(&adapter).alpha_modes[0],
        present_mode: wgpu::PresentMode::Fifo,
        view_formats: vec![],
    };
    surface.configure(&device, &surface_config);

    // loading texture and it's meta data
    let current_dir = std::env::current_dir().unwrap();
    let mut texture_dir = current_dir.clone();
    texture_dir.push("res/texture.png");
    let texture_data = image::io::Reader::open(texture_dir)
        .unwrap()
        .decode()
        .unwrap()
        .into_rgba8();
    // todo, rearrange the image data into a long thin one if needed, according a loaded metadata file
    // also maybe we can have in game editor that saves and expand on existing texture atlas
    let pages_to_assign = 1;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: texture_data.width(),
            height: texture_data.height(),
            depth_or_array_layers: pages_to_assign,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTextureBase {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &texture_data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(texture_data.width() * 4),
            rows_per_image: Some(texture_data.height() * 4),
        },
        wgpu::Extent3d {
            width: texture_data.width(),
            height: texture_data.height(),
            depth_or_array_layers: 1,
        },
    );

    // sound system todo

    // font handler todo

    // uniform data
    let mut uniform_data = Uniform {
        height_resolution: minimal_half_height_resolution,
        texture_width: texture_data.width() as f32,
        texture_height: texture_data.height() as f32,
        window_width: 0.0,
        window_height: 0.0,
        utime: 0.0,
        last_utime: 0.0,
        delta_time: 0.0,
        global_offset_x: 0.0,
        global_offset_y: 0.0,
    };
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        mapped_at_creation: false,
        size: std::mem::size_of::<Uniform>() as u64,
        usage: wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::UNIFORM,
    });
    queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&[uniform_data]));

    // storage buffer
    let mut sprite_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        mapped_at_creation: false,
        size: std::mem::size_of::<Sprite>() as u64 * max_sprites as u64,
        usage: wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::STORAGE,
    });
    let mut animation_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        mapped_at_creation: false,
        size: std::mem::size_of::<Animation>() as u64 * max_sprites as u64,
        usage: wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::STORAGE,
    });

    // swap buffer
    let mut swap_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        mapped_at_creation: false,
        size: std::mem::size_of::<Animation>() as u64 * max_sprites as u64,
        usage: wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::MAP_READ,
    });

    // depth texture for transparency sorting
    let mut depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: window.inner_size().width,
            height: window.inner_size().height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });

    // bind_group
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    });
    let mut bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &sprite_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &animation_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&texture.create_view(
                    &wgpu::TextureViewDescriptor {
                        label: None,
                        format: None,
                        dimension: None,
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: 0,
                        mip_level_count: None,
                        base_array_layer: 0,
                        array_layer_count: None,
                    },
                )),
            },
        ],
    });

    // shader
    let shader = device.create_shader_module(wgpu::include_wgsl!("./shader.wgsl"));

    // render pipeline
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        // you can have more bind group, maybe that is how you switch out the buffers in the bind group
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: surface.get_capabilities(&adapter).formats[0],
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
    });

    // ecs and the important states
    let mut ecs = ecs::ECS::new(entry_point);

    // all the sprites, which is sortet then submitted to the storage buffer
    let mut sorted_sprites: Vec<Sprite> = Vec::with_capacity(max_sprites as usize);

    // texture map data
    let sprite_master = SpriteMaster3000::new(
        current_dir.clone(),
        max_sprites,
        &mut ecs.table,
        &queue,
        &animation_buffer,
        vec![Animation::new_empty(); max_sprites as usize],
    );

    let collision_manager = CollisionManager::new(&mut ecs.table);

    // ecs prep work
    ecs.table.add_state(uniform_data).unwrap();
    ecs.table.add_state(RunningState::Running).unwrap();
    ecs.table.add_state(sprite_master).unwrap();
    ecs.table.add_state(collision_manager).unwrap();
    ecs.table
        .add_state(MouseState {
            x: 0.0,
            y: 0.0,

            left: false,
            left_clicked: false,
            left_released: false,

            middle: false,
            middle_clicked: false,
            middle_released: false,

            right: false,
            right_clicked: false,
            right_released: false,

            wheel_delta: 0.0,
            in_screen: true,
            just_entered: false,
            just_left: false,
        })
        .unwrap();
    ecs.table
        .add_state(winit::event::ModifiersState::empty())
        .unwrap();
    ecs.table.add_state(KeyState::new()).unwrap();
    ecs.table
        .add_state::<Vec<winit::event::VirtualKeyCode>>(Vec::with_capacity(128))
        .unwrap();
    ecs.table.register_column::<Sprite>();
    ecs.table.register_column::<CollisionRect>();

    // custom prep work done to ecs
    (prep_func)(&mut ecs.table);

    event_loop.run(move |event, _, control_flow| {
        match ecs.table.read_state::<RunningState>().unwrap() {
            RunningState::Running => control_flow.set_poll(),
            RunningState::Closed => {
                control_flow.set_exit();
            }
        }
        match event {
            winit::event::Event::MainEventsCleared => window.request_redraw(),
            winit::event::Event::LoopDestroyed => {
                (post_func)(&mut ecs.table);
            }
            winit::event::Event::RedrawRequested(_) => {
                // local uniform -> table uniform
                uniform_data.utime = start_time.elapsed().as_secs_f32();
                uniform_data.delta_time = uniform_data.utime - uniform_data.last_utime;
                let uni = ecs.table.read_state::<Uniform>().unwrap();
                uni.utime = uniform_data.utime;
                uni.window_width = uniform_data.window_width;
                uni.window_height = uniform_data.window_height;
                uni.delta_time = uniform_data.delta_time;
                uni.last_utime = uniform_data.last_utime;

                // collision handling
                let collision_rects = ecs.table.read_column::<CollisionRect>().unwrap();
                ecs.table
                    .read_state::<CollisionManager>()
                    .unwrap()
                    .update(collision_rects);

                // depth sorting before ticking to prevent the jankness of sometimes depth data goes to 0 for some reason
                let sprites = ecs.table.read_column::<Sprite>().unwrap();
                unsafe {
                    sorted_sprites.set_len(sprites.len());
                    std::ptr::copy(sprites.as_ptr(), sorted_sprites.as_mut_ptr(), sprites.len());
                };
                sorted_sprites[0..sprites.len()].par_sort_unstable_by(|x, y| {
                    if x.base_depth == 0.5 && y.base_depth == 0.5 {
                        (y.pos_y - y.origin).total_cmp(&(x.pos_y - x.origin))
                    } else {
                        y.base_depth.total_cmp(&x.base_depth)
                    }
                });
                queue.write_buffer(
                    &sprite_buffer,
                    0,
                    bytemuck::cast_slice(unsafe {
                        from_raw_parts(
                            sorted_sprites.as_ptr() as *const u8,
                            sprites.len() * std::mem::size_of::<Sprite>(),
                        )
                    }),
                );

                // ecs ticking
                ecs.tick();

                // reset some states after ticking
                ecs.table.read_state::<KeyState>().unwrap().reset();
                ecs.table.read_state::<MouseState>().unwrap().reset();

                // uniform stuff
                // after ticking we can adjust the last_utime
                uniform_data.last_utime = uniform_data.utime;
                // table uniform -> local uniform
                let uni = ecs.table.read_state::<Uniform>().unwrap();
                uniform_data.height_resolution = uni.height_resolution;
                uniform_data.global_offset_x = uni.global_offset_x;
                uniform_data.global_offset_y = uni.global_offset_y;
                // write uniform buffer
                queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&[uniform_data]));

                // create encoder
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                // copying buffer
                encoder.copy_buffer_to_buffer(
                    &animation_buffer,
                    0,
                    &swap_buffer,
                    0,
                    animation_buffer.size(),
                );
                let canvas = surface.get_current_texture().unwrap();
                let canvas_view = canvas
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &canvas_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 203.0 / 255.0,
                                g: 202.0 / 255.0,
                                b: 192.0 / 255.0,
                                a: 255.0 / 255.0,
                            }),
                            store: false,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                });
                render_pass.set_pipeline(&pipeline);
                render_pass.set_bind_group(0, &bind_group, &[]);
                render_pass.draw(0..sprites.len() as u32 * 6, 0..1);
                drop(render_pass);
                // submit all the changes in this frame
                queue.submit(Some(encoder.finish()));
                // present the result
                canvas.present();

                // mapping the buffer, after waiting for it to map copy the content to the host side of the buffer, then finally unmap it
                swap_buffer.slice(..).map_async(wgpu::MapMode::Read, |x| {});
                device.poll(wgpu::MaintainBase::Wait);
                ecs.table
                    .read_state::<SpriteMaster3000>()
                    .unwrap()
                    .anim_data
                    .clone_from_slice(bytemuck::cast_slice::<u8, Animation>(
                        &swap_buffer.slice(..).get_mapped_range()[..],
                    ));
                swap_buffer.unmap();
            }
            winit::event::Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::Resized(new_size) => {
                    surface_config.height = new_size.height;
                    surface_config.width = new_size.width;
                    surface.configure(&device, &surface_config);
                    depth_texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: None,
                        size: wgpu::Extent3d {
                            width: new_size.width,
                            height: new_size.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Depth32Float,
                        usage: wgpu::TextureUsages::COPY_DST
                            | wgpu::TextureUsages::COPY_SRC
                            | wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    });
                    uniform_data.window_width = new_size.width as f32;
                    uniform_data.window_height = new_size.height as f32;
                    window.request_redraw();
                }
                winit::event::WindowEvent::CloseRequested => {
                    control_flow.set_exit();
                }

                winit::event::WindowEvent::KeyboardInput { input, .. } => {
                    let key_state = ecs.table.read_state::<KeyState>().unwrap();
                    if let Some(code) = input.virtual_keycode {
                        match input.state {
                            winit::event::ElementState::Pressed => key_state.press(code),
                            winit::event::ElementState::Released => key_state.release(code),
                        }
                    }
                }

                winit::event::WindowEvent::MouseWheel { delta, .. } => match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => {
                        ecs.table.read_state::<MouseState>().unwrap().wheel_delta = y;
                    }
                    winit::event::MouseScrollDelta::PixelDelta(_) => {}
                },

                winit::event::WindowEvent::CursorEntered { .. } => {
                    ecs.table.read_state::<MouseState>().unwrap().just_entered = true;
                }
                winit::event::WindowEvent::CursorLeft { .. } => {
                    ecs.table.read_state::<MouseState>().unwrap().just_left = true;
                }

                winit::event::WindowEvent::ModifiersChanged(mod_state) => {
                    *ecs.table
                        .read_state::<winit::event::ModifiersState>()
                        .unwrap() = mod_state;
                }

                winit::event::WindowEvent::CursorMoved { position, .. } => {
                    ecs.table.read_state::<MouseState>().unwrap().x = position.x as f32;
                    ecs.table.read_state::<MouseState>().unwrap().y = position.y as f32;
                }
                winit::event::WindowEvent::MouseInput { state, button, .. } => match button {
                    winit::event::MouseButton::Left => match state {
                        winit::event::ElementState::Pressed => {
                            let mouse_state = ecs.table.read_state::<MouseState>().unwrap();
                            mouse_state.left = true;
                            mouse_state.left_clicked = true;
                            mouse_state.left_released = false;
                        }
                        winit::event::ElementState::Released => {
                            let mouse_state = ecs.table.read_state::<MouseState>().unwrap();
                            mouse_state.left = false;
                            mouse_state.left_released = true;
                            mouse_state.left_clicked = false;
                        }
                    },
                    winit::event::MouseButton::Middle => match state {
                        winit::event::ElementState::Pressed => {
                            let mouse_state = ecs.table.read_state::<MouseState>().unwrap();
                            mouse_state.middle = true;
                            mouse_state.middle_clicked = true;
                            mouse_state.middle_released = false;
                        }
                        winit::event::ElementState::Released => {
                            let mouse_state = ecs.table.read_state::<MouseState>().unwrap();
                            mouse_state.middle = false;
                            mouse_state.middle_released = true;
                            mouse_state.middle_clicked = false;
                        }
                    },
                    winit::event::MouseButton::Right => match state {
                        winit::event::ElementState::Pressed => {
                            let mouse_state = ecs.table.read_state::<MouseState>().unwrap();
                            mouse_state.right = true;
                            mouse_state.right_clicked = true;
                            mouse_state.right_released = false;
                        }
                        winit::event::ElementState::Released => {
                            let mouse_state = ecs.table.read_state::<MouseState>().unwrap();
                            mouse_state.right = false;
                            mouse_state.right_released = true;
                            mouse_state.right_clicked = false;
                        }
                    },
                    _ => {}
                },
                _ => (),
            },
            _ => (),
        }
    })
}
