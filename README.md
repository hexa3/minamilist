# minamilist

> the minamilist IDE

minamilist is a small, native code editor built in Rust. It keeps the writing
surface quiet while providing the essentials locally: fast Unicode editing,
syntax highlighting, project search, language tooling, a terminal, Git, and
optional reviewed code assistance.

Version 1.0.5 is the current stable release.

## Run it

You need Rust 1.88 or newer and the platform libraries required by `winit` and
`wgpu`. On Linux, that normally means Wayland or X11 development libraries and
a Vulkan-capable graphics stack.

```sh
cargo run --release -p apex-cli -- [FILE | DIRECTORY]
```

Opening a directory starts an untitled buffer with that directory as the
project root. Opening a file uses its parent directory. With no argument,
minamilist uses the current directory.

```text
the minamilist IDE

Usage: minamilist [OPTIONS] [FILE | DIRECTORY]

Options:
  -h, --help       Print help
  -V, --version    Print version
```

## Keys

| Key | Action |
| --- | --- |
| `Ctrl/Cmd+P` | Find a project file |
| `Ctrl/Cmd+Shift+P` | Open the command palette |
| `Ctrl/Cmd+O` | Find a project file |
| `Ctrl/Cmd+S` | Save the active file |
| `Ctrl/Cmd+Z` | Undo |
| `Ctrl/Cmd+Shift+Z` or `Ctrl+Y` | Redo |
| `Ctrl/Cmd+Left/Right` | Move by word |
| `Ctrl/Cmd+Home/End` | Move to document edge |
| `Escape` | Close a palette |

Untitled buffers deliberately require a path before they can be saved. The
native UI does not yet include a file chooser, so open a target file from the
command line when you intend to save it.

## Design

- Native windowing and GPU text rendering; no web view.
- Rope-backed, Unicode-safe editing and bounded undo history.
- Incremental Tree-sitter highlighting for Rust, Python, JavaScript,
  TypeScript, Go, C, and C++.
- Argument-safe Git, LSP, DAP, and pseudo-terminal libraries.
- Provider-neutral assistance with local-only policy enforcement and explicit
  review before applying multi-file changes.
- Human-readable settings that are only written after an explicit apply.

The crates remain separated by responsibility so the editor core is testable
without a windowing environment. The `apex-*` crate names are internal legacy
identifiers; the shipped product and executable are named `minamilist`.

## Verify

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release --workspace
```

minamilist is available under the [MIT license](LICENSE).
