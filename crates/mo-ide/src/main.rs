mod app;
mod editor;
mod export;
mod inspector;
mod scene_tree;
mod timeline;
mod viewport;
mod watcher;

use app::MathOxideApp;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "mathoxide", about = "MathOxide IDE — interactive animation studio")]
struct Cli {
    /// Path to a .moa script or .moab bundle to open
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,
}

fn main() -> eframe::Result<()> {
    let args = Cli::parse();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("MathOxide IDE")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 600.0]),
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
