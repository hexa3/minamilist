# Apex

Apex is an open, Rust-native, GPU-accelerated, AI-first IDE. The current build
is an early but real native editor foundation—not a completed 1.0 IDE.

## What works now

- Native winit/wgpu window and glyphon text rendering (no Electron or DOM).
- Unicode-safe rope buffer, multiple cursors, open/save, and a large-file test.
- Incremental Tree-sitter parsing and visible highlighting for Rust, Python,
  JavaScript/TypeScript, Go, C, and C++.
- `Ctrl+P` fuzzy project file finder and `Ctrl+Shift+P` command palette.
- Backend crates for LSP, DAP, real PTYs, and argument-safe Git operations.
- Provider-neutral AI with Anthropic support, repository retrieval, reviewed
  multi-file proposals, and a hard local-only policy.
- Portable TOML settings that are only persisted explicitly and journal every
  change.

The detailed completion ledger is in [docs/SPRINT_STATUS.md](docs/SPRINT_STATUS.md).

## Build

Requirements: a current stable Rust toolchain and Linux development libraries
for Wayland/X11 and Vulkan.

```sh
cargo run -p apex-cli -- [optional-file]
```

Run all checks with:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Principles

- Native performance: no browser engine or DOM.
- AI is a core subsystem with explicit review before edits are applied.
- Useful without plugins: language tooling, terminal, Git, debugging, tests.
- Trustworthy defaults: settings are portable and never silently rewritten.
- Open and forkable under the MIT license.

## Keyboard controls

| Key | Action |
| --- | --- |
| `Ctrl+P` | Find and open a project file |
| `Ctrl+Shift+P` | Open the command palette |
| `Ctrl+S` | Save the active file |
| `Left` / `Right` | Move the primary cursor |
| `Escape` | Close a palette |

## Workspace map

| Crate | Responsibility |
| --- | --- |
| `apex-core` | Rope buffers, cursors, workspaces, commands, project index |
| `apex-ui` | Native events, GPU surface, glyph atlas, interactive palettes |
| `apex-syntax` | Incremental Tree-sitter documents and highlights |
| `apex-lsp` | LSP JSON-RPC client and safe rename validation |
| `apex-dap` | Debug Adapter Protocol client |
| `apex-term` | Cross-platform native pseudo-terminal sessions |
| `apex-git` | Status, diff, stage, commit, and push |
| `apex-ai` | Providers, repository index, plan/review/apply orchestration |
| `apex-settings` | Portable explicit-consent configuration |
| `apex-cli` | Native executable bootstrap |
