# MathOxide — Project Handoff Document (2026-10-18, evening)

## 1. Executive Summary & Status

* **Repository**: `/home/arjun/Projects/mathoxide` (git branch `main`)
* **Core Philosophy**: A pure-Rust, lightweight mathematical animation engine, visual IDE studio, and agentic media suite replacing Manim / ManimGL with zero Python runtime dependency, optimized to run smoothly on standard laptops with integrated graphics (~128 MB VRAM; 16 GB RAM — buffers/lists are fine, VRAM is the constraint).
* **Build & Test Health**: **68 workspace unit tests pass, zero compiler warnings** (mo-core: 35, mo-dsl: 17, mo-ide: 12, mo-moab: 2, mo-render: 2).
* This session delivered:
  1. Full **Manim Community (Manim CE) built-in object library**: complete 2D shapes (`arc`, `rounded_rect`, `ellipse`, `regular_polygon`, `triangle`, `annulus`, `sector`, `annular_sector`, `star`, `brace`, `elbow`), 1D lines (`dashed_line`, `tangent_line`, `double_arrow`, `curved_arrow`, `curved_double_arrow`), and 3D primitives (`cylinder`, `cone`, `line3d`, `dot3d`).
  2. **Coordinate systems & calculus tools**: `number_plane` (grid lines), `complex_plane`, `polar_plane`, `number_line`, `parametric` curves, `riemann` sums (left/right/midpoint), `secant` lines, `area` under curve, `barchart`.
  3. **Multi-font vector typography & dynamic math**: TrueType font selection (`font="ubuntu"`, `font="mono"`, `font="serif"`, `font="sans"`, or custom `.ttf`/`.otf` path), `decimal` counting mobject, `matrix` formatting (brackets `[`, `(`, `|`).
  4. **Expanded animation suite**: `drawBorderThenFill`, `uncreate` / `unwrite`, `circumscribe`, `flash`, `scale`, `shift`.
  5. **IDE & MCP integration**: Autocomplete, hover tooltips, timeline coloring, and syntax highlighting updated for all new primitives.

---

## 2. Project-Per-Video Model & Scripting Conventions

```
~/mathoxide/                      # SRC_HOME
├── <project>/master.moa + <project>/assets/*.moa
~/Videos/mathoxide/               # OUT_HOME renders
```

* One video = one folder. Imports resolve file-relative (`parse_file`, `parse_with_origins_in`).
* Renders default to OUT_HOME. Play provenance (`PlayOrigin`) powers cross-file timing edits.
* Session state persists in `~/.config/mathoxide/studio.toml` (`src_root`, `active_project`), restored on IDE launch; falls back to newest-master project.

### Asset & Master Scripting Convention (Standard Law)

1. **Asset Files (`assets/<name>.moa`)**:
   - **Local Origin Purity**: Assets are designed around local origin `(0, 0)`. When imported in master with `at (x, y)`, all internal geometries are cleanly shifted together.
   - **Declarative Primitives**: Declare shapes, labels, colors, and strokes (`circle`, `square`, `tex`, `text`, `arrow`, `dot`, `axes`).
   - **Syntax Tokens**: Strict `//` comments; `#RRGGBB` or `#RRGGBBAA` for hex colors (e.g. `fill=#58C4DD33, stroke=cyan, sw=3`).
   - Can include standalone animations for single-asset previewing, or remain purely declarative objects.

2. **Master Composition (`master.moa`)**:
   - **Top-of-File Imports**: All `import "assets/<name>.moa" as <alias> at (x, y);` statements **must stay at the very top of `master.moa`** (immediately below the header comments). The IDE automatically inserts any new or right-click imports at the top.
   - **First-Class Composite Asset Animations**: Imported assets are automatically registered as unified composite `VGroup`s under `<alias>`. You can animate the entire asset directly with full timing and easing controls:
     ```moa
     play showCreation(intro_card) 2.0s smooth;
     play write(intro_card) 1.5s smooth;
     play transform(intro_card, conclusion_card) 2.0s smooth;
     play fadeOut(intro_card) 1.0s;
     wait 0.5s;
     ```
   - **Timeline Orchestrator**: The master script controls the macro timeline, sequencing asset entrances, camera moves (`camera polar ...`), transitions, and pacing.

---

## 3. What Changed This Session (IDE overhaul & Manim Built-ins)

