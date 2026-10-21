mod app;
mod completion;
mod assets;
mod editor;
mod ext;
mod export;
mod inspector;
mod timeline;
mod viewport;
mod watcher;

use app::MathOxideApp;
use clap::Parser;
use ext::eframe;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "mathoxide", about = "MathOxide IDE — interactive animation studio")]
struct Cli {
    /// Path to a .moa script or .moab bundle to open
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,
}

/// Embedded app icon (largest .ico frame decoded) for the window / taskbar.
fn load_window_icon() -> Option<std::sync::Arc<eframe::egui::IconData>> {
    const ICO_BYTES: &[u8] = include_bytes!("../assets/mo.ico");
    let rgba = crate::ext::image::load_from_memory(ICO_BYTES).ok()?.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(std::sync::Arc::new(eframe::egui::IconData {
        width,
        height,
        rgba: rgba.into_raw(),
    }))
}

fn main() -> eframe::Result<()> {
    let args = Cli::parse();

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("MathOxide IDE")
        .with_inner_size([1280.0, 800.0])
        .with_resizable(false);
    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "MathOxide IDE",
        options,
        Box::new(move |cc| {
            let app = MathOxideApp::new(cc, args.file)
                .expect("failed to initialize MathOxide IDE");
            Ok(Box::new(app))
        }),
    )
}
