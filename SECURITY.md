# Security policy

## Supported version

Security fixes are applied to the latest tagged release, currently 1.0.5.

## Reporting a vulnerability

Do not include exploit details in a public issue. Send a private report to the
repository owner with the affected version, operating system, reproduction
steps, impact, and any suggested mitigation. Expect an acknowledgement within
three business days and a status update within seven.

## Execution boundary

minamilist only executes a Python file after an explicit `F5` or `Ctrl+R`
command. Running a file grants it the same filesystem, network, and process
permissions as the editor. Review untrusted code before running it.

The integrated terminal also runs with the editor's full operating-system
permissions. Commands are never executed automatically; they require explicit
terminal input.

Remote code assistance remains disabled while local-only mode is active, and
reviewed multi-file proposals reject path traversal and stale source files.