| Area | Before | Now |
|---|---|---|
| **Manim Built-in Geometry** | Basic circles, rects, lines, arrows | **Complete Manim CE geometric catalog**: `arc`, `rounded_rect` (corner radius `r`), `ellipse`, `regular_polygon` ($n$-sided), `triangle`, `annulus`, `sector`, `annular_sector`, `star`, `brace` ($\underbrace{\quad}$ curly bracket), `elbow` ($\llcorner$), `dashed_line` (dash & space len), `tangent_line`, `double_arrow` ($\longleftrightarrow$), `curved_arrow`, `curved_double_arrow`, `cylinder`, `cone`, `line3d`, `dot3d`. |
| **Coordinate Systems & Calculus** | Simple axes & basic function plot | **Full coordinate grids & calculus suite**: `number_plane` (grid lines with major/minor steps), `complex_plane` (real/imaginary axes), `polar_plane` (concentric rings & spokes), `number_line` (1D ticks), `parametric` curves ($(x(t), y(t))$), `riemann` sums (`left`, `right`, `mid`), `secant` lines, `area` under curve, `barchart` (discrete data visualization). |
| **Typography & Fonts** | Single hardcoded font (`UBUNTU_LIGHT`) | **Multi-font vector text engine**: TrueType selection via `font="ubuntu"`, `font="mono"` / `"hack"`, `font="serif"`, `font="sans"` / `"noto"`, or any custom `.ttf`/`.otf` path. Added dynamic `decimal` number display (`places=2`) and native `matrix` formatting (brackets `[`, `(`, or `\|`). |
| **Animation Classes** | Basic showCreation, fadeIn/Out, transform | **Expanded animation suite**: `drawBorderThenFill` (traces stroke boundary first then floods fill), `uncreate` / `unwrite` (retracts/erases curves), `circumscribe` (animated highlight loop/box), `flash` (radial burst of rays), `scale` (smooth scaling by factor), `shift` (smooth translation). |
| **Explorer** | Flat asset list in a tab, congested | **Permanent leftmost Explorer column**: active project's `master.moa` on top (click-only), `assets/` always expanded below, click opens, right-click → Open / Insert import / Rename (inline) / Delete (confirm dialog). Loose root .moa in own section. Project dropdown switches active project (opens its master). Auto-rescan every 2s — external (MCP/CLI) changes appear automatically. **Selected file styling**: Bold warm gold (`#FFE066`) on deep royal violet highlight pill; unselected files in soft lavender (`#DDD4F5`), eliminating electric cyan washout. |
| **Layout** | Tabbed left panel, resizable everything | **Fixed IDE layout**: Explorer (270px) → Editor (430px, h-scrolls, never wraps) → right column (Viewport over Inspector 300px) → Timeline dock (150px). Nothing resizable; app window 1280×800 non-resizable. Scene Tree panel removed. |
| **Editor** | Plain highlighting | **Autocomplete** (typing any letter pops matching builtins + user symbols — object ids and import aliases scanned live; ↑/↓/Enter/Tab/Esc, click-to-accept, caret lands after insertion), **hover docs** (0.6s stationary on any builtin → signature + description), **cyberpunk theme** (keywords #FF00A8, anims #F5FF00, types #C724FF, ids/easings #00FF9D, strings #00FFF7, numbers #FF6B35, comments #8A2BE2, operators #00D4FF; chrome: panels #0A002B, editor bg #05001A, gutter #4A0099). **Comments & Colors**: Comments strictly use `//` (not `#`); `#` is reserved for hex color literals (`#FFD166` warm gold tokens), preventing lines with `fill: #...` from turning entirely purple. |
| **Build & Cache** | Target cache ballooned to 24.5 GB from debug symbols | Reclaimed 24.5 GB via `cargo clean`. Added `[profile.dev] debug = 1` to `Cargo.toml` to suppress multi-gigabyte DWARF inspection tables while preserving accurate line-number stack traces. |
| **Focus & Caret** | Caret launched to EOF on errors due to ID shifts and focus reclaim | **Stable ID Scope & Caret Preservation**: Wrapped editor inside `ui.push_id("code_editor_fixed_scope")` with `id_salt("editor_scroll_area")` and `id_salt("code_editor_text_edit")`. The error bar appearing/disappearing no longer shifts TextEdit's ID. Added `EditorPanel::last_caret_char`; if egui tries to default caret to end-of-text on `request_focus()` / `gained_focus()`, the known caret is restored immediately. |
| **Imports in IDE** | `parse_with_origins` used CWD → `Could not find imported asset` | New `mo_dsl::parse_with_origins_in(src, base_dir)`; IDE compiles with the open file's folder (`compile_base_dir()`), falling back to active project then src_home. |
| **New Asset** | Auto-named `asset_N.moa`, orphaned into src_home with no project | Named inline (➕ by `assets/` or right-click header), created in **active project** only; auto-imports into its master.moa. No src_home fallback. |
| **Top-of-File Imports** | New imports were blindly appended to the bottom of `master.moa` | `insert_import_at_top` places new imports at the top of the file directly beneath header comments or after existing imports. |
| **Composite Asset Animation** | Imported assets could only be animated via individual sub-object IDs | The DSL automatically registers a composite `VGroup` under `<alias>` (shifted by `at (x, y)`). Master compositions can directly animate the entire imported asset using `play showCreation(alias)`, `play write(alias)`, `play transform(alias, to)`, `play fadeOut(alias)`. |

---

## 4. Workspace Crates & Architecture

```
mathoxide/
├── crates/
│   ├── mo-core                 # engine: VGroup, geom (surface_3d[_filled_with_stride]), math, camera, Player
│   ├── mo-render               # wgpu renderer; buf_cache in pipelines.rs draw_scene_pass
│   ├── mo-dsl                  # .moa parser: parse / parse_file / parse_with_origins[_in], PlayOrigin
│   ├── mo-moab                 # .moab container (postcard, versioned)
│   ├── mo-cli                  # `mo`: validate/render/compile/preview/inspect; -o optional
│   ├── mo-interactive          # winit preview window (camera anims not applied here yet)
│   ├── mo-ide                  # Studio (egui/eframe): app.rs (fixed layout, focus, shortcuts),
│   │                           #   assets.rs (Explorer + project model), editor.rs (highlight,
│   │                           #   autocomplete UI, hover docs), completion.rs (catalog, symbol
│   │                           #   scan, prefix-span helpers), timeline.rs, export.rs, watcher.rs
│   └── mo-mcp                  # JSON-RPC stdio server; timeline arrays; project-tree scanning
├── shaders/                    # vendored ManimGL WGSL (reference; embedded copies in mo-render)
└── .mcp.json                   # registers mathoxide-mcp for agents
```

`mo-ide/src/assets.rs` still owns `src_home()`, `out_home()`, `project_of()`, `newest_master_project()`, `save_config()`.

---

## 5. Key Mechanisms

* **16:9 letterboxed viewport** — camera.width/height synced on resize (no resize now; fixed window).
* **3D camera** — polar φ/θ/γ; orbit/pan/zoom; `Player::camera_at(tl, t, base)` wired into CLI + IDE (NOT `mo preview`).
* **Typst-powered LaTeX**, sequential subpath reveal; no synthetic stroke on glyphs ever.
* **Timeline sequencer** — drag/trim/`💾 Apply Timings to Code` via `apply_timings_multi` + `PlayOrigin` (cross-file rewrites).
* **GPU buffer cache** (`mo-render/src/pipelines.rs`) — content-hashed uploads; wireframe decimation; res clamp 400.
* **Explorer scan model** — `ProjectEntry { name, path, master, assets }` per folder under src_root; legacy flat `assets/`/`examples/` folders surface as pseudo-projects; root .moa files go to `loose`.
* **Autocomplete internals** — `completion.rs` is pure/logic-side (catalog, `user_symbols`, `prefix_before_caret`, `prefix_span_at`, `line_col_of`, `word_at_line_col`); editor.rs does egui UI. Accept path replaces the *byte span* of the identifier prefix ending at the caret and restores caret via `TextEditState::load/store` + `cursor.set_char_range`.

---

## 6. How to Run & Test

```bash
cargo test --workspace                                  # 61 tests, 0 warnings

cargo run -p mo-ide                                     # Studio (1280×800 fixed)

cargo run -p mo-cli -- render ~/mathoxide/master-showcase/master.moa
cargo run -p mo-cli -- render ~/mathoxide/master-showcase/master.moa --frame 4.5
cargo run -p mo-cli -- validate ~/mathoxide/cube_3d/master.moa

printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_assets","arguments":{}}}\n' \
  | cargo run -p mo-mcp
```

---

## 7. Engineering Gotchas (accumulated — keep)

1. **WGPU 24**: `Instance::new(&desc)` / `request_adapter` return Results; explicit stencil masks.
2. **Subpath separation** for multi-char text (winding-fill whitespace bug).
3. **No artificial stroke on fonts** (`sw=0`).
4. **Timeline drag borrows**: `mem::take(&mut self.timeline_panel)`; Wait insert shifts Play index.
5. **Apply-timings needs origins** — `origins.len() == run_times.len()`; re-parse fresh with `parse_with_origins` (or `_in`).
6. **iGPU budget**: res clamp 400, stride-decimated wireframes.
7. **egui 0.31 APIs**: `TextEditState::load/store(ctx, id)` (not `read`), `cursor.char_range()`/`set_char_range(CCursorRange{primary, secondary})`, `rect_stroke(rect, radius, stroke, StrokeKind)`, `WidgetVisuals` via named fields (no `values_mut`), `Label` needs `.sense(Sense::click())` for clicks, `ui.indent` is **vertical-layout-only** (panics inside `ui.horizontal`).
8. **Byte vs char indices**: egui cursors are char-based; `str::replace_range` is byte-based. Always go through `prefix_span_at` — the naive mix produced "extra chars" on accept.
9. **`ui.indent` / scanner panics on empty input**: guard `words.first()` in line scanners (blank lines hit `words[0]`).
10. **Panel scans**: keep the 2s auto-rescan in `AssetPanel::show`; never trust a stale cache after external writes.
11. **Focus contract**: editor reclaims focus only when *no* widget holds it (so inline rename / new-asset fields still work); global single-key shortcuts must check `editor_panel.focus_held`.
12. **Animated vs interactive camera** — viewport keeps a base camera; `camera_at` overrides during/after keyframed anims.
13. **Imported Asset Isolation**: Unplayed imported assets or sub-objects must NEVER be emitted as scene `Stmt::Decl`s; only alias-referenced composites or referenced sub-objects are lowered to `tl.stmts`. This prevents unplayed assets from appearing and freezing at $t = 0$.
14. **Live Dragging in Master vs Assets**: `apply_shift_to_dsl` supports `import "..." as <alias> [at (x, y)]`. Dragging an imported sub-object (e.g. `exp_title`) while `master.moa` is open identifies the origin asset file via `prefix_` and writes updated coordinates directly to the asset file on disk, immediately recompiling the master composition live.
15. **Timeline Timings Base Directory**: Always resolve imports with `parse_with_origins_in(&code, &self.compile_base_dir())`. Never call `parse_with_origins` without base dir in IDE workflows, or imports will fail relative to process CWD.
16. **FileWatcher Dirty Guard**: `reload_from_disk()` must only trigger when `!self.editor_panel.is_dirty` to prevent file system notify jitter from clobbering active user typing.
17. **Non-Exhaustive Animation Pattern Matching**: When adding any new `Animation` variant to `mo-core`, always update all five consumer sites:
    - `anim::sample` in `crates/mo-core/src/anim.rs`
    - `intro_times` and `state_at` match arms in `crates/mo-core/src/scene.rs`
    - Lowering and imported asset alias-renaming in `crates/mo-dsl/src/parser.rs`
    - Timeline block labels & color styling in `crates/mo-ide/src/timeline.rs`
    - JSON-RPC response `anim_label` in `crates/mo-mcp/src/main.rs`
18. **Multi-Font Zero-Dependency Fallback**: `mo_core::text::get_font_bytes` resolves fonts dynamically (checking system paths for `"serif"` / `"sans"` / custom paths), but always falls back to embedded TrueType byte arrays (`UBUNTU_LIGHT`, `HACK_REGULAR`, `NOTO_EMOJI_REGULAR`). The engine runs with 0 system font dependencies and never crashes when offline or on minimal OS installations.
19. **DSL Object Declaration Grammar**: MathOxide DSL uses whitespace-delimited `<kind> <id> <param>=<val> ...;` syntax (e.g. `arc a r=2.0 start=0.0 end=3.14;`), NOT `let x = kind(...);`. Object kinds and animation functions are lowercase/camelCase.

---

## 8. Direction & Roadmap — Next Up

1. **Playback smoothness for dense scenes**: `Player::state_at` clones every VGroup every frame — cache/incremental CPU evaluation is the remaining bottleneck (GPU side is cached).
2. **Directional lighting / normal-based shading** for filled `surface3d`.
3. **Camera anims in `mo preview`** (winit).
4. **Undo/redo** in the IDE (code edits, explorer rename/delete, apply-to-code round-trips).
5. **`.mcp.json` + agent loop hardening**: `assemble_scene` should use `parse_with_origins`; consider an MCP `apply_timings` tool.
6. **CLI `render` of a project folder** (`mo render ~/mathoxide/foo` → `foo/master.moa`).
7. **Polish candidates from this session**: hover docs for user symbols (object ids with their param list); go-to-def style click on import aliases; error squiggles per line instead of the top error bar.

### Instructions for Future Agents
* **Hardware**: 16 GB RAM (use freely), iGPU ~128 MB VRAM (keep per-frame GPU uploads bounded).
* **Scenes live outside the repo** in `~/mathoxide`. Never recreate `assets/`/`examples/` in-repo; tests build temp projects.
* Keep committing clean, self-contained milestones to `main`.
* **Arjun's preferences**: direct, mechanism-first, no fluff; assets edited one-at-a-time by design; MCP output states timings explicitly so AI never guesses overlaps; IDE layout is FIXED (don't reintroduce resizable panels or tab switching); palette is cyberpunk (no grays — use deep purples for neutrals).
