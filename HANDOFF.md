# MathOxide — Project Handoff Document (2026-10-18)

## 1. Executive Summary & Status

* **Repository**: `/home/arjun/Projects/mathoxide` (git branch `main`)
* **Core Philosophy**: A pure-Rust, lightweight mathematical animation engine, visual IDE studio, and agentic media suite replacing Manim / ManimGL with zero Python runtime dependency, optimized to run smoothly on standard laptops with integrated graphics (~128 MB VRAM; 16 GB RAM — buffers/lists are fine, VRAM is the constraint).
* **Build & Test Health**: **59 workspace unit tests pass, zero compiler warnings.**
  ```
  mo-core:        35 passed  (incl. viridis ramp, camera_at interpolation)
  mo-dsl:         14 passed  (incl. camera anims, import provenance, parse_file)
  mo-ide:          7 passed  (incl. apply-timings round-trips, panel scan layouts)
  mo-moab:         2 passed
  mo-render:       2 passed
  ```
* **Latest commits on `main`** (2026-10-18):
  - `e021156`: `fix: asset panel scans every folder layout + Open File button`
  - `c986f4d`: `perf: iGPU-friendly high-res surfaces — buffer reuse, wireframe decimation, res clamp`
  - `cdac4de`: `feat: project-per-video workflow — ~/mathoxide/<project>/{master.moa,assets/}`
  - `d969921`: `feat: file-relative import resolution — parse_file()`
  - `2d3a111`/`7b14607`/`b80e8d1`/`931aac6`: MCP timelines, cross-file timeline editing, bold-graph fix

---

## 2. The Project-Per-Video Model (BIGGEST CHANGE — read first)

The old repo-level `assets/` + `examples/` layout is **gone**. Everything migrated to:

```
~/mathoxide/                      # SRC_HOME (source of truth for videos)
├── master-showcase/
│   ├── master.moa                # was examples/master_video.moa
│   └── assets/{euler_formula,sine_wave}.moa
├── demo_3d/  latex_showcase/  demo/          # self-contained masters
├── euler_formula/  sine_wave/  gaussian_curve/  cube_3d/  camera_motion/  shaded_surface/
│                                 # each former asset -> own project
~/Videos/mathoxide/               # OUT_HOME (renders)
└── <project>.mp4 | .png | .moab  # CLI + IDE export defaults
```

* **One video = one folder**: `~/mathoxide/<project>/master.moa` + `<project>/assets/*.moa`.
* **Imports resolve file-relative**: `mo_dsl::parse_file(path)` lowers with `base_dir = file.parent()`; `import "assets/x.moa"` works from any CWD. `parse(src)` (string API) keeps CWD resolution for tests/compat.
* **Renders default to OUT_HOME**: CLI `-o` is optional (`~/Videos/mathoxide/<project>.<ext>`, project folder name used when file is `master.moa`); IDE export dialogs prefill OUT_HOME + `<project>.mp4`/`.moab`.
* **Creating is one click**: IDE Videos panel → type name → `🎬 New Video` makes the folder, master, and `assets/`. `➕ New Asset` creates in the *active project's* assets/ **and auto-appends the import line into master.moa**.
* Helpers live in `crates/mo-ide/src/assets.rs`: `src_home()`, `out_home()`, `project_of(path, src_root)`.

### Workflow invariants (do not regress)
* One asset file open at a time in the IDE is **intentional** (fast hot reload, small edits per clip).
* Play provenance: `mo_dsl::PlayOrigin { file: Option<PathBuf>, local_play_idx: usize }` — recorded per play in flat timeline order by the import lowerer (`parser.rs`, `lower(raws, base_dir)`). Powers cross-file timing rewrites.

---

## 3. What Changed Since the Previous Handoff (2026-10-17 → 10-18)

