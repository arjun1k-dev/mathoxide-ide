# MathOxide — Handoff (2026-10-17)

## State

Pure Rust mathematical animation studio & ManimGL replacement at `/home/arjun/Projects/mathoxide` (git, branch `main`).
**51 unit tests green across workspace, 0 warnings.**

```
mo_core:        34 passed
mo_dsl:         10 passed
mo_ide:          3 passed
mo_moab:         2 passed
mo_render:       2 passed
```

---

## Breakthrough Highlights (Latest Session)

### 1. Modular Asset Architecture (`assets/*.moa` & Master Compositions)
- **Isolation over Monoliths**: Individual components, formulas, plots, and geometries live in isolated files (e.g. `assets/euler_formula.moa`, `assets/sine_wave.moa`, `assets/gaussian_curve.moa`, `assets/cube_3d.moa`).
- **Composition Import DSL**:
  ```moa
  import "assets/euler_formula.moa" as euler at (0, 1.8);
  import "assets/sine_wave.moa" as plot at (0, -2.0);
  ```
  Prefixes identifiers with `<alias>_`, offsets geometries by `at (x, y)`, offsets transform targets, and merges sub-timelines into master sequence.
- **Agent/MCP Friendly**: AI agents and developers inspect and edit individual small assets without touching or scanning monolithic scripts.

### 2. Viewport Squishing Resolved & 16:9 Letterboxed Canvas
- **Root Cause of Squishing**: Previous viewport stretched GPU render textures to arbitrary UI panel dimensions while camera aspect was mismatched, compressing graphics horizontally on 14-inch laptops and multi-panel layouts.
- **Fixed 16:9 Canvas**: Viewport now computes an exact 16:9 fitted rectangle centered in the available panel area with dark canvas backing, sleek 1px frame border, and synchronized `camera.width` / `camera.height`.
- **WYSIWYG 1:1 Parity**: Viewport display exactly matches 1080p MP4 exports. Circles are true circles, text retains proper aspect.

### 3. Full 3D Camera Orbit & 3D Primitives
- **Camera Orientation**: Added polar tilt (`phi`), azimuth (`theta`), and roll (`gamma`) angles to `Camera` with 3D view matrix in `uniforms()`:
  $$R = R_z(\gamma) \cdot R_x(\phi) \cdot R_z(\theta)$$
- **Viewport Interaction**:
  - **Right-Click Drag / Alt + Left Drag**: Smooth 3D camera orbit (`camera.orbit()`).
  - **Middle-Click Drag / Shift + Left Drag**: Pan camera (`camera.pan()`).
  - **Scroll Wheel**: Smooth zoom (`camera.frame_height`).
  - **Key 'R'**: Instant camera reset to 2D default.
- **3D Primitives in DSL & Core**:
  - `axes3d`: 3D Cartesian axes with labeled arrows (X: Red, Y: Cyan, Z: Purple).
  - `cube`: 3D wireframe cube with 12 edges and corner vertices.
  - `sphere`: 3D sphere wireframe with longitudinal and latitudinal geodesic rings.
  - `surface3d`: 3D parametric function mesh ($z = f(x, y)$) evaluated with `ndarray` grid sampling.
- **CLI 3D Rendering**: `mo render --phi <deg> --theta <deg> --gamma <deg>` renders 3D perspective directly to MP4 at 70–97 FPS.

### 4. Native Laptop File System Interaction & Asset Browser
- **Native File Dialogs (`rfd`)**:
  - `File -> Open File... (Ctrl+O)`: Native file picker (`.moa`, `.moab`).
  - `File -> Open Project Folder...`: Select any folder on the laptop; scans all assets.
  - `File -> Save (Ctrl+S)` / `Save As... (Ctrl+Shift+S)`: Native file save dialog.
  - `File -> Export MP4 Video...`: Pick destination filename; background export thread renders video without freezing the UI.
  - `File -> Export .moab Bundle...`: Native binary bundle export.
- **Asset Browser Panel (`assets.rs`)**:
  - Dedicated `📁 Assets` tab in the IDE left panel.
  - Real-time catalog of **🎬 Master Compositions** and **📦 Modular Assets** with tags (`[LaTeX]`, `[3D]`, `[Math Plot]`, `[Transform]`), file size, and line count.
  - Quick action: `➕ Import` button inserts `import "..." as ... at (0, 0);` straight into the active editor code.
  - `➕ New Asset` and `🎬 New Master` buttons generate scaffolded template clips.

### 5. Inspector "Apply Shift to Code"
- Dragging mobjects in the viewport establishes a live visual override.
- Inspector Transform panel includes **💾 Apply to Code**: writes the new `at (x, y)` coordinates directly into the `.moa` source code in the editor, recompiles, and clears the temporary override.

### 6. Full Model Context Protocol (MCP) Server (`crates/mo-mcp`)
- Implemented standard JSON-RPC 2.0 MCP server over stdio (`mathoxide-mcp`).
- Tools exposed to AI agents:
  1. `list_assets`: Returns all modular assets, master scenes, durations, line counts.
  2. `read_asset`: Reads source code, parses syntax validity, duration, and object hierarchy.
  3. `write_asset`: Creates or updates asset files with automated syntax validation.
  4. `validate_code`: Headless DSL parser & lowerer returning structured diagnostics.
  5. `render_frame`: Headless offscreen GPU render of any scene at timestamp $t$ to PNG.
  6. `assemble_scene`: Automatically sequences multiple assets into a master `.moa` file.

### 7. Typography & Font Weight Bug Fix
- **Cause**: Text outlines were stroked with $sw > 0$ during `ShowCreation` transitions, ballooning vector TrueType glyphs into distorted bold shapes before snapping to normal weight.
- **Fix**: Outlines now render strictly with vector fill ($sw = 0$) using sequential glyph reveal (`extract_subpaths()`). Typography is crisp and slim from frame 0.

---

## Workspace Structure

- `crates/mo-core` — Geometry, curves, 3D camera orbit, NumPy-substitute vector math (`ndarray`, `nalgebra`, `statrs`, `rustfft`), Typst LaTeX math compiler, and animation primitives.
- `crates/mo-render` — WGPU ManimGL 3-pass stencil fill + procedural GPU stroke pipeline, `render_with_camera()`, offscreen frame rendering.
- `crates/mo-dsl` — Recursive-descent parser supporting math functions, LaTeX expressions, 3D primitives, and modular `import "..." as ... at (...)` resolution.
- `crates/mo-ide` — MathOxide Studio GUI (eframe 0.31): 16:9 letterboxed viewport, 3D orbit controls, live code editor with hot reload, Asset Browser, Scene Tree, Inspector with "Apply to Code", and Sequencer Timeline.
- `crates/mo-cli` — Command line binary: `mo validate`, `mo render` (MP4/PNG with `--phi`, `--theta`), `mo preview`, `mo compile`, `mo inspect`.
- `crates/mo-mcp` — JSON-RPC stdio MCP server for agentic interaction.
- `assets/` — Modular component assets (`euler_formula.moa`, `sine_wave.moa`, `gaussian_curve.moa`, `cube_3d.moa`).
- `examples/` — Master scenes (`master_video.moa`, `demo_3d.moa`, `latex_showcase.moa`, `demo.moa`).
