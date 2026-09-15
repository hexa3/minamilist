//! Native winit + wgpu frontend for minamilist.

use std::{path::PathBuf, sync::Arc, time::Instant};

mod python;

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
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowId},
};

use python::PythonRun;

const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.035,
    g: 0.043,
    b: 0.059,
    a: 1.0,
};

const PRODUCT_NAME: &str = "minamilist";
const PRODUCT_TAGLINE: &str = "the minamilist IDE";

/// Start the native editor event loop.
pub fn run(buffer: EditorBuffer, project_root: PathBuf) -> Result<()> {
    let event_loop = EventLoop::new().context("failed to create native event loop")?;
    let mut application = Application::new(buffer, project_root);
    event_loop
        .run_app(&mut application)
        .context("native event loop failed")
}

struct Application {
    editor: Option<EditorBuffer>,
    project_root: PathBuf,
    window_state: Option<WindowState>,
    modifiers: ModifiersState,
}

impl Application {
    fn new(buffer: EditorBuffer, project_root: PathBuf) -> Self {
        Self {
            editor: Some(buffer),
            project_root,
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

        let primary = self.modifiers.control_key() || self.modifiers.super_key();
        if primary
            && self.modifiers.shift_key()
            && matches!(&event.logical_key, Key::Character(character) if character.eq_ignore_ascii_case("p"))
        {
            state.palette = Some(Palette::commands());
            state.window.request_redraw();
            return;
        }
        if primary
            && matches!(&event.logical_key, Key::Character(character) if character.eq_ignore_ascii_case("p"))
        {
            state.palette = Some(Palette::files());
            state.window.request_redraw();
            return;
        }
        if primary
            && matches!(&event.logical_key, Key::Character(character) if character.eq_ignore_ascii_case("o"))
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
                Key::Named(NamedKey::Space) if !primary && !self.modifiers.alt_key() => {
                    let palette = state.palette.as_mut().expect("checked above");
                    palette.query.push(' ');
                    palette.selected = 0;
                }
                Key::Character(character) if !primary && !self.modifiers.alt_key() => {
                    let palette = state.palette.as_mut().expect("checked above");
                    palette.query.push_str(character);
                    palette.selected = 0;
                }
                _ => return,
            }
            state.window.request_redraw();
            return;
        }

        if matches!(event.logical_key, Key::Named(NamedKey::F5)) {
            if self.modifiers.shift_key() {
                state.stop_python();
            } else {
                state.run_python();
            }
            return;
        }
        if primary
            && matches!(&event.logical_key, Key::Character(character) if character.eq_ignore_ascii_case("r"))
        {
            state.run_python();
            return;
        }
        if primary
            && matches!(&event.logical_key, Key::Character(character) if character.eq_ignore_ascii_case("j"))
        {
            state.show_output = !state.show_output;
            state.resize_text_buffers();
            state.window.request_redraw();
            return;
        }