| Area | Before | Now |
|---|---|---|
| **Bold graphs** | `plot` default `sw=4.0` (2× axes weight); assets had explicit `sw=3.5/4` | Plot default **2.0**, asset files matched to axes weight. Plots read light like manim. |
| **Timeline × imports** | `💾 Apply Timings to Code` **refused** any scene with `import` | Cross-file apply: `apply_timings_multi(master, origins, run_times)` rewrites master's own `Ns` positionally **and each imported asset file's durations in place** (timeline.rs). Timeline blocks show asset labels (`euler: write(euler_id)`). |
| **MCP timing knowledge** | `list_assets`/`read_asset` only had totals | Both return a `timeline` array: per-play `start`/`end`/`run_time`/`anim` — agents know exact overlaps instead of guessing. `list_assets` walks the whole project tree by default (`~/mathoxide`). |
| **Project workflow** | Masters in `examples/`, assets in `assets/`, CWD-relative imports, hardcoded import template | Project-per-video model above; file-relative imports; New Video / New Asset / Open File all in-IDE; startup opens most recently edited `master.moa`. |
| **High-res surfaces** | `res=300` filled surface3d → ~540k curves; per-frame GPU buffer allocation per object; lag + shutdown | Content-hashed **GPU buffer cache** (upload once for static geometry); **wireframe decimation** (`surface_3d_filled_with_stride`, auto ~20k wire curves, DSL `wire_stride=N`); `res` clamped to 400/axis. res=300 renders cleanly. |
| **Parser fix** | `rotate(c, 1.5708)` failed (animation args only accepted identifiers) | Numeric args accepted as strings; `rotate` lowers with explicit angle. |

---

## 4. Workspace Crates & Architecture

```
mathoxide/                       # code only now — no scene files
├── crates/
│   ├── mo-core                 # engine: VGroup, geom (surface_3d[_filled_with_stride]), math, camera, Player
│   ├── mo-render               # wgpu renderer; buf_cache in pipelines.rs draw_scene_pass
│   ├── mo-dsl                  # .moa parser: parse / parse_file / parse_with_origins, PlayOrigin
│   ├── mo-moab                 # .moab container (postcard, versioned)
│   ├── mo-cli                  # `mo`: validate/render/compile/preview/inspect; parse_file everywhere; -o optional
│   ├── mo-interactive          # winit preview window (NOTE: camera anims not applied here yet)
│   ├── mo-ide                  # MathOxide Studio (egui/eframe): assets.rs has src_home/out_home
│   └── mo-mcp                  # JSON-RPC stdio server; timeline arrays; project-tree scanning
├── shaders/                    # vendored ManimGL WGSL (reference; embedded copies in mo-render)
└── .mcp.json                   # registers mathoxide-mcp for agents
```

---

## 5. Key Mechanisms (cumulative, still true)

* **16:9 letterboxed viewport** — canvas = viewport aspect-clamped to 16:9, `camera.width/height` synced; hit-testing and exports match 1:1.
* **3D camera** — polar `φ`, azimuth `θ`, roll `γ`; right-click drag orbits, middle/shift pans, wheel zooms, `R` resets. Primitives: `axes3d`, `cube`, `sphere`, `surface3d` (filled = Viridis height-ramped cells + strided wireframe).
* **Camera keyframes in DSL** — `play camera.orbit(phi=60, theta=-45) 2s smooth;` etc. `Player::camera_at(tl, t, base)` is stateless; wired into CLI + IDE viewport (NOT `mo preview` — known gap).
* **Typst-powered LaTeX**, sequential subpath reveal for fonts (no synthetic stroke on glyphs, ever).
* **Interactive timeline sequencer** — body drag trades time with preceding Wait (auto-inserted), right-edge trims run_time, `💾 Apply Timings to Code` rewrites master + asset files via `PlayOrigin`.
* **GPU buffer cache** (`mo-render/src/pipelines.rs`) — `buf_cache: RefCell<HashMap<u64, BufCacheEntry>>`; key = hash(object id, record len, record bytes, mobject uniforms); >64 entries → clear. BindGroups keep backing buffers alive. Animated objects hash differently per frame and re-upload.
* **Native file dialogs (rfd)** — Open/Save/Save As/Exports; asset panel has its own 📄 Open File and 📁 Browse (switches source root from inside the IDE).
* **MCP tools** — `list_assets` (project-tree walk + timelines), `read_asset` (+ timeline), `write_asset`, `validate_code`, `render_frame`, `assemble_scene`.

