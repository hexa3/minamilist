//! Native winit + wgpu frontend for Apex.

use std::{sync::Arc, time::Instant};

use anyhow::{Context, Result};
use apex_core::{Buffer as EditorBuffer, CommandId, CommandRegistry, ProjectIndex};
use apex_syntax::{HighlightKind, Language, SyntaxDocument};
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

        if self.modifiers.control_key()
            && self.modifiers.shift_key()
            && matches!(&event.logical_key, Key::Character(character) if character.eq_ignore_ascii_case("p"))
        {
            state.palette = Some(Palette::commands());
            state.window.request_redraw();
            return;
        }
        if self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(character) if character.eq_ignore_ascii_case("p"))
        {
            state.palette = Some(Palette::files());
            state.window.request_redraw();
            return;
        }
        if state.palette.is_some() {
            match &event.logical_key {
                Key::Named(NamedKey::Escape) => state.palette = None,
                Key::Named(NamedKey::Backspace) => {
                    state.palette.as_mut().expect("checked above").query.pop();
                }
                Key::Named(NamedKey::ArrowDown) => state.move_palette_selection(1),
                Key::Named(NamedKey::ArrowUp) => state.move_palette_selection(-1),
                Key::Named(NamedKey::Enter) => state.accept_palette(),
                Key::Character(character)
                    if !self.modifiers.control_key()
                        && !self.modifiers.super_key()
                        && !self.modifiers.alt_key() =>
                {
                    let palette = state.palette.as_mut().expect("checked above");
                    palette.query.push_str(character);
                    palette.selected = 0;
                }
                _ => return,
            }
            state.window.request_redraw();
            return;
        }

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
        state.sync_syntax();
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
    syntax: Option<SyntaxDocument>,
    commands: CommandRegistry,
    project: Option<ProjectIndex>,
    palette: Option<Palette>,
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

        let syntax = editor
            .path()
            .and_then(Language::from_path)
            .and_then(|language| SyntaxDocument::new(language, editor.text()).ok());
        let project = std::env::current_dir()
            .ok()
            .and_then(|root| ProjectIndex::scan(root).ok());
        Self {
            instance,
            device,
            queue,
            surface,
            surface_config,
            editor,
            syntax,
            commands: CommandRegistry::default(),
            project,
            palette: None,
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

    fn sync_syntax(&mut self) {
        if let Some(syntax) = self.syntax.as_mut()
            && let Err(error) = syntax.update_source(&self.editor.text())
        {
            eprintln!("Apex syntax: {error}");
        }
    }

    fn move_palette_selection(&mut self, delta: isize) {
        let count = self.palette_items().len();
        let Some(palette) = self.palette.as_mut() else {
            return;
        };
        if count == 0 {
            palette.selected = 0;
        } else {
            palette.selected = palette.selected.saturating_add_signed(delta).min(count - 1);
        }
    }

    fn palette_items(&self) -> Vec<(String, Option<CommandId>)> {
        let Some(palette) = self.palette.as_ref() else {
            return Vec::new();
        };
        match palette.kind {
            PaletteKind::Commands => self
                .commands
                .search(&palette.query)
                .into_iter()
                .take(8)
                .map(|command| {
                    let shortcut = command
                        .shortcut
                        .map_or(String::new(), |value| format!("  {value}"));
                    (format!("{}{}", command.title, shortcut), Some(command.id))
                })
                .collect(),
            PaletteKind::Files => self
                .project
                .as_ref()
                .map(|project| {
                    project
                        .find_files(&palette.query, 8)
                        .into_iter()
                        .map(|entry| (entry.relative_path.display().to_string(), None))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    fn accept_palette(&mut self) {
        let Some(palette) = self.palette.as_ref() else {
            return;
        };
        let selected = palette.selected;
        let kind = palette.kind;
        let Some((label, command)) = self.palette_items().into_iter().nth(selected) else {
            return;
        };
        self.palette = None;
        match kind {
            PaletteKind::Files => {
                let Some(project) = self.project.as_ref() else {
                    return;
                };
                match EditorBuffer::open(project.root().join(label)) {
                    Ok(editor) => {
                        self.editor = editor;
                        self.syntax =
                            self.editor
                                .path()
                                .and_then(Language::from_path)
                                .and_then(|language| {
                                    SyntaxDocument::new(language, self.editor.text()).ok()
                                });
                    }
                    Err(error) => eprintln!("Apex: {error}"),
                }
            }
            PaletteKind::Commands => match command {
                Some(CommandId("file.save")) => {
                    if let Err(error) = self.editor.save() {
                        eprintln!("Apex: {error}");
                    }
                }
                Some(CommandId("file.find")) => self.palette = Some(Palette::files()),
                Some(CommandId("palette.open")) => self.palette = Some(Palette::commands()),
                _ => {}
            },
        }
        self.reset_blink();
    }

    fn palette_text(&self) -> Option<String> {
        let palette = self.palette.as_ref()?;
        let heading = match palette.kind {
            PaletteKind::Commands => "COMMAND PALETTE",
            PaletteKind::Files => "GO TO FILE",
        };
        let mut text = format!("{heading}\n> {}\n", palette.query);
        for (index, (label, _)) in self.palette_items().into_iter().enumerate() {
            let marker = if index == palette.selected {
                "›"
            } else {
                " "
            };
            text.push_str(&format!("{marker} {label}\n"));
        }
        text.push_str("────────────────────────────────────────\n");
        Some(text)
    }

    fn display_text(&self) -> (String, Option<usize>) {
        let mut text = self.editor.text();
        if let Some(palette) = self.palette_text() {
            text.insert_str(0, &palette);
            return (text, None);
        }
        if (self.blink_started.elapsed().as_millis() / 500).is_multiple_of(2) {
            let byte_index = text
                .char_indices()
                .nth(self.editor.cursor().char_index)
                .map_or(text.len(), |(index, _)| index);
            text.insert(byte_index, '│');
            return (text, Some(byte_index));
        }
        (text, None)
    }

    fn render(&mut self) {
        let (display_text, cursor_byte) = self.display_text();
        let default_attrs = Attrs::new().family(Family::Monospace);
        if let Some(syntax) = self.syntax.as_ref().filter(|_| self.palette.is_none()) {
            let spans = syntax.highlights();
            let rich_text = rich_text_segments(&display_text, &spans, cursor_byte);
            self.text_buffer
                .set_rich_text(rich_text, &default_attrs, Shaping::Advanced, None);
        } else {
            self.text_buffer
                .set_text(&display_text, &default_attrs, Shaping::Advanced, None);
        }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PaletteKind {
    Commands,
    Files,
}

#[derive(Debug)]
struct Palette {
    kind: PaletteKind,
    query: String,
    selected: usize,
}

impl Palette {
    fn commands() -> Self {
        Self {
            kind: PaletteKind::Commands,
            query: String::new(),
            selected: 0,
        }
    }

    fn files() -> Self {
        Self {
            kind: PaletteKind::Files,
            query: String::new(),
            selected: 0,
        }
    }
}

fn rich_text_segments<'text>(
    text: &'text str,
    spans: &[apex_syntax::HighlightSpan],
    cursor_byte: Option<usize>,
) -> Vec<(&'text str, Attrs<'static>)> {
    const CURSOR_LEN: usize = '│'.len_utf8();
    let mut segments = Vec::with_capacity(spans.len() * 2 + 1);
    let mut consumed = 0;
    for span in spans {
        let mut start = span.bytes.start;
        let mut end = span.bytes.end;
        if let Some(cursor) = cursor_byte {
            if cursor <= start {
                start += CURSOR_LEN;
                end += CURSOR_LEN;
            } else if cursor < end {
                end += CURSOR_LEN;
            }
        }
        if start < consumed
            || end > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
        {
            continue;
        }
        if consumed < start {
            segments.push((
                &text[consumed..start],
                Attrs::new().family(Family::Monospace),
            ));
        }
        segments.push((
            &text[start..end],
            Attrs::new()
                .family(Family::Monospace)
                .color(highlight_color(span.kind)),
        ));
        consumed = end;
    }
    if consumed < text.len() {
        segments.push((&text[consumed..], Attrs::new().family(Family::Monospace)));
    }
    if segments.is_empty() {
        segments.push((text, Attrs::new().family(Family::Monospace)));
    }
    segments
}

fn highlight_color(kind: HighlightKind) -> Color {
    match kind {
        HighlightKind::Comment => Color::rgb(106, 153, 85),
        HighlightKind::String => Color::rgb(206, 145, 120),
        HighlightKind::Number | HighlightKind::Constant => Color::rgb(181, 206, 168),
        HighlightKind::Keyword => Color::rgb(197, 134, 192),
        HighlightKind::Function => Color::rgb(220, 220, 170),
        HighlightKind::Type => Color::rgb(78, 201, 176),
    }
}
