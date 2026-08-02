use std::num::NonZeroU64;
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::{Vec2, vec2};
use wgpu::util::DeviceExt;

const WAVEFRONT_FACTOR: f32 = 5.0;
const MAX_DELAY: f32 = 2.0;
const ANIM_TIME: f32 = 0.3;
const TRIANGLE_SIZE: f32 = 140.0;
const RADIUS: f32 = 300.0;

const IDLE_TIMEOUT: f32 = 0.3;
const FADE_IN_TIME: f32 = 0.9;
const FADE_OUT_TIME: f32 = 0.9;
const MAX_DT: f32 = 0.1;

pub struct AnimationRenderer {
    uniforms: Uniforms,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    compute_pipeline: wgpu::ComputePipeline,
    storage_buffer: wgpu::Buffer,
    state_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    num_triangles: u32,
    last_frame: Instant,
    last_mouse_activity: Instant,
    mouse_active: bool,
    fade: f32,
}

impl AnimationRenderer {
    pub fn new(
        device: &wgpu::Device,
        surface_cfg: &wgpu::SurfaceConfiguration,
    ) -> anyhow::Result<Self> {
        let &wgpu::wgt::SurfaceConfiguration { width, height, .. } = surface_cfg;

        let delay_spread = MAX_DELAY.max(ANIM_TIME * WAVEFRONT_FACTOR);
        let (gpu_triangles, max_distance) = build_triangles(width as _, height as _, TRIANGLE_SIZE);
        let num_triangles = gpu_triangles.len() as u32;

        let shader_module =
            device.create_shader_module(wgpu::include_wgsl!("shader/animation.wgsl"));

        let uniforms = Uniforms {
            screen_size: [width as _, height as _],
            dt: 0.0,
            anim_time: ANIM_TIME,
            delay_spread,
            max_distance,
            cursor: [width as f32 / 2.0, height as f32 / 2.0],
            radius: RADIUS,
            num_triangles,
            fade: 0.0,
            _pad: 0.0,
            colors: PaletteName::Forest
                .colors()
                .map(|i| i.map(|i| i as f32 / 255.0)),
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("storage buffer"),
            contents: bytemuck::cast_slice(&gpu_triangles),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let state_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("triangle state buffer"),
            contents: bytemuck::cast_slice(&vec![TriangleState::zeroed(); gpu_triangles.len()]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::COMPUTE,
                    count: None,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            NonZeroU64::new(std::mem::size_of::<Uniforms>() as _).unwrap(),
                        ),
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::COMPUTE,
                    count: None,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::COMPUTE,
                    count: None,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: storage_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: state_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("renderer pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("compute pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_cfg.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            uniforms,
            bind_group,
            render_pipeline,
            compute_pipeline,
            storage_buffer,
            state_buffer,
            uniform_buffer,
            num_triangles,
            last_frame: Instant::now(),
            last_mouse_activity: Instant::now(),
            mouse_active: false,
            fade: 0.0,
        })
    }

    pub fn update_mouse_position(&mut self, queue: &wgpu::Queue, x: f32, y: f32) {
        self.uniforms.cursor = [x, y];
        self.mouse_active = true;
        self.last_mouse_activity = Instant::now();

        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );
    }

    pub fn mark_mouse_activity(&mut self) {
        self.mouse_active = true;
        self.last_mouse_activity = Instant::now();
    }

    pub fn mouse_left(&mut self) {
        self.mouse_active = false;
    }

    pub fn is_active(&self) -> bool {
        let idle = self.last_mouse_activity.elapsed().as_secs_f32();
        self.fade > 0.0 || (self.mouse_active && idle < IDLE_TIMEOUT)
    }

    pub fn render(&mut self, encoder: &mut wgpu::CommandEncoder, queue: &wgpu::Queue) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_frame)
            .as_secs_f32()
            .clamp(0.0, MAX_DT);
        self.last_frame = now;

        let idle = now.duration_since(self.last_mouse_activity).as_secs_f32();
        let target_fade = if self.mouse_active && idle < IDLE_TIMEOUT {
            1.0
        } else {
            0.0
        };

        let fade_time = if target_fade >= self.fade {
            FADE_IN_TIME
        } else {
            FADE_OUT_TIME
        };

        let k = 1.0 - (-dt / fade_time).exp();
        self.fade += (target_fade - self.fade) * k;
        if self.fade < 0.001 {
            self.fade = 0.0;
        }

        self.uniforms.dt = dt;
        self.uniforms.fade = self.fade;
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );

        if self.is_active() {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("animation compute pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.bind_group, &[]);
            compute_pass.dispatch_workgroups(self.num_triangles.div_ceil(64), 1, 1);

            drop(compute_pass);
        }
    }

    pub fn render_pass(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..self.num_triangles);
    }
}

