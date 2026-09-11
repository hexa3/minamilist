# ADR 0007: Provider-neutral AI with mandatory review

- Status: accepted
- Date: 2026-09-11

## Decision

Separate model providers, repository retrieval, and edit orchestration. Network
policy is checked inside providers so local-only mode cannot be bypassed by a
caller. Agent output is a structured proposal; file edits require an explicit
accept decision and stale-content check before writing.

## Consequences

Anthropic is usable first while local and OpenAI providers can implement the
same trait. The lexical index can be populated with provider embeddings without
changing retrieval consumers. Per-hunk review and transactional rollback are
still required for the full Phase 4 experience.
