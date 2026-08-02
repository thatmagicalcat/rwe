use anyhow::{Context, Result};

#[allow(deprecated)]
use wgpu::rwh::{HasRawDisplayHandle, HasRawWindowHandle};

use layershellev::WindowStateUnit;

use crate::{animation::AnimationRenderer, wallpaper::WallpaperRenderer};

pub struct WgpuState {
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_configuration: wgpu::SurfaceConfiguration,

    wallpaper_renderer: WallpaperRenderer,
    animation_renderer: AnimationRenderer,
}

impl WgpuState {
    pub async fn new(window: &WindowStateUnit<()>) -> Result<Self> {
        let instance = wgpu::Instance::default();
        let surface = Self::create_surface(&instance, window)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .context("[WgpuState::new] in request_adapter")?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: adapter
                    .features()
                    .intersection(wgpu::Features::VERTEX_WRITABLE_STORAGE),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .unwrap();

        // FIXME: move animation renderer's triangle generation code to `resize` function so we
        // won't have to hardcode width and height here

        // let (width, height) = window.get_size();
        let caps = surface.get_capabilities(&adapter);
        let surface_configuration = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: caps.formats[0],
            width: 1920,
            height: 1080,
            present_mode: wgpu::PresentMode::Fifo, // V-Sync locked
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };

        let wallpaper_renderer = WallpaperRenderer::new(&device, &queue, &surface_configuration)?;
        let animation_renderer = AnimationRenderer::new(&device, &surface_configuration)?;

        Ok(Self {
            instance,
            surface,
            device,
            queue,
            surface_configuration,
            wallpaper_renderer,
            animation_renderer,
        })
    }

    #[allow(deprecated)]
    pub fn create_surface(
        instance: &wgpu::Instance,
        window: &WindowStateUnit<()>,
    ) -> anyhow::Result<wgpu::Surface<'static>> {
        Ok(unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(window.raw_display_handle()?),
                raw_window_handle: window.raw_window_handle()?,
            })?
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_configuration.width = width;
            self.surface_configuration.height = height;
            self.surface
                .configure(&self.device, &self.surface_configuration);
        }
    }

    pub fn update_mouse_position(&mut self, x: f32, y: f32) {
        self.animation_renderer
            .update_mouse_position(&self.queue, x, y);
    }

    pub fn mark_mouse_activity(&mut self) {
        self.animation_renderer.mark_mouse_activity();
    }

    pub fn mouse_left(&mut self) {
        self.animation_renderer.mouse_left();
    }

    pub fn render(&mut self) {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Occluded | wgpu::CurrentSurfaceTexture::Timeout => return,
            e => panic!("{e:?}"), // TODO: handle other cases
        };

        let texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());

        self.animation_renderer.render(&mut encoder, &self.queue);

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &texture_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        self.wallpaper_renderer.render(&mut render_pass);
        self.animation_renderer.render_pass(&mut render_pass);

        drop(render_pass);
        self.queue.submit([encoder.finish()]);
        self.queue.present(surface_texture);
    }
}