fn build_triangles(w: f32, h: f32, side_length: f32) -> (Vec<GpuTriangle>, f32) {
    let padding = 6.0;
    let origin = vec2(-side_length, -side_length);
    let altitude = side_length * 3_f32.sqrt() / 2.0;

    let vertical = (h / side_length).ceil() as u32 + 4;
    let horizontal = (w / side_length).ceil() as u32 + 4;

    let center = vec2(w / 2.0, h / 2.0);

    let mut triangles = Vec::new();
    for i in 0..horizontal {
        for j in 0..vertical {
            let base = origin
                + vec2(
                    i as f32 * (altitude + padding / 2.0),
                    j as f32 * (side_length + padding),
                );

            let (t01, t02, t03, t11, t12, t13) = if i % 2 == 0 {
                let t01 = base;
                let t02 = t01 + vec2(altitude, side_length / 2.0);
                let t03 = t01 + vec2(0.0, side_length);

                let t11 = t03 + vec2(0.0, padding / 2.0);
                let t12 = t11 + vec2(altitude, -side_length / 2.0);
                let t13 = t11 + vec2(altitude, side_length / 2.0);

                (t01, t02, t03, t11, t12, t13)
            } else {
                let t01 = base + vec2(0.0, side_length / 2.0 - padding / 2.0);
                let t02 = t01 + vec2(0.0, -side_length);
                let t03 = t01 + vec2(altitude, -side_length / 2.0);

                let t11 = t01 + vec2(0.0, padding / 2.0);
                let t12 = t11 + vec2(altitude, -side_length / 2.0);
                let t13 = t11 + vec2(altitude, side_length / 2.0);

                (t01, t02, t03, t11, t12, t13)
            };

            let palette_index = (i * 7 + j * 13) % 4;

            let make_gpu_tri = |pa: Vec2, pb: Vec2, pc: Vec2, idx: u32| {
                let centroid = (pa + pb + pc) / 3.0;
                let distance = (centroid - center).length();
                let a_start = pb + (pc - pb) * 0.5;

                GpuTriangle {
                    pa: pa.into(),
                    pb: pb.into(),
                    pc: pc.into(),
                    a_start: a_start.into(),
                    distance,
                    palette_index: idx,
                }
            };

            triangles.push(make_gpu_tri(t02, t01, t03, palette_index));
            triangles.push(make_gpu_tri(t12, t11, t13, palette_index));
        }
    }

    let max_distance = triangles.iter().map(|t| t.distance).fold(0.0, f32::max);
    (triangles, max_distance)
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    dt: f32,
    anim_time: f32,
    delay_spread: f32,
    max_distance: f32,
    cursor: [f32; 2],
    radius: f32,
    num_triangles: u32,
    fade: f32,
    _pad: f32,
    colors: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuTriangle {
    pa: Vec2,
    pb: Vec2,
    pc: Vec2,
    a_start: Vec2,
    distance: f32,
    palette_index: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TriangleState {
    t: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PaletteName {
    Ember,
    Ocean,
    Synth,
    Forest,
    Mono,
}

impl PaletteName {
    fn colors(self) -> [[u8; 4]; 4] {
        match self {
            PaletteName::Ember => [
                [255, 69, 0, 255],
                [255, 106, 0, 255],
                [255, 140, 66, 255],
                [255, 196, 92, 255],
            ],
            PaletteName::Ocean => [
                [0, 71, 171, 255],
                [0, 180, 216, 255],
                [72, 202, 228, 255],
                [144, 224, 239, 255],
            ],
            PaletteName::Synth => [
                [106, 13, 173, 255],
                [201, 24, 74, 255],
                [255, 93, 143, 255],
                [255, 179, 198, 255],
            ],
            PaletteName::Forest => [
                [45, 106, 79, 255],
                [64, 145, 108, 255],
                [82, 183, 136, 255],
                [149, 213, 178, 255],
            ],
            PaletteName::Mono => [
                [255, 255, 255, 255],
                [217, 217, 217, 255],
                [166, 166, 166, 255],
                [115, 115, 115, 255],
            ],
        }
    }
}
