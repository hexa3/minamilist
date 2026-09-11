# ADR 0001: Native Rust workspace

- Status: accepted
- Date: 2026-09-11

## Context

Apex needs predictable latency, direct GPU access, sandboxable components, and
a core that remains useful outside any one frontend.

## Decision

Use a Cargo workspace. Keep editing state and commands in `apex-core`, native
GPU presentation in `apex-ui`, and process/bootstrap concerns in `apex-cli`.
The UI uses wgpu directly and does not embed a browser engine.

## Consequences

Subsystem boundaries are explicit and independently testable. Native graphics
bring platform integration work but avoid DOM and garbage-collection latency.

