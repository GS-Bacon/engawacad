mod view;

use clap::{Parser, Subcommand};
use engawa_build::build_assembly;
use engawa_format::Document;
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::tessellation::{
    merge_meshes, tessellate_solid_with, to_ascii_stl, TessellationOptions,
};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "engawa", version, about = "MyCad CAD kernel CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert a .engawa file to STL
    Export {
        /// Input .engawa file
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
        /// Input .engawa file
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

    let base_dir = input.parent().unwrap_or(std::path::Path::new("."));
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, base_dir, &mut gen)
        .map_err(|e| format!("failed to build assembly: {e}"))?;

    let opts = match segments {
        Some(n) => TessellationOptions::new(n, 1),
        None => TessellationOptions::default(),
    };

    let meshes: Vec<_> = bodies
        .iter()
        .map(|b| tessellate_solid_with(&b.solid, &opts))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("failed to tessellate: {e}"))?;

    let mesh = merge_meshes(&meshes);
    let stl = to_ascii_stl(&mesh, "model");
    std::fs::write(output, stl)
        .map_err(|e| format!("failed to write {}: {e}", output.display()))?;
    Ok(())
}
