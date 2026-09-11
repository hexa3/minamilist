# ADR 0004: LSP over supervised subprocess JSON-RPC

- Status: accepted
- Date: 2026-09-11

## Decision

Run existing language servers as child processes and implement LSP's framed
JSON-RPC transport in `apex-lsp`. Requests are correlated on a dedicated reader
thread; notifications remain asynchronous. Apex validates rename workspace
edits for overlap before any caller may apply them.

## Consequences

rust-analyzer, Pyright/pylsp, and typescript-language-server can be consumed
without adapters. Server crashes are isolated from editor state. Applying file
edits and richer `documentChanges` support remain explicit next steps.
