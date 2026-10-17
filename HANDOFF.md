# MathOxide — Handoff (2026-10-17)

## State

Rust rewrite of ManimGL at `/home/arjun/Projects/mathoxide` (git, branch `main`). 26 tests green, zero warnings. ManimGL source reference: `/home/arjun/Downloads/manim-master`; its WGSL shaders vendored at `shaders/` (fill.wgsl, stroke.wgsl, inserts/) with LICENSE.manim.md.

## CRITICAL — Reopened Phase 2 (Rendering Fidelity Pass): COMPLETED

Phase 2 visual fidelity pass is complete. ManimGL rendering techniques ported and verified:
1. **Analytic Antialiasing & Stencil Fill**:
   - `build.rs` compiles `fill.wgsl`, `stroke.wgsl`, `border.wgsl`, `present.wgsl`, `present_srgb.wgsl` into `$OUT_DIR`.
   - ManimGL 3-pass stencil fill:
     - Pass 1: `fill_count_pipeline` (`WINDING_COUNT`) increments front / decrements back faces into stencil buffer without writing color.
     - Pass 2: `fill_border_pipeline` (`FILL_BORDER`) renders the boundary curve where stencil == 0 with sub-pixel analytical anti-aliasing.
     - Pass 3: `fill_cover_pipeline` (`WINDING_COVER`) shades regions where stencil != 0, clearing stencil to 0.
2. **GPU Procedural Stroke**:
   - Fully procedural stroke pipeline in GPU via `stroke.wgsl` with analytical signed distance AA, curvature subdivision, and rounded/miter joins.
3. **Camera & Projection Matrix**:
   - Fixed camera uniform model to match ManimGL: `view = Mat4::from_translation(-self.center)`, `frame_rescale_factors = [2/(h*aspect), 2/h, 1/focal_distance]`, $w=1.0$ at $z=0$.
4. **Color & Gamma**:
   - Manim sRGB colors preserved without blowout/washing: non-sRGB surface format preferred or converted with `srgb_to_linear`.
5. **Dynamic Window Resizing**:
   - Renderer resizes depth-stencil and frame textures dynamically on window resize events; no stretching or distortion.
6. **Partial Curve & ShowCreation**:
   - Fixed integer truncation in `vgroup::partial()`.
   - ShowCreation keeps fill opacity at 0 until path animation completes, preventing closing-chord wedge artifacts.

## Workspace

- `crates/mo-core` — VGroup (quadratic bezier VMobject layout: per subpath anchor,handle,anchor,... anchor = 2n+1 records; curve i = records 2i,2i+1,2i+2), geom (circle/rect/line), anims (enum: Transform, FadeIn, FadeOut, ShowCreation, Grow, Rotate; rate as String name), scene (Stmt enum, Timeline, `Player::state_at(t)` — stateless, powers scrubbing), camera (FrameUniforms std140), rate (rate_functions.py ported verbatim), color, text (stub).
- `crates/mo-render` — Renderer (offscreen/new, device()/queue() accessors), ManimGL 3-pass stencil fill + GPU procedural stroke + present pipelines, `draw_scene_pass(state, cam, encoder)` reused by interactive, `render()`, `present_to()`, `set_present_format()`, `resize()`.
- `crates/mo-dsl` — hand-rolled lexer + recursive descent parser (no winnow; winnow 0.7 churn killed it). 5 tests. Error recovery via sync().
- `crates/mo-moab` — magic "MOAB" + u32 version + postcard Bundle {timeline, final_state, meta}. Round-trip verified.
- `crates/mo-cli` — `mo validate|render|preview|compile|inspect` (mp4 via ffmpeg rawvideo pipe).
- `crates/mo-interactive` — winit 0.30 App (see `src/lib.rs`): playback (space/arrows/home), pan (drag empty), zoom (wheel), mobject drag via bounds hit-test + Overrides persistence. Wayland lessons: **must call `frame.present()`**; graceful skip when acquire times out under occlusion; dynamic resize.
- `crates/mo-ide` — `mathoxide` binary: full egui IDE with shared WGPU instance, live editor with syntax highlighting and diagnostics, scene tree, inspector with overrides, timeline scrubber with sequencer track lanes, native viewport with pan/zoom/drag selection gizmo, hot reload via `notify`, and `.moab`/`.mp4` export.
- `crates/mo-mcp` — Phase 5 (MCP server).