---

## 6. How to Run & Test

```bash
cargo test --workspace                                  # 59 tests, 0 warnings

cargo run -p mo-ide                                     # Studio (opens newest master.moa)

# CLI: -o optional, defaults to ~/Videos/mathoxide/<project>.<ext>
cargo run -p mo-cli -- render ~/mathoxide/master-showcase/master.moa
cargo run -p mo-cli -- render ~/mathoxide/master-showcase/master.moa --frame 4.5   # PNG
cargo run -p mo-cli -- validate ~/mathoxide/cube_3d/master.moa

# MCP (list walks ~/mathoxide, returns per-play timelines)
printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_assets","arguments":{}}}\n' \
  | cargo run -p mo-mcp
```

---

## 7. Engineering Gotchas (accumulated — keep)

1. **WGPU 24**: `Instance::new(&desc)` / `request_adapter` return Results; `queue.submit(Some(enc.finish()))`; explicit stencil `read_mask`/`write_mask`.
2. **Subpath separation**: multi-char text must keep per-character `SubPath` records — merging bridges whitespace in winding fill.
3. **Camera uniform rescaling**: update `camera.width/height` on any resize (letterbox math in `mo-ide/src/viewport.rs`).
4. **IDE window fits 1280×760** (14" laptop target).
5. **No artificial stroke on fonts** — TrueType outlines are fill-only (`sw=0`).
6. **Timeline drag borrows**: `mem::take(&mut self.timeline_panel)` in app.rs; inserting a Wait shifts the Play index — increment after insert.
7. **Apply-timings is provenance-driven now** — `apply_timings_multi` needs `origins.len() == run_times.len()`; caller re-parses with `parse_with_origins` fresh. Never rewrite positionally across files without origins.
8. **Animated vs interactive camera** — viewport keeps a base camera; `camera_at` result overrides during/after keyframed anims; never hold a borrow across input handling.
9. **iGPU budget**: `res` on surface3d is samples/axis → res² cells × ~4 curves. Clamp 400. Wireframe on filled surfaces is stride-decimated; full wireframe (`fill` omitted) is intentionally the surface itself.
10. **Regex-massaged code in app.rs**: some `let (tl, origins, err) = compile_code(...)` lines have odd indentation from scripted edits — harmless, fix opportunistically.

---

## 8. Direction & Roadmap — Next Up

Priority queue for the incoming agent:

1. **Playback smoothness for dense scenes**: `Player::state_at` clones every VGroup every frame. Cache evaluated states keyed by (timeline version, t) or evaluate incrementally — the GPU side is now cached, CPU-side scrubbing at res≥200 is the remaining bottleneck.
2. **Directional lighting / normal-based shading** for filled `surface3d` (flat shading today; normals per cell would read better at high res).
3. **Camera anims in `mo preview`** (winit) — only CLI + IDE apply `camera_at` today.
4. **Undo/redo** in the IDE (code edits, drags, apply-to-code round-trips).
5. **`.mcp.json` + agent loop hardening**: `assemble_scene` should use `parse_with_origins` so generated masters carry correct timing structure; consider an MCP `apply_timings` tool mirroring the IDE's cross-file rewrite.
6. **CLI `render` of a project folder** (e.g. `mo render ~/mathoxide/foo`) — resolve to `foo/master.moa` automatically.

### Instructions for Future Agents
* **Hardware**: 16 GB RAM (use it freely — caches, big Vecs), but iGPU ~128 MB VRAM: keep per-frame GPU uploads bounded, no heavy engines/compute shaders.
* **Scenes live outside the repo** in `~/mathoxide`. Never recreate `assets/`/`examples/` in-repo; tests build temp projects instead.
* Keep committing clean, self-contained milestones to `main`.
* **Arjun's preferences**: direct, mechanism-first, no fluff; assets edited one-at-a-time by design; MCP output should state timings explicitly so AI never guesses overlaps.
