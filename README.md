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

New files do not need to exist yet:

```sh
cargo run --release -p apex-cli -- hello.py
```

Type your program, press `F5`, and minamilist saves it before running it with
`python3` (or `python` as a fallback). Standard output, errors, duration, and
the exit code appear in the output panel without blocking the editor.

To try the included example:

```sh
target/release/minamilist examples/hello.py
```

Press `Ctrl/Cmd+\`` to open the integrated shell in the project directory.
Type commands normally and press Enter; output streams into the terminal panel.
Press Escape to return keyboard focus to the editor without closing the shell.

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
| `Ctrl/Cmd+O` | Open a file with the native dialog |
| `Ctrl/Cmd+S` | Save the active file |
| `Ctrl/Cmd+Shift+S` | Save As with the native dialog |
| `Ctrl/Cmd+F` | Find text in the active file |
| `Ctrl/Cmd+A/C/X/V` | Select all, copy, cut, or paste |
| `Ctrl/Cmd+Z` | Undo |
| `Ctrl/Cmd+Shift+Z` or `Ctrl+Y` | Redo |
| `Ctrl/Cmd+Left/Right` | Move by word |
| `Ctrl/Cmd+Home/End` | Move to document edge |
| `F5` or `Ctrl/Cmd+R` | Save and run the active Python file |
| `Shift+F5` | Stop the active Python process |
| `Ctrl/Cmd+J` | Toggle the output panel |
| `Ctrl/Cmd+\`` | Toggle and focus the integrated terminal |
| `Escape` in terminal | Return focus to the editor |
| `Ctrl+C` in terminal | Interrupt the foreground command |
| `Ctrl/Cmd+Shift+C` in terminal | Copy the terminal transcript |
| `Escape` | Close a palette |

Untitled buffers open a native Save As dialog on their first save. Closing or
replacing a modified file prompts to save, discard, or cancel.

## Design

- Native windowing and GPU text rendering; no web view.
- Rope-backed, Unicode-safe editing and bounded undo history.
- Incremental Tree-sitter highlighting for Rust, Python, JavaScript,
  TypeScript, Go, C, and C++.
- Integrated native pseudo-terminal plus argument-safe Git, LSP, and DAP
  libraries.
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

For distribution and support requirements, see the
[production handoff](docs/PRODUCTION.md).