## Phase 3: egui IDE — COMPLETED

1. **Shared WGPU Instance & Zero-Copy Viewport**:
   - `eframe` (wgpu backend, 0.31, Wayland/X11 native).
   - Viewport binds directly to `Renderer::target_view()` via `egui_wgpu::Renderer::register_native_texture` and updates on panel resize via `update_egui_texture_from_wgpu_texture`.
   - Full ManimGL 3-pass stencil fill and GPU procedural stroke rendering in real-time.
2. **Editor Panel (`editor.rs`)**:
   - Custom syntax highlighting for `.moa` DSL (primitives, animations, rates, hex colors, comments, keywords).
   - Line number gutter with synchronized scrolling.
   - Error diagnostics banner with line/col feedback from `mo-dsl`.
   - Quick compile and Ctrl+S saving.
3. **Scene Tree Panel (`scene_tree.rs`)**:
   - Hierarchy of mobjects at current scrub timestamp $t$.
   - Live visibility indicator, point counts, and bounding box dimensions.
   - Clicking selects mobject across the scene tree, inspector, and viewport.
4. **Inspector Panel (`inspector.rs`)**:
   - Live editable coordinates $(X, Y)$ via `DragValue` with real-time override shifts.
   - Color and stroke swatches with opacity values.
   - Geometry stats (points, subpaths, min/max bounds).
   - "Copy Shift to Clipboard" and "Reset Shift" buttons.
   - Scene metadata overview when deselected.
5. **Timeline Scrubber (`timeline.rs`)**:
   - Transport controls (Play/Pause, Step 1 Frame, Home, Loop, Speed 0.25x-2x).
   - Scrub slider spanning $[0.0, T]$.
   - Visual animation tracks (sequencer lanes) mapping individual animation blocks over time.
   - Real-time time readout (`01.25s / 04.50s`) and frame counter (`Frame 75 / 270 @ 60 FPS`).
6. **Direct Manipulation Viewport (`viewport.rs`)**:
   - Middle/Alt+Left drag pan, wheel zoom, 'R' reset camera.
   - Left-click hit-testing for mobjects; drag to translate in scene coordinates.
   - Selection bounding box gizmo with corner handles rendered on top of viewport.
   - HUD overlays: resolution, scene name, FPS, cursor coordinates $(x, y)$.
7. **Hot Reloading (`watcher.rs`)**:
   - Background file watcher with `notify` and debouncing.
8. **Vector Glyph Text (`mo-core::text`)**:
   - Real vector glyph outlines extracted via `ttf-parser` using embedded TrueType fonts (`epaint_default_fonts::UBUNTU_LIGHT`).
   - True scalable vector fonts with sub-pixel analytic anti-aliasing and counter hole stencil cutting.

## Phases remaining

- **Phase 4**: .moab compile/open/edit/re-export inside IDE.
- **Phase 5**: MCP server (rmcp, stdio): validate_moa, compile_moab, render_frame, inspect_moab, run_scene_headless, list primitives/easings. Skills + CLAUDE.md tooling. Later: LaTeX (latex+dvisvgm), func_plot.

## Gotchas (learned the hard way)

- wgpu 24 API: `Instance::new(&desc)`, `request_adapter` → Result, `queue.submit(Some(...))`, no `texture_format()` on views, stencil read/write_mask fields required.
- VGroup `start_subpath` pushes ONE record (not 2). `partial()` uses manim's exact `partial_quadratic_bezier_points` (de Casteljau).
- Circle arcs: control at `r/cos(dt/2)` on tangent intersection so curve stays inside radius.
- Serde: VmPoint/Animation/TimedAnim have derives; rate stored as String (fn pointer not serializable). glam needs "serde" + "bytemuck" features.
- Ortho vertex shader maps scene→clip via frame_rescale_factors, z clamped `pos.z*0.1`.
- Backface culling must stay off (fill fan triangles wind both ways).

## User prefs (Arjun)

Direct, mechanism-first, no fluff. Hard requirements: interactive window, IDE, agent tooling (MCP/skills), editable .moab. Locked: custom .moa DSL, egui IDE, core-engine-first phasing. Project memory: `/home/arjun/.claude/projects/-home-arjun/memory/project_mathoxide.md`.
