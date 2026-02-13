mod app;
mod font;
mod gallery;
mod image_loader;
mod input;
mod protocols;
mod render;
mod status;
mod viewer;
mod wayland;

use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("Usage: rimg <file>... | rimg <directory>");
        eprintln!("  Supported formats: jpg, jpeg, png, gif, webp");
        eprintln!();
        eprintln!("Keys:");
        eprintln!("  n/Space      Next image");
        eprintln!("  p/Backspace  Previous image");
        eprintln!("  g/G          First/last image");
        eprintln!("  +/-/0        Zoom in/out/reset");
        eprintln!("  h/j/k/l      Pan (viewer) / Navigate (gallery)");
        eprintln!("  Enter        Toggle gallery mode");
        eprintln!("  q/Escape     Quit");
        process::exit(1);
    }

    let paths = image_loader::collect_paths(&args);

    if paths.is_empty() {
        eprintln!("Error: no supported image files found");
        process::exit(1);
    }

    let mut app = app::App::new(paths);
    app.run();
}
