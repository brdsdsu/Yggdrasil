use std::path::PathBuf;

use clap::Parser;

use viz::loader::{SceneInput, load};

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

    /// Use the multi-coordinate variant of the demo scene.
    ///
    /// Same layer topology as the default demo, but each object's 10 point cloud points
    /// are held by a single multi-coordinate node instead of 10 separate nodes.
    #[arg(long, conflicts_with = "file")]
    demo_multi: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    let input = match args.file {
        Some(path) => SceneInput::File(path),
        None if args.demo_multi => SceneInput::DemoMulti,
        None => SceneInput::Demo,
    };

    let sg = load(input)?;
    viz::run(sg);
    Ok(())
}
