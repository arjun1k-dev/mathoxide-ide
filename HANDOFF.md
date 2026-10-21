# MathOxide — Project Handoff Document (2026-10-18, evening)

## 1. Executive Summary & Status

* **Repository**: `/home/arjun/Projects/mathoxide` (git branch `main`)
* **Core Philosophy**: A pure-Rust, lightweight mathematical animation engine, visual IDE studio, and agentic media suite replacing Manim / ManimGL with zero Python runtime dependency, optimized to run smoothly on standard laptops with integrated graphics (~128 MB VRAM; 16 GB RAM — buffers/lists are fine, VRAM is the constraint).
* **Build & Test Health**: **77 workspace unit tests pass, zero compiler warnings** (mo-core: 36, mo-dsl: 17, mo-ide: 20, mo-moab: 2, mo-render: 2).
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
* **GPU buffer cache + staging ring** (`mo-render/src/pipelines.rs`) — content-hashed over raw `points` bytes + subpaths + uniforms (`group_content_hash`); records are only built on cache miss; budget-based eviction (512 entries / 64 MB — kept lean for the 128 MB iGPU, stale-frame first) recycles evicted buffers into a bounded pool (`BufCache::retire`, ≤4 MB) instead of freeing them. Miss uploads are packed into a persistent 16 MB staging buffer and copied to pooled storage/uniform buffers (`stage_copy`; direct `queue.write_buffer` fallback when the ring is exhausted). Uploads are resolved BEFORE `begin_render_pass` — the pass borrows the encoder exclusively. Present bind group + readback buffer are persistent.
* **Explorer scan model** — `ProjectEntry { name, path, master, assets }` per folder under src_root; legacy flat `assets/`/`examples/` folders surface as pseudo-projects; root .moa files go to `loose`.
* **Autocomplete internals** — `completion.rs` is pure/logic-side (catalog, `user_symbols`, `prefix_before_caret`, `prefix_span_at`, `line_col_of`, `word_at_line_col`); editor.rs does egui UI. Accept path replaces the *byte span* of the identifier prefix ending at the caret and restores caret via `TextEditState::load/store` + `cursor.set_char_range`.
* **Arc scene state** — `Stmt::Decl.obj` and `Animation::Transform.target` are `Arc<VGroup>` (serde `rc` feature); `Player::state_at` returns `HashMap<String, Arc<VGroup>>` and cloning state is refcount bumps. Only the actively-animated object materializes a fresh VGroup per frame; completed Rotate/Scale/Shift finals are baked once per stmt into `TimelineIndex.baked_final`.
* **`Timeline::index()` / `Player::state_at_indexed`** — the t-invariant preamble is precomputed once per timeline: the Decl preamble is fully resolved into `decl_base` (ungated objects, cloned per sample as pure refcount bumps) + `intro_decls` (gated, inserted once `t >=` intro time), so the per-frame pure-target point-equality scan and per-Decl inserts are gone. `baked_final` caches completed Rotate/Scale/Shift geometry per stmt. Hot loops (IDE `eval_state_cached`, `mo preview`, CLI render loop) reuse the index; plain `state_at` builds it on the fly.
* **`FrameMemo`** (`mo-core/src/scene.rs`) — LRU (16384 entries) of sampled scene states keyed `(timeline_epoch, time_bits)`; values are Arc-shared maps so entries cost only map/key overhead. Wired into IDE `eval_state_cached` and `mo preview` `base_state()` — playback loops and scrub revisits are pure lookups; a timeline bump (epoch in key) orphans old entries. Eviction drops ~1/8 oldest in one pass (amortized). RAM-for-speed: idle ~90 MB, growth only from visited frames, plateaus at the cap.
* **`Timeline::duration()` memoized** via a `#[serde(skip)]` `OnceLock` (manual `Clone` resets it — struct literals elsewhere must use `Timeline::default()`/`new()`, not full literals).
* **IDE frame coherence** — one scene eval per frame shared by inspector + viewport (`app.rs::eval_state_cached`, keyed by time bits + `timeline_epoch`); viewport computes a `frame_sig` hash (time, camera, size, overrides, epoch) and skips the eval + GPU pass entirely when paused/unchanged; auto-compile is debounced 200 ms (flushed on save). `mo preview` similarly sig-guards `draw()` and only requests redraws while playing.

---

## 6. How to Run & Test