        match &event.logical_key {
            Key::Named(NamedKey::ArrowLeft) if primary => state.editor.move_word_left(),
            Key::Named(NamedKey::ArrowRight) if primary => state.editor.move_word_right(),
            Key::Named(NamedKey::Home) if primary => state.editor.move_to_document_start(),
            Key::Named(NamedKey::End) if primary => state.editor.move_to_document_end(),
            Key::Named(NamedKey::Backspace) => state.editor.backspace(),
            Key::Named(NamedKey::Delete) => state.editor.delete_forward(),
            Key::Named(NamedKey::ArrowLeft) => state.editor.move_left(),
            Key::Named(NamedKey::ArrowRight) => state.editor.move_right(),
            Key::Named(NamedKey::ArrowUp) => state.editor.move_up(),
            Key::Named(NamedKey::ArrowDown) => state.editor.move_down(),
            Key::Named(NamedKey::Home) => state.editor.move_to_line_start(),
            Key::Named(NamedKey::End) => state.editor.move_to_line_end(),
            Key::Named(NamedKey::Enter) => state.editor.insert("\n"),
            Key::Named(NamedKey::Tab) => state.editor.insert("    "),
            Key::Named(NamedKey::Space) if !primary && !self.modifiers.alt_key() => {
                state.editor.insert(" ");
            }
            Key::Character(character)
                if primary
                    && !self.modifiers.shift_key()
                    && character.eq_ignore_ascii_case("z") =>
            {
                state.editor.undo();
            }
            Key::Character(character)
                if primary
                    && (self.modifiers.shift_key() && character.eq_ignore_ascii_case("z")
                        || character.eq_ignore_ascii_case("y")) =>
            {
                state.editor.redo();
            }
            Key::Character(character) if primary && character.eq_ignore_ascii_case("s") => {
                if let Err(error) = state.editor.save() {
                    eprintln!("{PRODUCT_NAME}: {error}");
                }
            }
            Key::Character(character) if !primary && !self.modifiers.alt_key() => {
                state.editor.insert(character);
            }
            _ => return,
        }
        state.reset_blink();
        state.sync_syntax();
        state.reveal_cursor();
        state.update_title();
        state.window.request_redraw();
    }
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window_state.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(PRODUCT_TAGLINE)
            .with_inner_size(LogicalSize::new(1100.0, 720.0));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .expect("failed to create minamilist window"),
        );
        let editor = self.editor.take().expect("editor state initialized once");
        self.window_state = Some(pollster::block_on(WindowState::new(
            window,
            event_loop,
            editor,
            self.project_root.clone(),
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
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(state) = self.window_state.as_mut() {
                    state.pointer = position;
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(state) = self.window_state.as_mut() {
                    state.place_cursor_from_pointer();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(state) = self.window_state.as_mut() {
                    state.scroll(delta);
                }
            }
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
    output_buffer: TextBuffer,
    output: String,
    show_output: bool,
    output_scroll_line: usize,
    python_run: Option<PythonRun>,
    blink_started: Instant,
    scroll_line: usize,
    pointer: PhysicalPosition<f64>,
    rendered_cursor_byte: Option<usize>,
    window: Arc<Window>,
}

impl WindowState {
    async fn new(
        window: Arc<Window>,
        event_loop: &ActiveEventLoop,
        editor: EditorBuffer,
        project_root: PathBuf,
    ) -> Self {
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
        let mut output_buffer = TextBuffer::new(&mut font_system, Metrics::new(14.0, 20.0));
        output_buffer.set_size(Some(size.width.saturating_sub(32) as f32), Some(190.0));

        let syntax = editor
            .path()
            .and_then(Language::from_path)
            .and_then(|language| SyntaxDocument::new(language, editor.text()).ok());
        let project = ProjectIndex::scan(project_root).ok();
        let state = Self {
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
            output_buffer,
            output: "Python output will appear here.  Run with F5 or Ctrl+R.".into(),
            show_output: false,
            output_scroll_line: 0,
            python_run: None,
            blink_started: Instant::now(),
            scroll_line: 0,
            pointer: PhysicalPosition::new(0.0, 0.0),
            rendered_cursor_byte: None,
            window,
        };
        state.update_title();
        state
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.resize_text_buffers();
        self.window.request_redraw();
    }

    fn output_height(&self) -> u32 {
        if self.show_output {
            (self.surface_config.height * 35 / 100).clamp(140, 260)
        } else {
            0
        }
    }

    fn editor_bottom(&self) -> u32 {
        self.surface_config
            .height
            .saturating_sub(self.output_height())
    }

    fn resize_text_buffers(&mut self) {
        let width = self.surface_config.width.saturating_sub(32) as f32;
        self.text_buffer.set_size(
            Some(width),
            Some(self.editor_bottom().saturating_sub(24) as f32),
        );
        self.output_buffer.set_size(
            Some(width),
            Some(self.output_height().saturating_sub(32) as f32),
        );
    }

    fn reset_blink(&mut self) {
        self.blink_started = Instant::now();
    }

    fn reveal_cursor(&mut self) {
        let line = self.editor.line_column().0;
        let visible_lines = (self.editor_bottom() as usize / 24).max(1);
        if line < self.scroll_line {
            self.scroll_line = line;
        } else if line >= self.scroll_line + visible_lines {
            self.scroll_line = line + 1 - visible_lines;
        }
    }

    fn scroll(&mut self, delta: MouseScrollDelta) {
        if self.palette.is_some() {
            return;
        }
        let lines = match delta {
            MouseScrollDelta::LineDelta(_, y) => y.round() as isize,
            MouseScrollDelta::PixelDelta(position) => (position.y / 24.0).round() as isize,
        };
        if self.show_output && self.pointer.y >= f64::from(self.editor_bottom()) {
            self.output_scroll_line = self.output_scroll_line.saturating_add_signed(-lines);
        } else {
            self.scroll_line = self.scroll_line.saturating_add_signed(-lines);
        }
        self.window.request_redraw();
    }

    fn place_cursor_from_pointer(&mut self) {
        if self.palette.is_some()
            || self.pointer.x < 16.0
            || self.pointer.y < 16.0
            || self.pointer.y >= f64::from(self.editor_bottom())
        {
            return;
        }
        let Some(hit) = self
            .text_buffer
            .hit(self.pointer.x as f32 - 16.0, self.pointer.y as f32 - 16.0)
        else {
            return;
        };
        let source = self.editor.text();
        let mut display = source.clone();
        if let Some(cursor) = self.rendered_cursor_byte {
            display.insert(cursor.min(display.len()), '│');
        }
        let line_start = display
            .split_inclusive('\n')
            .take(hit.line)
            .map(str::len)
            .sum::<usize>();
        let mut byte = (line_start + hit.index).min(display.len());
        while byte > 0 && !display.is_char_boundary(byte) {
            byte -= 1;
        }
        if let Some(cursor) = self.rendered_cursor_byte
            && byte > cursor
        {
            byte = byte.saturating_sub('│'.len_utf8());
        }
        byte = byte.min(source.len());
        while byte > 0 && !source.is_char_boundary(byte) {
            byte -= 1;
        }
        self.editor.set_cursor(source[..byte].chars().count());
        self.reset_blink();
        self.update_title();
        self.window.request_redraw();
    }

    fn update_title(&self) {
        let file = self
            .editor
            .path()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("untitled");
        let dirty = if self.editor.is_dirty() { "● " } else { "" };
        let (line, column) = self.editor.line_column();
        let running = if self.python_run.is_some() {
            "  —  running Python"
        } else {
            ""
        };
        self.window.set_title(&format!(
            "{dirty}{file}  —  {}:{}{running}  —  {PRODUCT_NAME}",
            line + 1,
            column + 1
        ));
    }

    fn run_python(&mut self) {
        if self.python_run.is_some() {
            self.output
                .push_str("\nA Python process is already running.\n");
            self.show_output = true;
            self.resize_text_buffers();
            self.window.request_redraw();
            return;
        }
        let Some(path) = self.editor.path().map(PathBuf::from) else {
            self.output = "Cannot run an untitled buffer. Open or create a .py file first.".into();
            self.show_output = true;
            self.resize_text_buffers();
            self.window.request_redraw();
            return;
        };
        let is_python = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(extension.to_ascii_lowercase().as_str(), "py" | "pyw")
            });
        if !is_python {
            self.output = format!("{} is not a Python file.", path.display());
            self.show_output = true;
            self.resize_text_buffers();
            self.window.request_redraw();
            return;
        }
        if let Err(error) = self.editor.save() {
            self.output = format!("Could not save before running:\n{error}");
            self.show_output = true;
            self.resize_text_buffers();
            self.window.request_redraw();
            return;
        }
        let path = match python::canonical_python_path(&path) {
            Ok(path) => path,
            Err(error) => {
                self.output = format!("Could not resolve {}:\n{error}", path.display());
                self.show_output = true;
                self.resize_text_buffers();
                self.window.request_redraw();
                return;
            }
        };
        match PythonRun::start(&path) {
            Ok(run) => {
                self.output = format!("Running {}…", path.display());
                self.output_scroll_line = 0;
                self.python_run = Some(run);
            }
            Err(error) => {
                self.output = format!(
                    "Could not start Python:\n{error}\n\nInstall Python 3 and ensure python3 or python is on PATH."
                );
            }
        }
        self.show_output = true;
        self.resize_text_buffers();
        self.update_title();
        self.window.request_redraw();
    }

    fn stop_python(&mut self) {
        if let Some(run) = &self.python_run {
            run.stop();
            self.output.push_str("\nStopping…");
        } else {
            self.output = "No Python process is running.".into();
        }
        self.show_output = true;
        self.resize_text_buffers();
        self.window.request_redraw();
    }

    fn poll_python(&mut self) {
        let result = self.python_run.as_ref().and_then(PythonRun::try_finish);
        let Some(result) = result else {
            return;
        };
        let status = if result.stopped {
            "stopped".into()
        } else if let Some(code) = result.exit_code {
            format!("exited with code {code}")
        } else {
            "exited without a status code".into()
        };
        let mut output = format!(
            "Python ({}) {status} in {:.2?}\n",
            result.interpreter, result.elapsed
        );
        if !result.stdout.is_empty() {
            output.push_str("\nstdout\n──────\n");
            output.push_str(&result.stdout);
        }
        if !result.stderr.is_empty() {
            output.push_str("\nstderr\n──────\n");
            output.push_str(&result.stderr);
        }
        if result.stdout.is_empty() && result.stderr.is_empty() {
            output.push_str("\n(no output)\n");
        }
        self.output = output;
        self.output_scroll_line = 0;
        self.python_run = None;
        self.update_title();
    }

    fn sync_syntax(&mut self) {
        if let Some(syntax) = self.syntax.as_mut()
            && let Err(error) = syntax.update_source(&self.editor.text())
        {
            eprintln!("{PRODUCT_NAME} syntax: {error}");
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
                    Err(error) => eprintln!("{PRODUCT_NAME}: {error}"),
                }
            }
            PaletteKind::Commands => match command {
                Some(CommandId("file.save")) => {
                    if let Err(error) = self.editor.save() {
                        eprintln!("{PRODUCT_NAME}: {error}");
                    }
                }
                Some(CommandId("file.open" | "file.find")) => {
                    self.palette = Some(Palette::files());
                }
                Some(CommandId("palette.open")) => self.palette = Some(Palette::commands()),
                Some(CommandId("editor.undo")) => {
                    self.editor.undo();
                    self.sync_syntax();
                }
                Some(CommandId("editor.redo")) => {
                    self.editor.redo();
                    self.sync_syntax();
                }
                Some(CommandId("python.run")) => self.run_python(),
                Some(CommandId("python.stop")) => self.stop_python(),
                Some(CommandId("output.toggle")) => {
                    self.show_output = !self.show_output;
                    self.resize_text_buffers();
                }
                Some(CommandId("output.clear")) => self.output.clear(),
                _ => {}
            },
        }
        self.reset_blink();
        self.update_title();
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
        self.poll_python();
        let (display_text, cursor_byte) = self.display_text();
        self.rendered_cursor_byte = cursor_byte;
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
        let line = if self.palette.is_some() {
            0
        } else {
            self.scroll_line
        };
        self.text_buffer
            .set_scroll(glyphon::cosmic_text::Scroll::new(line, 0.0, 0.0));
        self.text_buffer
            .shape_until_scroll(&mut self.font_system, false);
        if self.show_output {
            let state = if self.python_run.is_some() {
                "RUNNING"
            } else {
                "OUTPUT"
            };
            let output = format!(
                "{state}    F5 run · Shift+F5 stop · Ctrl+J hide\n────────────────────────────────────────────────\n{}",
                self.output
            );
            self.output_buffer
                .set_text(&output, &default_attrs, Shaping::Advanced, None);
            self.output_buffer
                .set_scroll(glyphon::cosmic_text::Scroll::new(
                    self.output_scroll_line,
                    0.0,
                    0.0,
                ));
            self.output_buffer
                .shape_until_scroll(&mut self.font_system, false);
        }
        self.viewport.update(
            &self.queue,
            Resolution {
                width: self.surface_config.width,
                height: self.surface_config.height,
            },
        );
        let editor_bottom = self.editor_bottom() as i32;
        let mut areas = vec![TextArea {
            buffer: &self.text_buffer,
            left: 16.0,
            top: 16.0,
            scale: 1.0,
            bounds: TextBounds {
                left: 0,
                top: 0,
                right: self.surface_config.width as i32,
                bottom: editor_bottom,
            },
            default_color: Color::rgb(220, 225, 235),
            custom_glyphs: &[],
        }];
        if self.show_output {
            areas.push(TextArea {
                buffer: &self.output_buffer,
                left: 16.0,
                top: editor_bottom as f32 + 10.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: editor_bottom,
                    right: self.surface_config.width as i32,
                    bottom: self.surface_config.height as i32,
                },
                default_color: Color::rgb(170, 180, 198),
                custom_glyphs: &[],
            });
        }
        self.renderer
            .prepare(
                &self.device,
                &self.queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                areas,
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
                label: Some("minamilist frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("minamilist editor pass"),
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
