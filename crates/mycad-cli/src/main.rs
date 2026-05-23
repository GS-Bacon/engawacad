use clap::{Parser, Subcommand};
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::build_solid_from_features;
use mycad_kernel::tessellation::{tessellate_solid, to_ascii_stl};
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
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Export { input, output } => {
            if let Err(msg) = run_export(&input, &output) {
                eprintln!("error: {msg}");
                process::exit(1);
            }
        }
    }
}

fn run_export(input: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
    let doc = Document::from_path(input)
        .map_err(|e| format!("failed to read {}: {e}", input.display()))?;
    let mut gen = IdGenerator::new(0);
    let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
        .map_err(|e| format!("failed to build solid: {e}"))?;
    let mesh = tessellate_solid(&solid);
    let stl = to_ascii_stl(&mesh, "model");
    std::fs::write(output, stl)
        .map_err(|e| format!("failed to write {}: {e}", output.display()))?;
    Ok(())
}
