//! wgpu renderer: YUV video quad (converted on the GPU), then the bezel on top.

use crate::frame::Bezel;
use std::sync::Arc;
use winit::window::Window;

/// Decoded I420 frame, planes stored with their strides.
#[derive(Default)]
pub struct Yuv {
    pub width: u32,
    pub height: u32,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
    pub stride_y: u32,
    pub stride_uv: u32,
}

struct Layer {
    ubuf: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

struct VideoGpu {
    width: u32,
    height: u32,
    tex: [wgpu::Texture; 3],
    video: Layer,
    solid: Layer,
}

struct OverlayGpu {
    layer: Layer,
    width: u32,
    height: u32,
    opacity: f32,
}

struct BezelGpu {
    layer: Layer,
    hole: [f32; 4],
    aspect: f32,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    p_video: wgpu::RenderPipeline,
    p_solid: wgpu::RenderPipeline,
    p_bezel: wgpu::RenderPipeline,
    video: Option<VideoGpu>,
    bezel: Option<BezelGpu>,
    /// Window-only overlay (REC indicator, toasts). Never drawn into captures.
    overlay: Option<OverlayGpu>,
    /// The toolbar beside the phone (window only).
    toolbar: Option<OverlayGpu>,
    toolbar_on: bool,
    /// The device is in landscape: the bezel is drawn rotated by 90 degrees.
    landscape: bool,
    /// Video rectangle in window space (x, y, w, h), valid after `layout()`.
    pub video_rect: [f32; 4],
}

fn plane(device: &wgpu::Device, w: u32, h: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("plane"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

impl Renderer {
    pub fn new(window: Arc<Window>, bezel: Option<&Bezel>) -> Result<Self, String> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        // Vulkan on some Intel drivers fails to create a device; D3D12 is the dependable Windows backend
        if cfg!(windows) && std::env::var_os("WGPU_BACKEND").is_none() {
            desc.backends = wgpu::Backends::DX12;
        }
        // the default HWND swapchain is opaque; a DirectComposition one lets the window be transparent
        if cfg!(windows) && std::env::var_os("WGPU_DX12_PRESENTATION_SYSTEM").is_none() {
            desc.backend_options.dx12.presentation_system = wgpu::Dx12SwapchainKind::DxgiFromVisual;
        }
        let instance = wgpu::Instance::new(desc);
        let surface = instance.create_surface(window.clone()).map_err(|e| format!("surface: {e}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .map_err(|e| format!("adapter: {e}"))?;
        if std::env::var_os("HANDSET_DEBUG").is_some() {
            eprintln!("adapter: {:?}", adapter.get_info());
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .map_err(|e| format!("device: {e}"))?;

        let caps = surface.get_capabilities(&adapter);
        // Video is already gamma-encoded, so avoid an sRGB surface.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let alpha_mode = [wgpu::CompositeAlphaMode::PreMultiplied, wgpu::CompositeAlphaMode::PostMultiplied]
            .into_iter()
            .find(|m| caps.alpha_modes.contains(m))
            .unwrap_or(caps.alpha_modes[0]);
        if std::env::var_os("HANDSET_DEBUG").is_some() {
            eprintln!("surface: alpha {alpha_mode:?} of {:?}, format {format:?}", caps.alpha_modes);
        }
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 1,
            alpha_mode,
            color_space: wgpu::SurfaceColorSpace::Auto,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                tex_entry(2),
                tex_entry(3),
                tex_entry(4),
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let (p_video, p_solid, p_bezel) = (pipeline("fs_video"), pipeline("fs_solid"), pipeline("fs_bezel"));

        let mut r = Self {
            surface,
            device,
            queue,
            config,
            layout,
            sampler,
            p_video,
            p_solid,
            p_bezel,
            video: None,
            bezel: None,
            overlay: None,
            toolbar: None,
            toolbar_on: true,
            landscape: false,
            video_rect: [0.0, 0.0, 1.0, 1.0],
        };
        if let Some(b) = bezel {
            r.set_bezel(b);
        }
        Ok(r)
    }

    fn make_layer(&self, views: [&wgpu::TextureView; 3]) -> Layer {
        let ubuf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(views[0]) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(views[1]) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(views[2]) },
            ],
        });
        Layer { ubuf, bind }
    }

    fn set_bezel(&mut self, b: &Bezel) {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bezel"),
            size: wgpu::Extent3d { width: b.width, height: b.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            tex.as_image_copy(),
            &b.rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(b.width * 4), rows_per_image: None },
            wgpu::Extent3d { width: b.width, height: b.height, depth_or_array_layers: 1 },
        );
        let view = tex.create_view(&Default::default());
        let layer = self.make_layer([&view, &view, &view]);
        self.bezel = Some(BezelGpu { layer, hole: b.hole, aspect: b.aspect() });
    }

