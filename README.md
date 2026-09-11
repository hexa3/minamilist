# Apex

Apex is an open, Rust-native, GPU-accelerated, AI-first IDE. The project is in
its initial implementation sprint; today the workspace contains the editor
core and the native UI bootstrap.

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

