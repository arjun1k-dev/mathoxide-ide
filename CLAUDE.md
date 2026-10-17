# MathOxide

Rust rewrite of ManimGL (reference source: /home/arjun/Downloads/manim-master). Blazing-fast math animation engine + IDE.

## Layout

- `crates/mo-core` — engine: VGroup (quadratic bezier vector objects), geometry builders, animations (snapshot-lerp), stateless Player (scrub any time), camera, rate functions (ported from manim's rate_functions.py).
- `crates/mo-render` — wgpu renderer. Fill uses ManimGL's winding technique: fan triangle + curve triangle cut in uv space (y = x^2). Vendored manim shaders live in `shaders/` for reference (with license).
- `crates/mo-dsl` — the .moa declarative DSL (winnow parser → mo-core Timeline IR).
- `crates/mo-moab` — .moab compiled asset bundle: magic MOAB + version + postcard sections (editable Timeline IR + baked final-state geometry).
- `crates/mo-cli` — `mo` binary: validate / render (mp4 via ffmpeg pipe) / compile / inspect.
- `crates/mo-interactive`, `crates/mo-ide`, `crates/mo-mcp` — phases 2/3/5.

## Commands

```sh
cargo test --workspace
cargo run -p mo-cli -- render examples/demo.moa -o /tmp/demo.mp4
cargo run -p mo-cli -- render examples/demo.moa --frame 1.0 -o /tmp/f1.png
cargo run -p mo-cli -- compile examples/demo.moa -o /tmp/demo.moab
cargo run -p mo-cli -- inspect /tmp/demo.moab
```

## DSL

```moa
circle c at (0,0) r=2 fill=#58C4DD33, stroke=blue, sw=6;
rect s at (3,0) w=3 h=3;
play showCreation(c) 2s smooth;
wait 0.5s;
play transform(c, s) 1.5s;
```

Objects: circle (r, at), rect (w,h,at), line (from,to), text. Animations: showCreation, fadeIn, fadeOut, growFromCenter, rotate, transform. Easings: smooth (default), linear, there_and_back, rush_into, rush_from, slow_into, double_smooth, wiggle, exponential_decay.

## Conventions

- VMobject point layout mirrors manim: per subpath anchor/handle/.../anchor (2n+1 records), curve i = records 2i,2i+1,2i+2.
- All state evaluation is stateless (`Player::state_at(timeline, t)`) — powers scrubbing and the interactive window.
- wgsl shaders embedded in mo-render (`fill_embedded.wgsl`, `stroke_embedded.wgsl`); manim's originals in `shaders/` are reference only.
