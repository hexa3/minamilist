# Implementation status

Version 1.0.5 ships a focused native editing surface. This document separates
features visible in the application from libraries that are ready for later UI
integration.

## Available in the editor

- Native `winit` window and `wgpu`/glyphon text rendering.
- Unicode-safe rope editing, multi-cursor operations, and bounded history.
- File open, safe replacement saves, project search, and command search.
- Incremental syntax highlighting for Rust, Python, JavaScript, TypeScript,
  Go, C, and C++.
- Keyboard and pointer cursor placement, word/document navigation, and
  vertical scrolling.
- File, dirty-state, and cursor position in the window title.
- Non-blocking Python execution with stop control and a captured output panel.
- Integrated project-root shell with interactive input and streaming output.
- Mouse and keyboard text selection, native clipboard operations, file dialogs,
  in-document search, and unsaved-change confirmation.

## Available as tested libraries

- Language Server Protocol and Debug Adapter Protocol subprocess clients.
- Native pseudo-terminal sessions and argument-safe Git operations.
- Portable, explicit settings storage with a change journal.
- Repository retrieval and reviewed multi-file proposals with traversal and
  stale-file protection.

## Known limits

- Source control, debugger, language tooling, and reviewed-edit libraries do
  not yet have integrated panels. Python output and the terminal are integrated.
- The editor does not yet provide tabs or split-pane rendering.
- Release binaries are not currently packaged; build with Cargo.

## Verification

The release gate runs formatting, workspace tests, Clippy with warnings denied,
and an optimized workspace build. The core suite includes a roughly 4.8 MB rope
edit budget test.