```bash
cargo test --workspace                                  # 77 tests, 0 warnings

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
20. **Dependency Façade Rule**: every third-party dep has one owning module per crate; don't add `use <dep>::` outside it. Concretely: mo-ide imports GUI types only via `crate::ext` (`crates/mo-ide/src/ext.rs` re-exports `egui`/`eframe`/`egui_wgpu`/`wgpu`/`image`); mo-render's wgpu boundary is aliased in `pipelines.rs` (`GpuDevice`, `GpuQueue`, `GpuTextureFormat`, `GpuCommandEncoder`, `GpuTextureView`) and callers never name `wgpu::`; mo-interactive/src/lib.rs owns all winit; mo-core/src/lib.rs re-exports shared math deps (`glam`, `nalgebra`, `ndarray`, `num_complex`) for downstream crates. A dependency version bump should be a one-file edit most of the time. The app icon is embedded at `crates/mo-ide/assets/mo.ico` (decoded in `main.rs::load_window_icon` → `ViewportBuilder::with_icon`); desktop launcher at `~/.local/share/applications/mathoxide.desktop` + `~/Desktop/`, binary installed to `~/.local/bin/mathoxide`, hicolor icon at `~/.local/share/icons/hicolor/256x256/apps/mathoxide.png`.
21. **Arc-state mutation sites**: `HashMap<String, Arc<VGroup>>` values are immutable — to apply a per-object override (IDE viewport shifts, `mo preview` drag), clone the one VGroup out of its Arc, mutate, and re-insert a fresh `Arc` (see `viewport.rs` / `mo-interactive/src/lib.rs`). Never `Arc::make_mut` inside eval — it would deep-copy shared geometry.
22. **Timeline replacement must bump `timeline_epoch`** — in mo-ide always assign via `self.set_timeline(...)` (never `self.timeline = ...` directly) or the state cache / viewport frame signature will serve stale frames.
23. **Staging-ring uploads precede the render pass**: `draw_scene_pass` resolves cache hits/uploads *before* `begin_render_pass` because the pass borrows the encoder exclusively (`stage_copy` needs `&mut encoder`). Don't move uploads back inside the draw loop. Misses that overflow the 16 MB ring fall back to direct `queue.write_buffer` — correct, just unpooled.
24. **Override-shift cache keys on base `Arc::as_ptr`** (viewport `shift_cache`): the entry keeps its base `Arc<VGroup>` alive, which is what makes the pointer identity sound — never store a raw pointer key without keeping the owner alive. Cleared on timeline epoch and capped (~128) against playback-drag growth.
25. **Timeline dock + scope**: the track area lives in a vertical ScrollArea (`drag_to_scroll(false)` — the painter's click_and_drag owns block move/trim/seek); no track cap; dock is fixed 180px. `TimelinePanel.scope` (ThisFile/FullMaster) is backed by `MathOxideApp.master_scope` (master path + mtime + compiled timeline + origins, rebuilt by `sync_master_scope`); in FullMaster scope playback/scrub/inspector/viewport/Apply-Timings all use the master timeline while the editor keeps the open asset. `TrackDrag` holds stmt indices — scroll-safe.
26. **Selection is a Vec, not Option**: `MathOxideApp.selection` (pick order, last = primary for Inspector). Viewport group drag captures per-id grab points (`ViewportPanel::group_grab`); overrides stay per-id in one map. Persistence is batched: `apply_shifts_to_dsl` rewrites all master-owned ids in one pass; alias-prefixed leftovers group per asset file (read/rewrite/write once each); a single recompile follows. `apply_shift_to_dsl` is now a `#[cfg(test)]` 1-element wrapper.
27. **Inline edit contract**: viewport double-click emits `ViewportAction::EditSource(id)`; the app locates the decl via `find_string_decl` (open file first, then imported asset files via alias prefix) and edits `text=`/`expr=` values through `string_param_span` (quote-terminates; the DSL has no `\"` escapes). Global shortcuts are gated on `!editor_focused && inline_edit.is_none()`; **Escape is ungated** (cancels edit → deselects). Overlay placement uses `ViewportPanel::last_canvas_rect` + `last_cam` (updated every `show()`).
28. **Asset solo playback is real**: the parser flattens imported asset plays into master at the import position (pixel-verified on master-showcase), and the IDE can play any file standalone (open it, or the Explorer `▶` button). "Assets don't play" reports should first be repro'd with `mo render --frame` before suspecting the engine.
29. **mo preview camera anims**: mo-interactive computes `has_camera_anims` once and routes through `Player::camera_at`; the draw signature hashes `phi/theta/gamma` — when adding camera-affecting state, extend that hash or keyframed moves get frozen by the skip-render path.

---

## 8. Direction & Roadmap — Next Up

1. **Directional lighting / normal-based shading** for filled `surface3d`.
2. **Fixed 1080p IDE viewport target** (half-res during playback/drag) so preview pixels match export; camera in target-pixel space with canvas↔target scale helpers.
3. **Undo/redo** in the IDE (code edits, explorer rename/delete, apply-to-code round-trips) — prerequisite for a Delete-object keybind.
4. **Marquee rubber-band select** behind a tool mode (would need to take empty-drag pan).
5. **`.mcp.json` + agent loop hardening**: `assemble_scene` should use `parse_with_origins`; consider an MCP `apply_timings` tool.
6. **CLI `render` of a project folder** (`mo render ~/mathoxide/foo` → `foo/master.moa`).
7. **Video-editor polish**: onion skin, timeline markers + loop in/out region, timecode (HH:MM:SS:FF), audio track (rodio behind a façade module), keyframe easing-graph editor, autosave, render-range export, copy/paste objects, error squiggles per line + go-to-def on import aliases, hover docs for user symbols.

### Strict keybind table (IDE; single-key gated on editor unfocused + no inline edit)
| Key | Action |
|---|---|
| Space | play / pause |
| ← / → | nudge selection 0.1 (no selection: step ±1 frame) |
| Shift+← / → | nudge 1.0 (no selection: step ±10 frames) |
| ↑ / ↓ | nudge selection vertically |
| Home / End | jump to start / end |
| R | reset camera |
| G | toggle grid snap (0.25) |
| Esc | cancel inline edit / deselect |
| Double-click | inline edit text/expr of object |
| Shift+click | toggle object in selection |
| Ctrl+S / ⇧Ctrl+S | save / save as |
| Ctrl+O / Ctrl+N | open file / new asset |

### Instructions for Future Agents
* **Hardware**: 16 GB RAM (use freely), iGPU ~128 MB VRAM (keep per-frame GPU uploads bounded).
* **Scenes live outside the repo** in `~/mathoxide`. Never recreate `assets/`/`examples/` in-repo; tests build temp projects.
* Keep committing clean, self-contained milestones to `main`.
* **Arjun's preferences**: direct, mechanism-first, no fluff; assets edited one-at-a-time by design; MCP output states timings explicitly so AI never guesses overlaps; IDE layout is FIXED (don't reintroduce resizable panels or tab switching); palette is cyberpunk (no grays — use deep purples for neutrals).
