# MathOxide — Project Handoff Document (2026-10-17)

## 1. Executive Summary & Status

* **Repository**: `/home/arjun/Projects/mathoxide` (git branch `main`)
* **Core Philosophy**: A pure-Rust, hardware-accelerated mathematical animation engine, visual IDE studio, and agentic media suite replacing Manim / ManimGL with zero Python runtime dependency.
* **Build & Test Health**: **51 workspace unit tests pass with zero errors and zero compiler warnings.**
  ```
  mo-core:        34 passed
  mo-dsl:         10 passed
  mo-ide:          3 passed
  mo-moab:         2 passed
  mo-render:       2 passed
  ```
* **Git Checkpoint State**: Clean working tree. Commits on `main`:
  - `9c61767`: `feat: modular asset architecture, 16:9 letterbox viewport, 3D orbit and native file dialogs`
  - `7e46e17`: `feat: complete Model Context Protocol (MCP) server for MathOxide agent integration`
  - `ae5bbdd`: `docs: update HANDOFF.md with asset architecture, 16:9 viewport, 3D orbit, and MCP server`

---

## 2. Workspace Crates & Architecture

```
mathoxide/
├── assets/                     # Modular component clips (isolated .moa files)
│   ├── euler_formula.moa       # Euler's identity & complex exponential morphing
│   ├── sine_wave.moa           # Harmonic wave plotted over Cartesian axes
│   ├── gaussian_curve.moa      # Normal distribution curve & integral formula
│   └── cube_3d.moa             # 3D wireframe cube with coordinate axes
├── examples/                   # Master scenes & compositions
│   ├── master_video.moa        # Video-editor style sequence importing modular assets
│   ├── demo_3d.moa             # Full 3D showcase (axes3d, cube, sphere, surface3d)
│   ├── latex_showcase.moa      # LaTeX equations compiled into quadratic Bézier curves
│   └── demo.moa                # Basic shapes and morph transforms
├── shaders/                    # Vendored ManimGL WGSL shaders
│   ├── fill.wgsl               # 3-pass winding number stencil fill & anti-aliasing
│   ├── stroke.wgsl             # Procedural GPU ribbon expansion with analytic AA
│   └── inserts/                # FrameUniforms std140 camera layout
└── crates/
    ├── mo-core                 # Core math, geometry, camera, typography, NumPy substitutes
    ├── mo-render               # Offscreen and native WGPU rendering pipelines
    ├── mo-dsl                  # Hand-rolled recursive-descent parser & import lowerer
    ├── mo-moab                 # Binary animation container format (Postcard + Header)
    ├── mo-interactive          # Low-level winit 0.30 preview window
    ├── mo-ide                  # MathOxide Studio GUI (egui 0.31 / eframe)
    ├── mo-cli                  # Headless command-line renderer & compiler (`mo`)
    └── mo-mcp                  # Standard JSON-RPC 2.0 Model Context Protocol server
```

---

## 3. Major Features & Mechanisms Ported

### A. Modular Asset-Wise Architecture (`assets/*.moa`)
* **Isolated Component Files**: Every equation, function plot, or geometry is written as an isolated clip file (`assets/*.moa`). Assets are evaluated independently with their own local coordinate space.
* **Composition Imports**:
  ```moa
  // examples/master_video.moa
  text title text="MathOxide Modular Scene Composition" at (0, 3.4) size=0.7 color=white;
  play write(title) 1.0s smooth;

  import "assets/euler_formula.moa" as euler at (0, 1.8);
  import "assets/sine_wave.moa" as plot at (0, -2.0);

  wait 1.0s;
  ```
  The compiler (`mo-dsl`) prefixes all identifiers with `<alias>_`, offsets object geometries and transform targets by `at (x, y)`, and concatenates the animation timeline.

### B. Viewport 16:9 Letterbox (Zero Squishing)
* **Root Cause of Prior Squishing**: Arbitrary panel sizing in egui caused the WGPU texture to be stretched to non-16:9 aspect ratios while camera matrices remained fixed, compressing circles into vertical ellipses on 14-inch laptops.
* **The Fix**: Viewport computes an exact 16:9 fitted rectangle centered in the panel area, synchronizes `camera.width = canvas_w` and `camera.height = canvas_h`, and renders with a sleek 1px frame border and dark letterbox backdrop. 100% WYSIWYG parity with 1080p MP4 exports.

### C. 3D Camera Orbit & 3D Primitives
* **Camera Model**: Added polar tilt ($\phi$), azimuth ($\theta$), and roll ($\gamma$) angles with standard 3D rotation in `Camera::uniforms()`:
  $$R = R_z(\gamma) \cdot R_x(\phi) \cdot R_z(\theta)$$
* **Viewport Mouse Gestures**:
  - **Right-Click Drag / Alt + Left Drag**: Orbit 3D elevation and azimuth.
  - **Middle-Click Drag / Shift + Left Drag**: Pan camera in scene units.
  - **Scroll Wheel**: Smooth zoom (`camera.frame_height`).
  - **Press `R`**: Instantly reset camera to 2D default.
