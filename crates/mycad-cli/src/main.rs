mod view;

use clap::{Parser, Subcommand};
use mycad_build::build_bodies_from_features;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::tessellation::{
    merge_meshes, tessellate_solid_with, to_ascii_stl, TessellationOptions,
};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "mycad", version, about = "MyCad CAD kernel CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert a .mycad file to STL
    Export {
        /// Input .mycad file
        input: PathBuf,
        /// Output STL file
        #[arg(short, long)]
        output: PathBuf,
        /// Number of angular segments for curved surfaces (default: 32)
        #[arg(long)]
        segments: Option<usize>,
    },
    /// Start a local server and open the model in a browser
    View {
        /// Input .mycad file
        input: PathBuf,
        /// Port to listen on (default: 7878)
        #[arg(long, default_value_t = view::DEFAULT_PORT)]
        port: u16,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Export {
            input,
            output,
            segments,
        } => {
            if let Err(msg) = run_export(&input, &output, segments) {
                eprintln!("error: {msg}");
                process::exit(1);
            }
        }
        Commands::View { input, port } => {
            if let Err(msg) = view::run_view(&input, port) {
                eprintln!("error: {msg}");
                process::exit(1);
            }
        }
    }
}

fn run_export(
    input: &std::path::Path,
    output: &std::path::Path,
    segments: Option<usize>,
) -> Result<(), String> {
    let doc = Document::from_path(input)
        .map_err(|e| format!("failed to read {}: {e}", input.display()))?;

    let root = &doc.root_component;
    if root.reference.is_some() || !root.children.is_empty() {
        return Err("assembly/reference documents are not supported".to_string());
    }

    let mut gen = IdGenerator::new(0);
    let bodies = build_bodies_from_features(&root.features, &mut gen)
        .map_err(|e| format!("failed to build solid: {e}"))?;

    let opts = match segments {
        Some(n) => TessellationOptions::new(n, 1),
        None => TessellationOptions::default(),
    };

    let meshes: Vec<_> = bodies
        .live()
        .map(|b| tessellate_solid_with(&b.solid, &opts))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("failed to tessellate: {e}"))?;

    let mesh = merge_meshes(&meshes);
    let stl = to_ascii_stl(&mesh, "model");
    std::fs::write(output, stl)
        .map_err(|e| format!("failed to write {}: {e}", output.display()))?;
    Ok(())
}
