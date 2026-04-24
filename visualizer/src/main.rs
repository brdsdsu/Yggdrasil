use std::path::PathBuf;

use clap::Parser;

use visualizer::loader::{SceneInput, load};

/// Interactive 3D visualizer for Yggdrasil scene graphs.
///
/// Pass a JSON file to visualize a serialized SceneGraph, or omit the argument
/// to explore the built-in 4-layer demo scene.
#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    /// Path to a JSON file containing a serialized SceneGraph.
    ///
    /// Produce this file from Rust with:
    /// ```
    /// std::fs::write("scene.json", serde_json::to_string(&scene_graph)?)?;
    /// ```
    file: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    let input = match args.file {
        Some(path) => SceneInput::File(path),
        None => SceneInput::Demo,
    };

    let sg = load(input)?;
    visualizer::run(sg);
    Ok(())
}
