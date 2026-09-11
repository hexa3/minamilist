# ADR 0002: wgpu and glyphon renderer

- Status: accepted
- Date: 2026-09-11

## Context

The editor needs a native, cross-platform GPU surface and high-quality Unicode
text without a browser runtime. Rebuilding font shaping correctly during the
sprint would add risk without differentiating Apex.

## Decision

Use winit for native events, wgpu for explicit GPU presentation, and glyphon
(cosmic-text plus a GPU glyph atlas) for shaping and rasterization. The editor
requests redraw at display vsync and keeps two frames in flight.

## Consequences

The input and render loop stay native and are positioned for 120 Hz displays.
Font shaping and atlas management remain replaceable behind `apex-ui`.
