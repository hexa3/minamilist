# ADR 0008: Settings never change without consent

- Status: accepted
- Date: 2026-09-11

## Decision

Settings are portable TOML. Missing keys receive in-memory defaults, but Apex
does not rewrite the file during load or upgrade. Only an explicit `apply`
persists a snapshot, atomically, with a reason appended to a sibling journal.
Network AI defaults to local-only.

## Consequences

Defaults can improve without mutating user state, and every persisted default
or migration is attributable and reversible from version control or backups.
