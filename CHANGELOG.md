# Changelog

This project follows semantic versioning. Dates use ISO 8601.

## [1.0.5] - 2026-09-15

### Fixed

- Report an error when saving an untitled buffer instead of silently
  discarding the request.
- Write files through a same-directory temporary before replacing the target.
- Resolve file and directory launch targets consistently.

### Added

- Run Python files asynchronously with `F5` or `Ctrl+R`.
- Capture stdout, stderr, duration, and exit status in a native output panel.
- Stop running Python processes with `Shift+F5`.
- Open not-yet-created file paths and save them on first use.
- Added keyboard and mouse-drag selection with native cut/copy/paste.
- Added native Open and Save As dialogs plus unsaved-change protection.
- Added in-document search with Unicode-safe result selection.

### Changed

- Renamed the shipped product and executable to `minamilist`.
- Added a compact document, cursor, and dirty-state window title.
- Added cross-platform primary-modifier shortcuts and word/document movement.
- Reworked the public documentation around the stable native editor surface.

## Pre-1.0 development

- Built the native window, GPU renderer, and Unicode rope editor.
- Added project search, the command palette, and incremental syntax support.
- Added LSP, DAP, pseudo-terminal, Git, settings, and reviewed-edit libraries.
- Added multi-cursor editing and bounded undo and redo history.
