//! mo — MathOxide CLI: validate, render, compile, preview.

use anyhow::{bail, Context, Result};
use clap::{Parser as ClapParser, Subcommand};
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(ClapParser)]
#[command(name = "mo", about = "MathOxide CLI — math animation engine")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Validate a .moa file, print diagnostics.
    Validate { file: String },
    /// Render a .moa file to mp4 (via ffmpeg subprocess) or PNG at a time.
    Render {
        file: String,
        #[arg(short, long)]
        out: String,
        /// Render a single frame at this time (seconds) as PNG.
        #[arg(long)]
        frame: Option<f32>,
        #[arg(long, default_value = "1920")]
        width: u32,
        #[arg(long, default_value = "1080")]
        height: u32,
        #[arg(long, default_value = "60")]
        fps: u32,
        /// 3D Camera tilt / elevation in degrees (e.g. 60.0).
        #[arg(long, default_value = "0.0", allow_hyphen_values = true)]
        phi: f32,
        /// 3D Camera azimuth in degrees (e.g. -45.0).
        #[arg(long, default_value = "0.0", allow_hyphen_values = true)]
        theta: f32,
        /// 3D Camera roll in degrees.
        #[arg(long, default_value = "0.0", allow_hyphen_values = true)]
        gamma: f32,
    },
    /// Open an interactive preview window (space = play/pause, arrows = scrub,
    /// drag = pan, wheel = zoom, drag object = move it).
    Preview { file: String },
    /// Compile .moa into a .moab bundle.
    Compile {
        file: String,
        #[arg(short, long)]
        out: String,
    },
    /// Inspect a .moab bundle: timeline and final state.
    Inspect { file: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Validate { file } => {
            let src = std::fs::read_to_string(&file).context("read file")?;
            match mo_dsl::parse(&src) {
                Ok(tl) => {
                    println!("OK — {} statements, {:.2}s", tl.stmts.len(), tl.duration());
                }
                Err(diags) => {
                    for d in diags {
                        eprintln!("{}:{}:{}: {}", file, d.line, d.col, d.message);
                    }
                    std::process::exit(1);
                }
            }
        }
        Cmd::Render { file, out, frame, width, height, fps, phi, theta, gamma } => {
            let src = std::fs::read_to_string(&file).context("read file")?;
            let tl = mo_dsl::parse(&src).map_err(|d| anyhow::anyhow!("{d:?}"))?;
            let renderer = mo_render::Renderer::offscreen(width, height)?;
            let mut camera = mo_core::camera::Camera::default();
            camera.width = width as f32;
            camera.height = height as f32;
            camera.phi = phi.to_radians();
            camera.theta = theta.to_radians();
            camera.gamma = gamma.to_radians();

            match frame {
                Some(t) => {
                    let state = mo_core::scene::Player::state_at(&tl, t);
                    let frame_cam = mo_core::scene::Player::camera_at(&tl, t, &camera);
                    let img = renderer.render_with_camera(&state, &frame_cam);
                    img.save(&out).context("save png")?;
                    println!("wrote frame @{t}s -> {out}");
                }
                None => render_video(&renderer, &tl, &out, fps, &camera)?,
            }
        }
        Cmd::Preview { file } => {
            let src = std::fs::read_to_string(&file).context("read file")?;
            let tl = mo_dsl::parse(&src).map_err(|d| anyhow::anyhow!("{d:?}"))?;
            mo_interactive::preview(tl)?;
        }
        Cmd::Compile { file, out } => {
            let src = std::fs::read_to_string(&file).context("read file")?;
            let bytes = mo_moab::compile(&src)?;
            std::fs::write(&out, bytes)?;
            println!("compiled {file} -> {out}");
        }
        Cmd::Inspect { file } => {
            let bytes = std::fs::read(&file).context("read file")?;
            let (tl, state, meta) = mo_moab::open(&bytes)?;
            println!("generator: {}", meta.generator);
            println!("duration: {:.2}s, {} statements", tl.duration(), tl.stmts.len());
            println!("objects in final state:");
            for (id, g) in &state {
                let (mn, mx) = g.bounds();
                println!(
                    "  {id}: {} curves, bounds [{:.2},{:.2},{:.2}]..[{:.2},{:.2},{:.2}]",
                    g.curve_count(),
                    mn[0], mn[1], mn[2], mx[0], mx[1], mx[2]
                );
            }
        }
    }
    Ok(())
}

fn render_video(
    renderer: &mo_render::Renderer,
    tl: &mo_core::scene::Timeline,
    out: &str,
    fps: u32,
    camera: &mo_core::camera::Camera,
) -> Result<()> {
    let duration = tl.duration();
    if duration <= 0.0 {
        bail!("scene has zero duration");
    }
    let width = renderer.width();
    let height = renderer.height();
    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "rawvideo",
            "-pix_fmt", "rgba",
            "-s", &format!("{width}x{height}"),
            "-r", &fps.to_string(),
            "-i", "-",
            "-pix_fmt", "yuv420p",
            "-c:v", "libx264",
            "-preset", "fast",
            out,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .context("spawn ffmpeg (is it installed?)")?;
    let mut stdin = ffmpeg.stdin.take().unwrap();

    let total = (duration * fps as f32).ceil() as u32;
    for frame in 0..total {
        let t = frame as f32 / fps as f32;
        let state = mo_core::scene::Player::state_at(tl, t);
        // Camera animations (camera.orbit/zoom/move_to) animate the camera over time.
        let frame_cam = mo_core::scene::Player::camera_at(tl, t, camera);
        let img = renderer.render_with_camera(&state, &frame_cam);
        stdin.write_all(img.as_raw())?;
    }
    drop(stdin);
    ffmpeg.wait()?;
    println!("wrote {total} frames -> {out}");
    Ok(())
}