* **3D Primitives**:
  - `axes3d`: 3D coordinate system with colored arrowheads (X: Red, Y: Cyan, Z: Purple).
  - `cube`: 3D wireframe box with 12 edges and corners.
  - `sphere`: 3D wireframe sphere with latitude and longitude rings.
  - `surface3d`: 3D parametric function mesh ($z = f(x, y)$) evaluated with `ndarray` grid sampling.

### D. Native Laptop File Dialogs & Asset Browser Panel
* **Native Dialogs (`rfd`)**: Integrated cross-platform OS file pickers for Open File (`Ctrl+O`), Open Project Folder, Save (`Ctrl+S`), Save As (`Ctrl+Shift+S`), and Export MP4 / MOAB.
* **Asset Browser (`📁 Assets` tab)**: Catalogs all files in `assets/` and `examples/`, displays tags (`[LaTeX]`, `[3D]`, `[Math Plot]`, `[Transform]`), file sizes, and provides a `➕ Import` button to insert import statements directly into active code.
* **Inspector "Apply Shift to Code"**: Visual viewport dragging updates coordinates in the `.moa` code automatically when clicking `💾 Apply to Code`.

### E. Model Context Protocol (MCP) Server (`crates/mo-mcp`)
* A standard JSON-RPC 2.0 stdio server (`mathoxide-mcp`) allowing autonomous AI agents to interact with MathOxide headlessly:
  - `list_assets`: Returns all modular assets, master scenes, durations, line counts.
  - `read_asset`: Reads source code, parses syntax validity, duration, and object hierarchy.
  - `write_asset`: Creates or updates asset files with automated syntax validation.
  - `validate_code`: Headless DSL parser & lowerer returning structured diagnostics.
  - `render_frame`: Headless offscreen GPU render of any scene at timestamp $t$ to PNG.
  - `assemble_scene`: Automatically sequences multiple assets into a master `.moa` file.

### F. Typography & Font Weight Bug Fix
* Vector text transitions (`ShowCreation` / `Write`) previously stroked glyph outlines with $sw = 2.0$, causing text to look heavily bold before snapping to normal weight.
* Outlines now render strictly with vector fill ($sw = 0$) using sequential glyph reveal (`extract_subpaths()`). Typography is crisp from frame 0.

---

## 4. How to Run & Test

### Run the Interactive Studio (IDE):
```bash
cargo run -p mo-ide
```
* Use `📁 Assets` tab on the left to switch between scenes and modular components.
* Hold **Right-Click** and drag on the viewport to orbit 3D camera. Press **R** to reset.
* Drag any object to position it, then click **`💾 Apply to Code`** in the inspector.

### Run All Workspace Unit Tests:
```bash
cargo test --workspace
```

### Render Videos Headless via CLI:
```bash
# Render modular master video to MP4:
cargo run -p mo-cli -- render examples/master_video.moa -o master_showcase.mp4

# Render 3D perspective video with camera tilt and azimuth:
cargo run -p mo-cli -- render examples/demo_3d.moa -o demo_3d.mp4 --phi 65 --theta -40

# Render a single still frame as PNG:
cargo run -p mo-cli -- render examples/master_video.moa -o frame.png --frame 4.5
```

### Test the MCP Server via stdio:
```bash
printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_assets","arguments":{"dir":"."}}}\n' | cargo run -p mo-mcp
```

---

## 5. Key Engineering Gotchas & Hard-Won Lessons

1. **WGPU 24 API Conventions**:
   - `Instance::new(&desc)` and `request_adapter` return Results.
   - Submitting work requires `queue.submit(Some(encoder.finish()))`.
   - Stencil state requires explicit `read_mask` and `write_mask` fields.
2. **Subpath Separation in VGroup**:
   - Multi-character text and math equations must keep individual character curves in separate `SubPath` records in `VGroup`. Merging all points into one gigantic subpath causes stencil winding passes to bridge empty space between characters.
3. **Camera Uniform Rescaling**:
   - Camera uniforms use `frame_rescale_factors = [2.0 / (frame_height * aspect), 2.0 / frame_height, 1.0 / focal_distance]`. Always update `camera.width` and `camera.height` when resizing viewport render targets to avoid horizontal squishing.
4. **Wayland / Eframe Stability**:
   - Keep window size within `1280x760` or `1366x768` by default to ensure comfort on 14-inch laptop screens without panel clipping.
5. **No Artificial Stroke on Fonts**:
   - TrueType font glyphs extracted via `ttf-parser` are designed solely for vector fill. Never set `stroke_width > 0` on font outlines unless explicitly requested for an outline-only aesthetic.

---

## 6. Instructions for Future Agents & Developers

* **Preserve Documentation & Comments**: Always maintain existing comments and docstrings.
* **Keep Asset Isolation**: When creating new mathematical animations, always write the individual component in `assets/<name>.moa` first, test it in isolation, and then import it into a composition.
* **Version Control**: Keep committing clean, self-contained milestones to git (`main` branch) to prevent regressions.
* **Project Memory & Preferences**: User is Arjun. Direct, mechanism-first, no fluff.
