#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_mut,
    unused_assignments,
    unreachable_code
)]
#![feature(path_file_prefix, alloc_layout_extra)]

pub use ecs;
pub use winit;

#[repr(C)]
#[derive(Debug, bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
// todo, find an appropriate data structure to resolve this from basic info
// todo, animation
// todo, mouse pos in shader
// todo, collision
/// specify the depth as 0.5 to enable y sorting
pub struct Sprite {
    pub pos_x: f32,
    pub pos_y: f32,

    pub width: f32,
    pub height: f32,

    pub tex_x: f32,
    pub tex_y: f32,

    pub tex_width: f32,
    pub tex_height: f32,

    pub depth: f32,
    pub origin: f32,

    pub frames: u32,
    /// if positive it loops all the time, if negative it only plays once
    pub duration: f32,
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

            depth: 0.0,
            origin: 0.0,

            duration: 0.0,
            frames: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
pub struct Uniform {
    pub height_resolution: f32,
    texture_width: f32,
    texture_height: f32,
    window_width: f32,
    window_height: f32,
    pub utime: f32,
    pub mouse_x: f32,
    pub mouse_y: f32,
    // todo move cam functionality, in pixel
    pub global_offset_x: f32,
    pub global_offset_y: f32,
}

#[derive(Clone, Copy)]
pub enum RunningState {
    Running,
    Closed,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MouseState {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MouseKeyEvent {
    pub key: winit::event::MouseButton,
    pub state: winit::event::ElementState,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MouseWheelEvent {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MouseCursorEvent {
    Enter,
    Left,
}

pub fn run(
    minimal_height_resolution: f32,
    max_sprites: u32,
    entry_point: fn(&mut ecs::Table),
    prep_func: fn(&mut ecs::Table),
) {
    // utime
    let start_time = std::time::Instant::now();
    // window and event loop stuff
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    let proxy = event_loop.create_proxy();

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
    let mut current_dir = std::env::current_dir().unwrap();
    let mut texture_dir = current_dir.clone();
    texture_dir.push("res/texture.png");
    let texture_data = image::io::Reader::open(texture_dir)
        .unwrap()
        .decode()
        .unwrap()
        .into_rgba8();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: texture_data.width(),
            height: texture_data.height(),
            depth_or_array_layers: 1,
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

    // todo
    let mut metadata_dir = current_dir.clone();
    metadata_dir.push("res/texture.png");
    let texture_meta_data = ();

    // uniform data
    let mut uniform_data = Uniform {
        height_resolution: minimal_height_resolution,
        texture_width: texture_data.width() as f32,
        texture_height: texture_data.height() as f32,
        window_width: 0.0,
        window_height: 0.0,
        utime: 0.0,
        mouse_x: 0.0,
        mouse_y: 0.0,
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

    // vertex buffer
    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        mapped_at_creation: false,
        size: 6 * max_sprites as u64,
        usage: wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::VERTEX,
    });

    // storage buffer
    let sprite_storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        mapped_at_creation: false,
        size: std::mem::size_of::<Sprite>() as u64 * max_sprites as u64,
        usage: wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::STORAGE,
    });
    let sprite_anim_data_storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        mapped_at_creation: false,
        // assuming that all sprites can be animated
        size: 8 * max_sprites as u64,
        usage: wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::STORAGE,
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
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
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
                    buffer: &sprite_storage_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &sprite_anim_data_storage_buffer,
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

    // all the sprites, which is sortet then submitted to the storage buffer
    let mut sorted_sprites = vec![Sprite::new_empty(); max_sprites as usize];

    // ecs and the important states
    let mut ecs = ecs::ECS::new(entry_point);
    ecs.table.add_state(uniform_data).unwrap();
    ecs.table.add_state(RunningState::Running).unwrap();
    ecs.table
        .add_state(winit::event::ModifiersState::empty())
        .unwrap();
    ecs.table.register_event::<winit::event::KeyboardInput>();
    ecs.table.register_event::<MouseKeyEvent>();
    ecs.table.register_event::<MouseWheelEvent>();
    ecs.table.register_event::<MouseCursorEvent>();
    ecs.table.register_column::<Sprite>();

    // custom prep work done to ecs
    (prep_func)(&mut ecs.table);

    event_loop.run(move |event, _, control_flow| {
        match ecs.table.read_state::<RunningState>().unwrap() {
            RunningState::Running => control_flow.set_poll(),
            RunningState::Closed => control_flow.set_exit(),
        }
        match event {
            winit::event::Event::MainEventsCleared => window.request_redraw(),
            winit::event::Event::RedrawRequested(_) => {
                // update utime
                uniform_data.utime = start_time.elapsed().as_secs_f32();
                ecs.table.read_state::<Uniform>().unwrap().utime = uniform_data.utime;

                // ecs ticking
                ecs.tick();

                // read updated uniform data
                let uni = ecs.table.read_state::<Uniform>().unwrap();
                // update height resolution
                uniform_data.height_resolution = uni.height_resolution;
                // update global_offset
                uniform_data.global_offset_x = uni.global_offset_x;
                uniform_data.global_offset_y = uni.global_offset_y;

                // load, sort sprites
                // todo, change to new double sized buffer if it's reaching limit
                let sprites = ecs.table.query_raw::<Sprite>().unwrap();
                sorted_sprites[0..sprites.len()].clone_from_slice(sprites);
                sorted_sprites[0..sprites.len()].sort_by(|x, y| {
                    if x.depth == 0.5 && y.depth == 0.5 {
                        (y.pos_y - y.origin).total_cmp(&(x.pos_y - x.origin))
                    } else {
                        y.depth.total_cmp(&x.depth)
                    }
                });

                // write buffers
                queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&[uniform_data]));
                queue.write_buffer(
                    &sprite_storage_buffer,
                    0,
                    bytemuck::cast_slice(&sorted_sprites[0..sorted_sprites.len()]),
                );

                // render
                let canvas = surface.get_current_texture().unwrap();
                let canvas_view = canvas
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
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
                render_pass.draw(0..sorted_sprites.len() as u32 * 6, 0..1);
                drop(render_pass);
                queue.submit(Some(encoder.finish()));
                canvas.present();
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
                winit::event::WindowEvent::CloseRequested => control_flow.set_exit(),
                winit::event::WindowEvent::KeyboardInput { input, .. } => {
                    // todo handle multiple key board inputs
                    // send the events (only once no repeat) to the event column in the ecs
                    if ecs
                        .table
                        .read_event::<winit::event::KeyboardInput>()
                        .unwrap()
                        .last()
                        != Some(&input)
                    {
                        ecs.table.fire_event(input.clone());
                    }
                }

                winit::event::WindowEvent::MouseInput { state, button, .. } => {
                    let event = MouseKeyEvent { key: button, state };
                    if ecs.table.read_event::<MouseKeyEvent>().unwrap().last() != Some(&event) {
                        ecs.table.fire_event(event);
                    }
                }

                winit::event::WindowEvent::MouseWheel { delta, phase, .. } => match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        let event = MouseWheelEvent { x, y };
                        if ecs.table.read_event::<MouseWheelEvent>().unwrap().last() != Some(&event)
                        {
                            ecs.table.fire_event(event);
                        }
                    }
                    winit::event::MouseScrollDelta::PixelDelta(_) => {}
                },

                winit::event::WindowEvent::ModifiersChanged(mod_state) => {
                    *ecs.table
                        .read_state::<winit::event::ModifiersState>()
                        .unwrap() = mod_state;
                }

                winit::event::WindowEvent::CursorMoved { position, .. } => {
                    uniform_data.mouse_x = position.x as f32;
                    uniform_data.mouse_y = position.y as f32;
                }

                winit::event::WindowEvent::CursorEntered { .. } => {
                    ecs.table.fire_event(MouseCursorEvent::Enter);
                }
                winit::event::WindowEvent::CursorLeft { .. } => {
                    ecs.table.fire_event(MouseCursorEvent::Left);
                }
                _ => (),
            },
            _ => (),
        }
    })
}
