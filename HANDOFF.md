# MathOxide — Project Handoff Document (2026-10-17)

## 1. Executive Summary & Status

* **Repository**: `/home/arjun/Projects/mathoxide` (git branch `main`)
* **Core Philosophy**: A pure-Rust, lightweight mathematical animation engine, visual IDE studio, and agentic media suite replacing Manim / ManimGL with zero Python runtime dependency, optimized to run smoothly on standard laptops with integrated graphics (~128 MB VRAM).
* **Build & Test Health**: **51 workspace unit tests pass with zero errors and zero compiler warnings.**
  ```
  mo-core:        34 passed
  mo-dsl:         10 passed
  mo-ide:          3 passed
  mo-moab:         2 passed
  mo-render:       2 passed
  ```
* **Git Checkpoint State**: Clean working tree. Latest commits on `main`:
  - `9c61767`: `feat: modular asset architecture, 16:9 letterbox viewport, 3D orbit and native file dialogs`
  - `7e46e17`: `feat: complete Model Context Protocol (MCP) server for MathOxide agent integration`
  - `005ea61`: `docs: finalize comprehensive HANDOFF.md for future sessions`

---

## 2. Evolution & Decisions: What Changed from Initial Handoff to Now

This section outlines every user request, architectural decision, and code transformation that took place between the initial handoff and the current state:

| Area | Initial Handoff State | User Feedback / Request | Decision & Solution Implemented |
|---|---|---|---|
| **Viewport Display** | GPU texture stretched dynamically across the center panel. Circles appeared as squished tall ovals on 14-inch laptops. | *"First thing, the renderer is squished... give it a permanent 14 inch width so it isn't a screen size or window width issue."* | **16:9 Letterboxed Canvas**: Decided to enforce a fixed 16:9 aspect ratio centered within the viewport panel with dark letterbox backing and synced `camera.width` / `camera.height`. Viewport is now 1:1 pixel-accurate to 1080p MP4 exports. |
| **Project Architecture** | Monolithic scripts containing entire scene definitions from scratch. | *"We almost have an interface like a video editor... what I want is a file for each asset... code in each file is an individual asset and not an entire video template... main file imports all other assets and puts them into scene."* | **Modular Asset Paradigm**: Built `assets/*.moa` for component clips and `examples/master_video.moa` for compositions. Added `import "path" [as alias] [at (x, y)];` syntax in `mo-dsl`. Pre-resolves geometries, offsets coordinates, and prevents human/AI context overload. |
| **Math & NumPy Equivalent** | Hardcoded math approximations for sine curves; no general vector math engine. | *"Is it real math?? Manim uses numpy which is literally compiled C... here is a list of useful libraries (ndarray, nalgebra, rustfft, statrs)... apply it so software doesn't hardcode."* | **Real Vectorized Math in Core**: Integrated `ndarray`, `nalgebra`, `rustfft`, `statrs`, `simba` into `mo-core::math`. Replaced ad-hoc point loops with vectorized `linspace`, parametric 2D/3D evaluation, and exact matrix transformations. |
| **Typography & LaTeX** | Raw basic SVG text and placeholder math. | *"Let's refine rendering... we need latex rendering, object transformation, and other stuff that makes manim cool."* | **Typst-Powered LaTeX Math**: Integrated Typst compiler into `mo-core::svg`, translating standard LaTeX formulas into high-precision quadratic Bézier curves with exact mathematical kerning without external LaTeX installations. |
| **Font Boldness Bug** | Text and formulas spawned as heavy, bold vector shapes and snapped to slim weights when animations completed. | *"When text or math appears, it initially starts as very bold font, and once fully into scene it turns to normal. Look what's causing this and fix it!"* | **Zero Synthetic Stroke**: Identified that `ShowCreation` applied an artificial stroke ($sw = 2.0$) on font curves during transitions. TrueType outlines are designed for fill only. Replaced with sequential subpath reveal (`extract_subpaths()`) with $sw = 0$. Fonts are razor-sharp from frame 0. |
| **File System Interaction** | IDE only edited a hardcoded buffer; couldn't open or save files on the laptop. | *"Currently, the IDE can't interact with files on my laptop. So add that feature or fix it if already exists."* | **Native OS File Dialogs (`rfd`)**: Added cross-platform system file dialogs for Open File (`Ctrl+O`), Open Project Folder, Save (`Ctrl+S`), Save As (`Ctrl+Shift+S`), and Export MP4 / MOAB. Added dedicated `📁 Assets` browser tab in IDE left panel. |
| **3D Rendering** | Purely 2D top-down camera with no elevation tilt, azimuth, or 3D primitives. | *"We also need to add 3D, just tell me if current resources are enough or you need more."* | **Full 3D Orbit & Primitives**: Existing WGPU shaders and `glam` were 100% sufficient without adding heavy game engines. Added polar tilt ($\phi$), azimuth ($\theta$), and roll ($\gamma$) to `Camera`. Added `axes3d`, `cube`, `sphere`, `surface3d` ($z = f(x, y)$), and right-click viewport 3D orbit controls. |
| **Agent / MCP Tooling** | `crates/mo-mcp` was an empty stub with `main()` printing "coming in Phase 5". | *"This will allow our later MCP integration plan to be successful, as it won't be scanning entire codebase..."* | **Full Model Context Protocol Server**: Implemented standard JSON-RPC 2.0 stdio server in `crates/mo-mcp` with tools for listing assets, reading source, writing assets, syntax validation, and headless frame rendering. |

