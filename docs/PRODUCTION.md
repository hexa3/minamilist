# Production handoff

minamilist 1.0.5 is suitable for local Python editing and execution. Before
distributing it as a supported desktop product, complete this checklist.

## Release gate

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release --workspace
```

Manually verify on every supported operating system:

- Open, edit, Save As, close-with-unsaved-changes, and reopen a file.
- Keyboard and mouse selection plus copy, cut, and paste with native apps.
- Python success, syntax error, non-zero exit, long output, and stop behavior.
- Project search, document search, Unicode input, resizing, and GPU recovery.

## Distribution work still required

- Choose permanent bundle identifiers and repository/support URLs.
- Add application icons, platform metadata, installers, and uninstall paths.
- Sign Windows and macOS builds and notarize the macOS application.
- Publish checksums and a software bill of materials with each binary.
- Add automated dependency vulnerability and license scanning.
- Test accessibility APIs, input method editors, high-DPI displays, and screen
  readers on real hardware.
- Add crash recovery before positioning the editor for critical unsaved work.
- Decide the supported Python versions and add virtual-environment selection.

The cross-platform CI workflow verifies source quality and tests, but it does
not replace runtime acceptance testing or code signing.

## Python security model

Running a file is an explicit action and uses the editor's operating-system
permissions. minamilist does not sandbox Python. Treat downloaded projects as
untrusted until their code and dependencies have been reviewed.
