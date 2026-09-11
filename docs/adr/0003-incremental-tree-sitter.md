# ADR 0003: Incremental Tree-sitter documents

- Status: accepted
- Date: 2026-09-11

## Decision

Keep parsing in a frontend-independent `apex-syntax` crate. Each document owns
its source, parser, and prior tree; edits describe byte and point deltas so
Tree-sitter can reuse unchanged subtrees. The first classifier supports Rust,
Python, JavaScript/TypeScript, Go, C, and C++.

## Consequences

Rendering, outline views, LSP features, and AI chunking can consume one syntax
model. Highlight classification begins deliberately small while the parsing
contract and supported grammars are real and incremental.
