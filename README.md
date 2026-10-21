# MathOxide (mathoxide-ide)

<div align="center">

**A pure-Rust mathematical animation engine, real-time visual studio, and agentic media suite.**  
*Re-engineered from the ground up to replace Python Manim / ManimGL with zero Python dependencies, blazing real-time GPU performance, and an interactive studio.*

[![License: MIT](https://img.shields.io/badge/License-MIT-purple.svg)](LICENSE)
[![Rust: 2021](https://img.shields.io/badge/Rust-2021_Edition-orange.svg)](https://www.rust-lang.org)
[![Tests: Passing](https://img.shields.io/badge/Workspace_Tests-77_Passed_·_0_Warnings-00FF9D.svg)]()
[![Hardware: Low--VRAM Friendly](https://img.shields.io/badge/Hardware-iGPU_~128MB_VRAM-00FFF7.svg)]()

[Features](#-key-features) •
[Architecture](#-workspace-architecture) •
[DSL Guide](#-mathoxide-dsl-reference) •
[IDE Studio](#-visual-studio-ide) •
[CLI Tool](#-cli-tool-mo) •
[MCP Server](#-mcp-ai-agent-integration) •
[Installation](#-getting-started) •
[Homage to 3b1b](#-homage--tribute-to-3blue1brown-grant-sanderson) •
[License](#-license--attribution)

</div>

---

## ⚡ Why MathOxide?

[3Blue1Brown](https://github.com/3b1b)'s Manim and the Manim Community edition redefined mathematical visualization. However, Python runtime friction, multi-gigabyte virtual environments, complex C/Cairo dependencies, fragile OpenGL contexts, and non-interactive render loops often hinder workflow and performance.

**MathOxide** is a clean-room, pure-Rust re-implementation engineered for speed, interactive authoring, and autonomous agent workflows:

* 🚀 **Zero Python Dependency**: Compiled to a single static binary. No `python`, `pip`, `venv`, `ffmpeg-python`, or `pycairo` required.
* ⚡ **Ultra-Fast WGPU / WGSL Pipeline**: Modern WebGPU backend with content-hashed buffer caching, bounded pool recycling, and persistent staging rings.
* 💻 **Low-Spec Hardware Friendly**: Engineered specifically to run smoothly at 60 FPS on standard laptops with integrated graphics (~128 MB VRAM and 16 GB system RAM) via dynamic stride decimation and resolution clamping.
* 🎨 **Interactive Visual Studio**: A dedicated fixed-layout desktop studio (Explorer, Code Editor with live syntax highlighting & autocomplete, 1080p Viewport with interactive gizmos & inline text editing, Inspector, and Multi-Track Timeline Sequencer).
* 📐 **Exhaustive Manim CE Built-in Library**: Complete 2D shapes, 1D vectors, 3D geometry, coordinate systems, and calculus tools.
* 🖋️ **Typst-Powered Vector Math**: Crisp, instant mathematical typesetting powered by Typst without synthetic stroke distortion or external LaTeX installations.
* 🤖 **AI-Native & MCP Enabled**: First-class Model Context Protocol (`mo-mcp`) server and CLI toolchain allowing AI agents to inspect assets, assemble scenes, and manipulate timeline timings autonomously.

---

## 🌟 Key Features

### 1. Exhaustive Mathematical Built-ins
* **2D Primitives**: `circle`, `square`, `rectangle`, `rounded_rect`, `arc`, `ellipse`, `regular_polygon`, `triangle`, `annulus`, `sector`, `annular_sector`, `star`, `brace` ($\underbrace{\quad}$), and `elbow` ($\llcorner$).
* **1D Vectors & Lines**: `line`, `arrow`, `double_arrow` ($\longleftrightarrow$), `curved_arrow`, `curved_double_arrow`, `dashed_line`, and `tangent_line`.
* **Coordinate Systems & Graphing**: `axes`, `number_plane` (grid lines with major/minor steps), `complex_plane`, `polar_plane`, `number_line`, and parametric curves $(x(t), y(t))$.
* **Calculus & Data Tools**: `riemann` sums (`left`, `right`, `mid`), `secant` lines, `area` under curves, and `barchart` visualizers.
* **3D Geometry**: `cube_3d`, `sphere_3d`, `cylinder`, `cone`, `surface_3d`, `line3d`, and `dot3d` with polar camera control ($\phi, \theta, \gamma$).

### 2. Multi-Font Vector Typography
* True vector glyph rendering directly from TrueType curves.
* Select fonts seamlessly via `font="ubuntu"`, `font="mono"` / `"hack"`, `font="serif"`, `font="sans"` / `"noto"`, or any custom `.ttf`/`.otf` path.
* Embedded zero-dependency fallback fonts (`Ubuntu Light`, `Hack Regular`, `Noto Emoji`) guarantee that MathOxide renders reliably offline on minimal operating system setups.
* Dynamic numeric displays (`decimal`, `integer`) and mathematical `matrix` formatting (brackets `[`, `(`, or `|`).

### 3. Rich Animation Suite
* **Creation & Erasing**: `showCreation`, `uncreate`, `write`, `unwrite`, and `drawBorderThenFill`.
* **Transforms & Motion**: `transform` (point-wise morphing), `fadeIn`, `fadeOut`, `grow`, `scale`, and `shift`.
* **Dynamic Emphasis**: `indicate`, `wiggle`, `circumscribe` (animated highlight box/ellipse), `flash` (radiating light burst), `spinInFromNothing`, and `shrinkToCenter`.
* **Camera Animations**: `camera polar`, `camera zoom`, and `camera move` with smooth spherical interpolation.

---

## 🖥️ Visual Studio IDE

MathOxide includes a fixed-layout cyberpunk studio built with `egui` and `wgpu`:

```
┌─────────────────┬──────────────────────┬──────────────────────────────┐
│  ASSET EXPLORER │     CODE EDITOR      │       1080p VIEWPORT         │
│  (Projects &    │   (Autocomplete,     │   (Interactive Canvas,       │
│   Asset Files)  │    Hover Docs,       │    Inline Edit, Snap Grid)   │
│                 │    Syntax Colors)    ├──────────────────────────────┤
│                 │                      │          INSPECTOR           │
├─────────────────┴──────────────────────┴──────────────────────────────┤
│                     MULTI-TRACK TIMELINE SEQUENCER                    │
│        (Drag/Trim Blocks, Scrub Transport, "Apply Timings to Code")   │
└───────────────────────────────────────────────────────────────────────┘
```

### Studio Capabilities
* **Interactive Viewport Gizmos**: Select objects directly on the canvas, multi-select with `Ctrl + Click`, drag to reposition, nudge with arrow keys, and snap to a configurable grid (`Ctrl + G`).
* **Live Inline Editing**: Double-click or press `Enter` on any text, label, or math formula to pop an anchored inline editor that immediately recompiles the underlying code.
* **Master vs. Solo Asset Preview**: Declarative asset files without `play` statements automatically gain a synthetic sequential preview, letting you inspect component graphics instantly.
* **Bidirectional Timeline Rewrites**: Drag or trim animation blocks on the timeline, then click `💾 Apply Timings to Code` to write updated durations directly back into the source `.moa` files across project boundaries.
* **High-DPI / 1080p Coherence**: Viewport renders to an exact 1080p render target with half-resolution interactive fallback for butter-smooth scrubbing on low-spec hardware.

### IDE Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Space` | Toggle playback play / pause |
| `Home` | Seek transport to beginning (`0.0s`) |
| `←` / `→` | Step 1 frame backward / forward ($1/60\text{ s}$) |
| `Shift + ←` / `Shift + →` | Step 10 frames backward / forward |
| `Ctrl + S` | Save file & recompile |
| `Ctrl + G` | Toggle canvas grid & coordinate snapping |
| `Arrow Keys` | Nudge selected objects by grid step ($0.25\text{ units}$) |
| `Shift + Arrow Keys` | Fast nudge selected objects ($1.00\text{ unit}$) |
| `Enter` / Double Click | Open inline text/expr edit overlay in viewport |
| `Esc` | Cancel inline edit / Clear selection |
| `R` | Reset camera to 2D default orientation |
| `Ctrl + Click` | Toggle multi-select for canvas objects |

---

## 📐 MathOxide DSL Reference

MathOxide files (`.moa`) use a declarative, whitespace-delimited grammar.

### Grammar Conventions
1. **Comments**: Always start with `//` (C-style single line).
2. **Hex Colors**: Prefixed with `#` (e.g. `#58C4DD`, `#FFE066`, `#FF00A8AA`).
3. **Object Declarations**: `<kind> <id> [param=value ...];`
4. **Imports**: `import "assets/<name>.moa" as <alias> [at (x, y)];`
5. **Animations**: `play <animation>(<target>[, ...]) <duration>s [easing];`

### Example: Complete Project

#### `assets/card.moa` (Reusable Asset)
```moa
// Reusable presentation card centered at local origin (0, 0)
rounded_rect box w=6.0 h=3.5 r=0.4 stroke=#00FFF7 sw=2.5 fill=#0A002BCC;
tex title text="\\int_{-\\infty}^\\infty e^{-x^2} dx = \\sqrt{\\pi}" size=1.2 color=#FFE066 at (0, 0.4);
text subtitle text="Gaussian Integral" size=0.6 font="serif" color=#DDD4F5 at (0, -0.8);
```

#### `master.moa` (Main Composition)
```moa
// Top-of-file imports (automatically placed at the top)
import "assets/card.moa" as card at (0, 0);

// Setup background coordinate grid
number_plane grid x_step=1.0 y_step=1.0 color=#4A009933;

// Sequence scene animations
play showCreation(grid) 1.5s smooth;
wait 0.3s;

// Composite asset animation: animates all sub-objects of 'card' as a group
play drawBorderThenFill(card) 1.8s smooth;
play circumscribe(card) 1.2s;
wait 1.0s;

// Camera tilt into 3D
camera polar phi=1.1 theta=0.6 gamma=0.0;
play rotate(card) 2.0s angle=3.14 smooth;
wait 0.5s;

play fadeOut(card) 1.0s;
```

---

## 📁 Workspace Architecture

The repository is organized into focused, modular Rust crates:

```
mathoxide/
├── crates/
│   ├── mo-core           # Mathematical foundations, VGroup geometry, calculus solvers,
│   │                     #   multi-font TrueType parser, FrameMemo LRU cache, Player
│   ├── mo-render         # WebGPU pipeline (wgpu 24), WGSL shaders, buffer cache, staging ring
│   ├── mo-dsl            # .moa language parser, diagnostics, file-relative import resolution
│   ├── mo-moab           # Compact binary container format (.moab) via postcard
│   ├── mo-ide            # Studio GUI (egui/eframe): Explorer, Editor, Viewport, Timeline
│   ├── mo-interactive    # Standalone interactive winit/wgpu preview window
│   ├── mo-cli            # Command-line binary (`mo`): validate, render MP4/PNG, inspect
│   └── mo-mcp            # JSON-RPC stdio Model Context Protocol server for AI coding agents
├── shaders/              # WGSL vector stroke, fill, and surface shaders (MIT licensed)
├── Cargo.toml            # Workspace definition & profile configurations
├── HANDOFF.md            # Comprehensive internal engineering and architectural log
└── LICENSE               # MIT License & third-party attributions
```

---

## 🛠️ CLI Tool (`mo`)

The `mo-cli` package provides a standalone binary `mo` for headless workflows, continuous integration, and rapid rendering:

```bash
# Validate scene syntax and imports
cargo run -p mo-cli -- validate ~/mathoxide/my_project/master.moa

# Render full video to MP4
cargo run -p mo-cli -- render ~/mathoxide/my_project/master.moa -o output.mp4

# Render a single timestamp snapshot (e.g. at t = 4.5s) to PNG
cargo run -p mo-cli -- render ~/mathoxide/my_project/master.moa --frame 4.5 -o snapshot.png

# Compile project into a binary .moab container
cargo run -p mo-cli -- compile ~/mathoxide/my_project/master.moa -o bundle.moab

# Inspect objects and timeline tracks in a compiled .moab bundle
cargo run -p mo-cli -- inspect bundle.moab
```

---

## 🤖 MCP (AI Agent) Integration

MathOxide includes a built-in [Model Context Protocol](https://modelcontextprotocol.io/) server (`mo-mcp`) configured via `.mcp.json`. AI pair programmers and autonomous agents can interface directly with MathOxide via stdio:

* `list_assets`: Scans project directories and reports assets, aliases, and locations.
* `get_timeline`: Retrieves structured timeline statements with exact start times and durations.
* `assemble_scene`: Generates or edits `.moa` compositions programmatically.

To test the MCP server manually:
```bash
printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_assets","arguments":{}}}\n' \
  | cargo run -p mo-mcp
```

---

## 🚀 Getting Started

### Prerequisites
* [Rust toolchain](https://rustup.rs/) (version 1.80 or newer).
* Standard graphics drivers supporting Vulkan, Metal, or DirectX 12.
* Linux dependencies (if building on Debian/Ubuntu):
  ```bash
  sudo apt-get install build-essential libx11-dev libasound2-dev libudev-dev
  ```

### Build & Run
```bash
# Clone the repository
git clone https://github.com/arjun1k-dev/mathoxide-ide.git
cd mathoxide-ide

# Run all workspace unit tests (77 tests, 0 warnings)
cargo test --workspace

# Launch the Visual Studio Studio IDE
cargo run -p mo-ide --release
```

---

## 💙 Homage & Tribute to 3Blue1Brown (Grant Sanderson)

> *"Some of the most profound ideas in mathematics are visually intuitive when shown the right way."*

This entire project owes its existence and heartbeat to **Grant Sanderson ([3Blue1Brown](https://www.youtube.com/@3blue1brown))**.

Before 3Blue1Brown, mathematics often felt to so many of us like a wall of dry formulas, rigid mechanical procedures, and impenetrable abstraction. Grant changed everything. Through his peerless pedagogical genius, serene storytelling, and profound geometric intuition—from the groundbreaking *Essence of Linear Algebra* and *Essence of Calculus* series, to mind-bending explorations of neural networks, Fourier transforms, differential equations, and quaternions—he did something rare and extraordinary: **he made us fall in love with mathematics.** He revealed the living, breathing beauty behind the symbols.

Watching Grant's videos was the singular catalyst that ignited my passion for mathematics, transformed the way I understand abstract structures, and planted the spark of inspiration to create MathOxide. The visual language he invented through Manim didn't just illustrate concepts; it fundamentally transformed how mathematical ideas are visualized and communicated across the globe.

**MathOxide is, at its core, a love letter to that vision.** It is a heartfelt attempt to take the aesthetic elegance and animation philosophy Grant gifted to the world and rebuild it in the compiled, ultra-high-performance realm of pure Rust and WebGPU—bringing real-time 60 FPS interactivity, instant timeline scrubbing, and zero-setup tooling to mathematical creators everywhere.

To Grant: **Thank you.** Thank you for inspiring millions of minds, for demystifying the infinite, and for showing the world that mathematics is not just a calculation, but one of humanity's highest forms of art. MathOxide exists because of you.

---

## ⚖️ License & Attribution

MathOxide is open-source software licensed under the **[MIT License](LICENSE)**.

### Attribution & Acknowledgments
* **3Blue1Brown LLC (Grant Sanderson)**:
  MathOxide's shader mathematics and vector animation paradigm draw inspiration from [Manim](https://github.com/3b1b/manim) and [ManimGL](https://github.com/3b1b/manim). Portions of the WGSL shaders in `shaders/` are adapted from ManimGL under the MIT License (see `shaders/LICENSE.manim.md`).
* **Manim Community (Manim CE)**:
  Inspiration for the extended mathematical built-in primitives and calculus catalog.
* **Typst**:
  Modern mathematical layout and typesetting.
* **egui / eframe**:
  Immediate mode graphical user interface library for Rust.

---

<div align="center">
Developed with ❤️ by <a href="https://github.com/arjun1k-dev">Arjun</a> and MathOxide contributors.
</div>