---

## 3. Workspace Crates & Architecture

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

## 4. Key Mechanisms Implemented

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
* **The Math**:
  $$\text{aspect} = 16.0 / 9.0$$
  $$\text{canvas\_w} = \begin{cases} \text{avail\_h} \times \text{aspect} & \text{if } \text{avail\_w} / \text{avail\_h} > \text{aspect} \\ \text{avail\_w} & \text{otherwise} \end{cases}$$
  $$\text{canvas\_h} = \text{canvas\_w} / \text{aspect}$$
* The canvas is centered in the viewport with a 1px frame border.
* `camera.width = canvas_w` and `camera.height = canvas_h` are continuously maintained, ensuring hit-testing and WGPU projections match with 0% distortion.

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

---

## 5. How to Run & Test

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

## 6. Key Engineering Gotchas & Hard-Won Lessons

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

## 7. Direction & Roadmap: Where We Are Going & What to Carry On With

### The Grand Vision
MathOxide is evolving into an autonomous mathematical creative suite. A developer or agent describes a visual proof, numerical concept, or video structure, and MathOxide:
1. Generates modular assets in `assets/`.
2. Assembles them into compositions in `examples/`.
3. Validates and live-previews them in the interactive IDE.
4. Exports broadcast-quality 4K/60fps MP4s in fractions of real-time.

### Priority Queue for the Next Agent

If you are the incoming agent picking up the codebase, here is your concrete, prioritized roadmap:

#### ~~Task 1: Interactive Timeline Track Drag & Trim~~ — DONE (commit a32ca49)
Animation blocks are draggable (trades time with a preceding/inserted Wait) and right-edge trimmable; `💾 Apply Timings to Code` rewrites `Ns` durations in the source (skipped for scenes with `import` — durations there live in the asset files).

#### ~~Task 2: Camera Animation Keyframing in DSL~~ — DONE (commit 2b16592)
```moa
play camera.orbit(phi=60, theta=45) 2.0s smooth;
play camera.zoom(factor=1.5) 1.0s smooth;
play camera.move_to(x=2, y=-1) 1.5s smooth;
```
`Player::camera_at(tl, t, base)` interpolates camera state; wired into CLI render (frame + video) and the IDE viewport (animated camera overrides the interactive base). Demo: `assets/camera_motion.moa`.

#### ~~Task 3: 3D Surface Shading & Lighting Depth~~ — DONE (commit cfd7bec)
`surface3d ... fill=true` renders flat-shaded cell quads colored by a Viridis height ramp (`color::viridis`) with a darkened height-tinted wireframe on top. Demo: `assets/shaded_surface.moa`. Possible follow-up: true directional lighting in the shader.

#### ~~Task 4: Packaging the MCP Server~~ — DONE (commit e8f6eac)
Project-level `.mcp.json` registers `mathoxide-mcp` (release binary, stdio JSON-RPC). Claude Code prompts for approval on first use in this directory.

#### Next up (suggested)
1. Timeline drag/trim for imported-asset timings (currently Apply-Timings skips files with `import`).
2. Directional lighting / normal-based shading for `surface3d` fills.
3. Playback of camera anims inside `mo preview` (winit window) — currently only CLI render + IDE viewport use `camera_at`.

---

## 8. Instructions for Future Agents & Developers

* **Hardware Target & Constraints (CRITICAL)**:
  - The user's system runs on **integrated graphics with ~128 MB VRAM** (Intel/AMD iGPU), NOT a dedicated discrete GPU.
  - The existing lightweight WGPU shader pipeline (`fill.wgsl`, `stroke.wgsl`) is completely handleable for this system and should remain as-is.
  - **DO NOT force or assume heavy discrete GPU features**: Do NOT introduce heavy compute shaders, giant 4K/8K texture buffers, complex shadow cascades, or heavy game engines (e.g. Bevy / heavy render graphs) that would overwhelm 128 MB VRAM.
  - Keep geometry procedural, buffers lean and re-usable, memory allocations minimal, and CPU/iGPU execution smooth and lightweight.
* **Preserve Documentation & Comments**: Always maintain existing comments and docstrings.
* **Keep Asset Isolation**: When creating new mathematical animations, always write the individual component in `assets/<name>.moa` first, test it in isolation, and then import it into a composition.
* **Version Control**: Keep committing clean, self-contained milestones to git (`main` branch) to prevent regressions.
* **User Preferences (Arjun)**: Direct, mechanism-first, no fluff. Keep the engine native pure Rust, lightweight, and responsive on standard laptop hardware with integrated graphics.
