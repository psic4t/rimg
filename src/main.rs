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

fn print_help() {
    println!("Usage: rimg [options] <file>... | rimg [options] <directory>");
    println!("  Supported formats: jpg, jpeg, png, gif, webp, bmp, tiff, tif, svg, avif, heic, heif, jxl");
    println!();
    println!("Options:");
    println!("  -h, --help   Show this help message");
    println!("  -w           Set image as wallpaper (wlr-layer-shell)");
    println!();
    println!("Keys:");
    println!("  n/Space      Next image");
    println!("  p/Backspace  Previous image");
    println!("  g/G          First/last image");
    println!("  +/-/0        Zoom in/out/reset");
    println!("  h/j/k/l      Pan when zoomed, h/l navigate otherwise (also arrows)");
    println!("  Shift+w      Toggle fit-to-window for small images");
    println!("  Ctrl+0       Display at actual size (1:1 pixels)");
    println!("  r/R          Rotate clockwise/counterclockwise");
    println!("  Enter        Toggle gallery mode");
    println!("  q/Escape     Quit");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        print_help();
        process::exit(1);
    }

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        process::exit(0);
    }

    // Parse -w flag
    let wallpaper_mode = args.iter().any(|a| a == "-w");
    let file_args: Vec<String> = args.into_iter().filter(|a| a != "-w").collect();

    if file_args.is_empty() {
        eprintln!("Error: no image files specified");
        process::exit(1);
    }

    let paths = image_loader::collect_paths(&file_args);

    if paths.is_empty() {
        eprintln!("Error: no supported image files found");
        process::exit(1);
    }

    let mut app = app::App::new(paths, wallpaper_mode);
    app.run();
}
