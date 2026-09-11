# ADR 0005: Built-in Debug Adapter Protocol client

- Status: accepted
- Date: 2026-09-11

## Decision

Implement DAP as a core subprocess protocol client rather than a plugin. The
client exposes launch/attach, breakpoints, stepping, stack frames, variables,
and asynchronous debug events. Adapter failures remain outside the editor
process.

## Consequences

Python debugpy and CodeLLDB can share the same UI-facing API. Adapter discovery,
run configurations, and the debugger panel are still required for end-to-end
Phase 3 completion.
