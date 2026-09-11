//! Native winit + wgpu frontend for Apex.

use std::{sync::Arc, time::Instant};

use anyhow::{Context, Result};
use apex_core::Buffer as EditorBuffer;
use glyphon::{
    Attrs, Buffer as TextBuffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use wgpu::{
    CommandEncoderDescriptor, CompositeAlphaMode, DeviceDescriptor, Instance, InstanceDescriptor,
    LoadOp, MultisampleState, Operations, PresentMode, RenderPassColorAttachment,
    RenderPassDescriptor, RequestAdapterOptions, SurfaceColorSpace, SurfaceConfiguration,
    TextureFormat, TextureUsages, TextureViewDescriptor,
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowId},
};

const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.035,
    g: 0.043,
    b: 0.059,
    a: 1.0,
};

/// Start the native editor event loop.
pub fn run(buffer: EditorBuffer) -> Result<()> {
    let event_loop = EventLoop::new().context("failed to create native event loop")?;
    let mut application = Application::new(buffer);
    event_loop
        .run_app(&mut application)
        .context("native event loop failed")
}

struct Application {
    editor: Option<EditorBuffer>,
    window_state: Option<WindowState>,
    modifiers: ModifiersState,
}

impl Application {
    fn new(buffer: EditorBuffer) -> Self {
        Self {
            editor: Some(buffer),
            window_state: None,
            modifiers: ModifiersState::default(),
        }
    }

    fn handle_key(&mut self, event: &KeyEvent) {
        if event.state != ElementState::Pressed {
            return;
        }
        let Some(state) = self.window_state.as_mut() else {
            return;
        };

        match &event.logical_key {
            Key::Named(NamedKey::Backspace) => state.editor.backspace(),
            Key::Named(NamedKey::ArrowLeft) => state.editor.move_left(),
            Key::Named(NamedKey::ArrowRight) => state.editor.move_right(),
            Key::Named(NamedKey::Enter) => state.editor.insert("\n"),
            Key::Named(NamedKey::Tab) => state.editor.insert("    "),
            Key::Character(character)
                if self.modifiers.control_key() && character.eq_ignore_ascii_case("s") =>
            {
                if let Err(error) = state.editor.save() {
                    eprintln!("Apex: {error}");
                }
            }
            Key::Character(character)
                if !self.modifiers.control_key()
                    && !self.modifiers.super_key()
                    && !self.modifiers.alt_key() =>
            {
                state.editor.insert(character);
            }
            _ => return,
        }
        state.reset_blink();
        state.window.request_redraw();
    }
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window_state.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Apex")
            .with_inner_size(LogicalSize::new(1100.0, 720.0));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .expect("failed to create Apex window"),
        );
        let editor = self.editor.take().expect("editor state initialized once");
        self.window_state = Some(pollster::block_on(WindowState::new(
            window, event_loop, editor,
        )));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } => self.handle_key(&event),
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                if let Some(state) = self.window_state.as_mut() {
                    state.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(state) = self.window_state.as_mut() {
                    state.render();
                }
            }
            _ => {}
        }
    }
}

struct WindowState {
    instance: Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: SurfaceConfiguration,
    editor: EditorBuffer,
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    renderer: TextRenderer,
    text_buffer: TextBuffer,
    blink_started: Instant,
    window: Arc<Window>,
}

impl WindowState {
    async fn new(window: Arc<Window>, event_loop: &ActiveEventLoop, editor: EditorBuffer) -> Self {
        let size = window.inner_size();
        let instance = Instance::new(InstanceDescriptor::new_with_display_handle(Box::new(
            event_loop.owned_display_handle(),
        )));
        let surface = instance
            .create_surface(window.clone())
            .expect("failed to create GPU surface");
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("no compatible graphics adapter found");
        let (device, queue) = adapter
            .request_device(&DeviceDescriptor::default())
            .await
            .expect("failed to create GPU device");
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(TextureFormat::is_srgb)
            .unwrap_or(capabilities.formats[0]);
        let present_mode = if capabilities.present_modes.contains(&PresentMode::AutoVsync) {
            PresentMode::AutoVsync
        } else {
            PresentMode::Fifo
        };
        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &surface_config);

        let mut font_system = FontSystem::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::new(&device, &queue, &cache, format);
        let renderer = TextRenderer::new(&mut atlas, &device, MultisampleState::default(), None);
        let mut text_buffer = TextBuffer::new(&mut font_system, Metrics::new(17.0, 24.0));
        text_buffer.set_size(
            Some(size.width.saturating_sub(32) as f32),
            Some(size.height.saturating_sub(32) as f32),
        );

        Self {
            instance,
            device,
            queue,
            surface,
            surface_config,
            editor,
            font_system,
            swash_cache: SwashCache::new(),
            viewport,
            atlas,
            renderer,
            text_buffer,
            blink_started: Instant::now(),
            window,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.text_buffer.set_size(
            Some(width.saturating_sub(32) as f32),
            Some(height.saturating_sub(32) as f32),
        );
        self.window.request_redraw();
    }

    fn reset_blink(&mut self) {
        self.blink_started = Instant::now();
    }

    fn display_text(&self) -> String {
        let mut text = self.editor.text();
        if (self.blink_started.elapsed().as_millis() / 500).is_multiple_of(2) {
            let byte_index = text
                .char_indices()
                .nth(self.editor.cursor().char_index)
                .map_or(text.len(), |(index, _)| index);
            text.insert(byte_index, '│');
        }
        text
    }

    fn render(&mut self) {
        let display_text = self.display_text();
        self.text_buffer.set_text(
            &display_text,
            &Attrs::new().family(Family::Monospace),
            Shaping::Advanced,
            None,
        );
        self.text_buffer
            .shape_until_scroll(&mut self.font_system, false);
        self.viewport.update(
            &self.queue,
            Resolution {
                width: self.surface_config.width,
                height: self.surface_config.height,
            },
        );
        self.renderer
            .prepare(
                &self.device,
                &self.queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                [TextArea {
                    buffer: &self.text_buffer,
                    left: 16.0,
                    top: 16.0,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: self.surface_config.width as i32,
                        bottom: self.surface_config.height as i32,
                    },
                    default_color: Color::rgb(220, 225, 235),
                    custom_glyphs: &[],
                }],
                &mut self.swash_cache,
            )
            .expect("failed to prepare glyphs");

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                self.window.request_redraw();
                return;
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Suboptimal(_) => {
                self.surface.configure(&self.device, &self.surface_config);
                self.window.request_redraw();
                return;
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self
                    .instance
                    .create_surface(self.window.clone())
                    .expect("failed to recreate GPU surface");
                self.surface.configure(&self.device, &self.surface_config);
                self.window.request_redraw();
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => panic!("GPU surface validation error"),
        };
        let view = frame.texture.create_view(&TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Apex frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Apex editor pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(BACKGROUND),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.renderer
                .render(&self.atlas, &self.viewport, &mut pass)
                .expect("failed to render glyphs");
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        self.atlas.trim();

        // Continuous vsync redraw keeps the input-to-glyph path at display rate.
        self.window.request_redraw();
    }
}
