use anyhow::{bail, Context, Result};
use mo_core::scene::{Player, Timeline};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Export the source code to a `.moab` binary bundle.
pub fn export_moab(code: &str, out_path: &Path) -> Result<()> {
    let bytes = mo_moab::compile(code).map_err(|e| anyhow::anyhow!("moab compile error: {}", e))?;
    std::fs::write(out_path, bytes).context("failed to write .moab file")?;
    Ok(())
}

/// Render the timeline to an MP4 video via ffmpeg.
pub fn export_mp4(
    timeline: &Timeline,
    out_path: &Path,
    width: u32,
    height: u32,
    fps: u32,
    mut progress_cb: impl FnMut(u32, u32),
) -> Result<()> {
    let duration = timeline.duration();
    if duration <= 0.0 {
        bail!("scene has zero duration");
    }

    let renderer = mo_render::Renderer::offscreen(width, height)
        .context("failed to create offscreen renderer")?;

    let out_str = out_path.to_str().context("invalid output path")?;

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
            out_str,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn ffmpeg (is it installed in PATH?)")?;

    let mut stdin = ffmpeg.stdin.take().expect("ffmpeg stdin");
    let total_frames = (duration * fps as f32).ceil() as u32;

    for frame in 0..total_frames {
        let t = (frame as f32) / (fps as f32);
        let state = Player::state_at(timeline, t);
        let mut encoder = renderer.device().create_command_encoder(&Default::default());
        let cam = renderer.default_camera();
        renderer.draw_scene_pass(&state, &cam, &mut encoder);
        renderer.queue().submit(Some(encoder.finish()));
        let rgba = renderer.read_target();
        stdin.write_all(&rgba).context("write frame to ffmpeg stdin")?;
        progress_cb(frame + 1, total_frames);
    }

    drop(stdin);
    let status = ffmpeg.wait().context("wait on ffmpeg")?;
    if !status.success() {
        bail!("ffmpeg exited with non-zero status: {status}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_moab_roundtrip() {
        let code = r#"
// Test export scene
circle c at (0,0) r=1.5 fill=#58C4DD33, stroke=blue, sw=4;
play showCreation(c) 1s smooth;
"#;
        let tmp = std::env::temp_dir().join("test_export.moab");
        export_moab(code, &tmp).expect("export moab");
        assert!(tmp.exists());

        let bytes = std::fs::read(&tmp).expect("read exported moab");
        let (tl, state, meta) = mo_moab::open(&bytes).expect("open moab");
        assert_eq!(tl.duration(), 1.0);
        assert!(state.contains_key("c"));
        assert!(meta.generator.contains("mathoxide"));

        let _ = std::fs::remove_file(tmp);
    }
}

