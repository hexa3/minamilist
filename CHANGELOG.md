# Changelog

All notable changes to Apex are recorded here during the build sprint.

## Unreleased

- Hour 0: initialized the open Rust workspace and editor architecture.

### Added

- Native wgpu text editor with rope-backed Unicode editing and file save.
- Incremental highlighting for seven day-one language variants.
- Project file finder, command palette, multi-buffer/split data model.
- LSP, DAP, PTY, and Git backend crates with smoke tests.
- Privacy-gated Anthropic provider, repository retrieval, and reviewed edits.
- Explicit, portable, journaled settings and migration-safe defaults.
- Cross-platform CI plus release-profile and large-buffer verification.

### Sprint retro (current checkpoint)

Shipped a buildable native foundation and isolated backend contracts. The
visible debugger, terminal, Git, AI review, settings, test, and API-client
panels were cut from this checkpoint because their end-to-end definitions of
done were not met. WASM plugins, VS Code compatibility, CRDT collaboration,
release packaging, and non-Linux runtime validation remain future work. No
milestone tags were created for incomplete phases.

