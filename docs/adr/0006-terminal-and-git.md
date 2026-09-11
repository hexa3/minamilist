# ADR 0006: Native PTY and argument-safe Git integration

- Status: accepted
- Date: 2026-09-11

## Decision

Use operating-system pseudo terminals through `portable-pty`; terminal sessions
stream raw bytes so a later VT parser retains full control. Invoke the system
Git executable using structured arguments, never interpolated shell commands.

## Consequences

Interactive shells and native tools behave exactly as they do in a terminal.
Git honors the user's installed credential helpers and configuration while
paths and commit messages cannot become shell injection.
