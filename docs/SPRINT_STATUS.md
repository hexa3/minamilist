# Apex sprint status

This ledger is deliberately stricter than a feature checklist: a phase is only
complete when its stated end-to-end definition is demonstrable.

## Phase 1 — complete

- [x] Cargo workspace, CI, changelog, contribution guide, MIT license.
- [x] Native window and wgpu surface.
- [x] GPU glyph atlas/text renderer, typing, blinking cursor.
- [x] Optimized binary survives a live Wayland/GPU smoke run.
- [x] Tagged `v0.0.1-window-alive`.

## Phase 2 — partial

- [x] Rope file open/edit/save and large-buffer editing smoke budget.
- [x] Multi-cursor edit engine and multi-buffer/split-pane data model.
- [x] Incremental Tree-sitter parsing/highlighting for required languages.
- [x] Fuzzy file finder and command palette, both interactive.
- [ ] Rendered tab strip, split panes, and project tree sidebar.

## Phase 3 — backend complete, UI/end-to-end incomplete

- [x] LSP transport plus open/change, diagnostics channel, completion, hover,
  definition, and overlap-validated rename edits.
- [x] DAP transport plus launch/attach, breakpoints, stepping, stack, variables.
- [x] Real pseudo-terminal session backend.
- [x] Git status/diff/stage/commit/push backend.
- [ ] Connect real language/debug servers to editor panels and apply renames.
- [ ] Render terminal, debugger, diagnostics, and Git UI.

## Phase 4 — backend partial

- [x] Repository chunk/index pipeline, lexical retrieval, embedding storage.
- [x] Provider abstraction and Anthropic Messages API.
- [x] Hard local-only privacy enforcement.
- [x] Structured plan and multi-file proposal with explicit accept/reject,
  traversal rejection, and stale-file protection.
- [ ] Chat/inline-edit UI, per-hunk diff review, verification runner.
- [ ] Background worktree agent and live filesystem index updates.

## Phase 5 — partial

- [x] Human-readable settings, stable theme value, keymap presets.
- [x] No silent writes; explicit atomic apply plus auditable change journal.
- [ ] Theme rendering, REST/database panel, test runner, keymap wiring.

## Phase 6 — not started

- [ ] WASM plugin runtime and reference plugins.
- [ ] VS Code compatibility shim.
- [ ] CRDT collaboration.

## Phase 7 — partial

- [x] Linux release build and cross-platform CI definition.
- [x] Clone/build/run instructions and architecture map.
- [ ] Screenshots, packaged binaries, runtime validation on macOS/Windows.
- [ ] `v1.0.0-sprint-release` (intentionally withheld).

## Verification record

- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --release --workspace`
- Five-second native Wayland/wgpu launch smoke test opening `Cargo.toml`.
- Large rope: roughly 4.8 MB with 1,000 middle insert/delete pairs under the
  two-second cross-machine smoke threshold (about 50 ms on the build host).