    fn build_overlay(&self, p: &crate::overlay::Pill, opacity: f32) -> OverlayGpu {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("overlay"),
            size: wgpu::Extent3d { width: p.width, height: p.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            tex.as_image_copy(),
            &p.rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(p.width * 4), rows_per_image: None },
            wgpu::Extent3d { width: p.width, height: p.height, depth_or_array_layers: 1 },
        );
        let view = tex.create_view(&Default::default());
        let layer = self.make_layer([&view, &view, &view]);
        OverlayGpu { layer, width: p.width, height: p.height, opacity }
    }

    /// Replace (or clear) the overlay pill.
    pub fn set_overlay(&mut self, pill: Option<&crate::overlay::Pill>) {
        let opacity = self.overlay.as_ref().map_or(1.0, |o| o.opacity);
        self.overlay = pill.map(|p| self.build_overlay(p, opacity));
    }

    /// Replace (or clear) the toolbar image.
    pub fn set_toolbar_image(&mut self, pill: Option<&crate::overlay::Pill>) {
        self.toolbar = pill.map(|p| self.build_overlay(p, 1.0));
    }

    pub fn toolbar_visible(&self) -> bool {
        self.toolbar_on
    }

    pub fn set_toolbar_visible(&mut self, on: bool) {
        self.toolbar_on = on;
        if !on {
            self.toolbar = None;
        }
    }

    /// Width reserved beside the phone, as a fraction of the window height.
    pub fn side_frac(&self) -> f32 {
        if self.toolbar_on { crate::toolbar::SIDE_FRAC } else { 0.0 }
    }

    /// Overlay pill position in window space: bottom centre of the video.
    fn overlay_rect(video_rect: [f32; 4], o: &OverlayGpu, ww: f32, wh: f32) -> [f32; 4] {
        let (pw, ph) = (o.width as f32 / ww, o.height as f32 / wh);
        [video_rect[0] + (video_rect[2] - pw) / 2.0, video_rect[1] + video_rect[3] * 0.93 - ph, pw, ph]
    }

    /// Is this window-pixel point on the overlay pill (toast / REC)?
    pub fn overlay_contains(&self, x: f32, y: f32) -> bool {
        let Some(o) = &self.overlay else { return false };
        let (ww, wh) = (self.config.width as f32, self.config.height as f32);
        let r = Self::overlay_rect(self.video_rect, o, ww, wh);
        x >= r[0] * ww && x < (r[0] + r[2]) * ww && y >= r[1] * wh && y < (r[1] + r[3]) * wh
    }

    pub fn set_overlay_opacity(&mut self, opacity: f32) {
        if let Some(o) = self.overlay.as_mut() {
            o.opacity = opacity;
        }
    }

    /// Swap the bezel at runtime (`None` removes it).
    pub fn replace_bezel(&mut self, b: Option<&Bezel>) {
        match b {
            Some(b) => self.set_bezel(b),
            None => self.bezel = None,
        }
    }

    /// Aspect ratio (w / h) of what the window shows: the bezel, else the video.
    pub fn content_aspect(&self) -> Option<f32> {
        self.bezel_aspect().or_else(|| self.video.as_ref().map(|v| v.width as f32 / v.height as f32))
    }

    /// Aspect ratio (w / h) of the whole window, including the toolbar strip.
    pub fn window_aspect(&self) -> Option<f32> {
        self.content_aspect().map(|a| a + self.side_frac())
    }

    pub fn landscape(&self) -> bool {
        self.landscape
    }

    pub fn set_landscape(&mut self, landscape: bool) {
        self.landscape = landscape;
    }

    /// Aspect ratio (w / h) of the bezel as currently drawn.
    pub fn bezel_aspect(&self) -> Option<f32> {
        self.bezel.as_ref().map(|b| if self.landscape { 1.0 / b.aspect } else { b.aspect })
    }

    /// Screen area of the bezel as drawn, as fractions of the bezel (x, y, w, h).
    fn hole(&self) -> Option<[f32; 4]> {
        self.bezel.as_ref().map(|b| {
            let [x, y, w, h] = b.hole;
            if self.landscape { [1.0 - y - h, x, h, w] } else { [x, y, w, h] }
        })
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
    }

    /// Upload a decoded frame (recreating the planes if the size changed).
    pub fn upload(&mut self, f: &Yuv) {
        if self.video.as_ref().is_none_or(|v| v.width != f.width || v.height != f.height) {
            let (cw, ch) = (f.width / 2, f.height / 2);
            let tex =
                [plane(&self.device, f.width, f.height), plane(&self.device, cw, ch), plane(&self.device, cw, ch)];
            let views: Vec<_> = tex.iter().map(|t| t.create_view(&Default::default())).collect();
            let video = self.make_layer([&views[0], &views[1], &views[2]]);
            let solid = self.make_layer([&views[0], &views[1], &views[2]]);
            self.video = Some(VideoGpu { width: f.width, height: f.height, tex, video, solid });
        }
        let v = self.video.as_ref().unwrap();
        let planes = [
            (&f.y, f.stride_y, f.width, f.height),
            (&f.u, f.stride_uv, f.width / 2, f.height / 2),
            (&f.v, f.stride_uv, f.width / 2, f.height / 2),
        ];
        for (i, (data, stride, w, h)) in planes.into_iter().enumerate() {
            self.queue.write_texture(
                v.tex[i].as_image_copy(),
                data,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(stride), rows_per_image: None },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
        }
    }

    /// Where the bezel (or the whole target) sits, in target space, leaving
    /// `side` pixels free at the right (the toolbar strip).
    fn bezel_rect_in(&self, ww: f32, wh: f32, side: f32) -> [f32; 4] {
        let aw = ww - side;
        match self.bezel_aspect() {
            None => [0.0, 0.0, aw / ww, 1.0],
            Some(aspect) => {
                let (bw, bh) = if aw / wh > aspect { (wh * aspect, wh) } else { (aw, aw / aspect) };
                [((aw - bw) / 2.0) / ww, ((wh - bh) / 2.0) / wh, bw / ww, bh / wh]
            }
        }
    }

    fn side_px(&self, wh: f32) -> f32 {
        self.side_frac() * wh
    }

    /// Where the bezel (or the whole window) sits, in window space.
    pub fn bezel_rect(&self) -> [f32; 4] {
        let (ww, wh) = (self.config.width as f32, self.config.height as f32);
        self.bezel_rect_in(ww, wh, self.side_px(wh))
    }

    fn update_uniforms(&mut self, ww: f32, wh: f32, remember: bool) {
        let Some(v) = &self.video else { return };
        let br = self.bezel_rect_in(ww, wh, if remember { self.side_px(wh) } else { 0.0 });
        let hole = match self.hole() {
            Some(h) => [br[0] + h[0] * br[2], br[1] + h[1] * br[3], h[2] * br[2], h[3] * br[3]],
            None => br,
        };
        // letterbox the video inside the screen area (in pixels, to keep the aspect)
        let (hw, hh) = (hole[2] * ww, hole[3] * wh);
        let s = (hw / v.width as f32).min(hh / v.height as f32);
        let (vw, vh) = (v.width as f32 * s / ww, v.height as f32 * s / wh);
        let video_rect = [hole[0] + (hole[2] - vw) / 2.0, hole[1] + (hole[3] - vh) / 2.0, vw, vh];
        if remember {
            self.video_rect = video_rect;
        }

        let write = |buf: &wgpu::Buffer, r: [f32; 4], rot: f32, opacity: f32| {
            let data = [r[0], r[1], r[2], r[3], rot, opacity, 0.0, 0.0];
            self.queue.write_buffer(buf, 0, bytemuck::cast_slice(&data));
        };
        write(&v.video.ubuf, video_rect, 0.0, 1.0);
        write(&v.solid.ubuf, hole, 0.0, 1.0);
        if let Some(b) = &self.bezel {
            write(&b.layer.ubuf, br, if self.landscape { 1.0 } else { 0.0 }, 1.0);
        }
        // the overlay is only laid out for the window: bottom centre of the screen
        if remember {
            if let Some(t) = &self.toolbar {
                let [x, y, w, h] = crate::toolbar::rect(ww, wh);
                write(&t.layer.ubuf, [x / ww, y / wh, w / ww, h / wh], 0.0, 1.0);
            }
            if let Some(o) = &self.overlay {
                write(&o.layer.ubuf, Self::overlay_rect(video_rect, o, ww, wh), 0.0, o.opacity);
            }
        }
    }

    fn draw_into(&mut self, view: &wgpu::TextureView, w: u32, h: u32, clear: wgpu::Color, remember: bool) {
        self.update_uniforms(w as f32, h as f32, remember);
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(v) = &self.video {
                if self.bezel.is_some() {
                    pass.set_pipeline(&self.p_solid);
                    pass.set_bind_group(0, &v.solid.bind, &[]);
                    pass.draw(0..6, 0..1);
                }
                pass.set_pipeline(&self.p_video);
                pass.set_bind_group(0, &v.video.bind, &[]);
                pass.draw(0..6, 0..1);
            }
            if let Some(b) = &self.bezel {
                pass.set_pipeline(&self.p_bezel);
                pass.set_bind_group(0, &b.layer.bind, &[]);
                pass.draw(0..6, 0..1);
            }
            // window only: captures must not contain the toolbar or overlay
            if remember {
                if let Some(t) = &self.toolbar {
                    pass.set_pipeline(&self.p_bezel);
                    pass.set_bind_group(0, &t.layer.bind, &[]);
                    pass.draw(0..6, 0..1);
                }
                if let Some(o) = self.overlay.as_ref().filter(|o| o.opacity > 0.0) {
                    pass.set_pipeline(&self.p_bezel);
                    pass.set_bind_group(0, &o.layer.bind, &[]);
                    pass.draw(0..6, 0..1);
                }
            }
        }
        self.queue.submit([enc.finish()]);
    }

    pub fn render(&mut self) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            _ => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let (w, h) = (self.config.width, self.config.height);
        self.draw_into(&view, w, h, wgpu::Color::TRANSPARENT, true);
        self.queue.present(frame);
    }

    /// Pixel size of a capture whose longest side is `long_side`, following
    /// the bezel's aspect (or the video's without a bezel). Even, for video.
    pub fn capture_size(&self, long_side: u32) -> Option<(u32, u32)> {
        let aspect = self.bezel_aspect().or_else(|| self.video.as_ref().map(|v| v.width as f32 / v.height as f32))?;
        let even = |v: f32| ((v / 2.0).round() as u32).max(1) * 2;
        Some(if aspect >= 1.0 {
            (even(long_side as f32), even(long_side as f32 / aspect))
        } else {
            (even(long_side as f32 * aspect), even(long_side as f32))
        })
    }

    /// Render the composed phone offscreen and read it back as straight-alpha
    /// RGBA8. `bg` is an opaque background colour, or `None` to keep the
    /// transparent surroundings.
    pub fn capture(&mut self, (w, h): (u32, u32), bg: Option<[f32; 3]>) -> Option<Vec<u8>> {
        self.video.as_ref()?;
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let clear = match bg {
            Some([r, g, b]) => wgpu::Color { r: r as f64, g: g as f64, b: b as f64, a: 1.0 },
            None => wgpu::Color::TRANSPARENT,
        };
        self.draw_into(&view, w, h, clear, false);

        let bpr = (w * 4 + 255) & !255; // rows must be 256-byte aligned
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (bpr * h) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: None },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);

        let (tx, rx) = std::sync::mpsc::channel();
        buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv().ok()?.ok()?;

        let bgra = matches!(self.config.format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        {
            let data = buf.slice(..).get_mapped_range().ok()?;
            for row in data.chunks_exact(bpr as usize).take(h as usize) {
                for px in row[..(w * 4) as usize].as_chunks::<4>().0 {
                    let (r, g, b, a) = if bgra { (px[2], px[1], px[0], px[3]) } else { (px[0], px[1], px[2], px[3]) };
                    // the surface is premultiplied; PNG / video want straight alpha
                    let un = |c: u8| {
                        if a == 0 || a == 255 { c } else { ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 }
                    };
                    out.extend_from_slice(&[un(r), un(g), un(b), a]);
                }
            }
        }
        buf.unmap();
        Some(out)
    }
}
