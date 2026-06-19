mod view;

use clap::{Parser, Subcommand};
use engawa_build::build_assembly;
use engawa_format::{Document, Feature};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::tessellation::{
    merge_meshes, tessellate_solid_with, to_ascii_stl, TessellationOptions,
};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "engawa", version, about = "EngawaCAD CAD kernel CLI")]
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
    /// Feature history CRUD operations on a .engawa file
    Entry {
        #[command(subcommand)]
        op: EntryOp,
    },
}

#[derive(Subcommand)]
enum EntryOp {
    /// Insert a new feature at the given index in root_component.features
    Add {
        /// Input .engawa file
        input: PathBuf,
        /// Path to a YAML file containing a single Feature
        feature: PathBuf,
        /// Insert position (0 = head, len = tail)
        #[arg(long)]
        at: usize,
        /// Output path (default: overwrite input)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Don't write to disk; emit resulting YAML to stdout
        #[arg(long)]
        dry_run: bool,
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
        Commands::Entry { op } => {
            if let Err(msg) = run_entry(op) {
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

fn run_entry(op: EntryOp) -> Result<(), String> {
    match op {
        EntryOp::Add {
            input,
            feature,
            at,
            output,
            dry_run,
        } => {
            // Load input document
            let doc = Document::from_path(&input)
                .map_err(|e| format!("failed to read {}: {e}", input.display()))?;

            // Load feature from YAML file
            let feature_yaml = std::fs::read_to_string(&feature)
                .map_err(|e| format!("failed to read feature file {}: {e}", feature.display()))?;
            let new_feature: Feature = serde_yaml::from_str(&feature_yaml)
                .map_err(|e| format!("failed to parse feature YAML: {e}"))?;

            // Perform insert
            let updated = engawa_build::FeatureCrud::insert(&doc, new_feature, at)
                .map_err(|e| format!("insert failed: {e}"))?;

            // Serialize to YAML
            let yaml = updated
                .to_yaml()
                .map_err(|e| format!("failed to serialize document: {e}"))?;

            if dry_run {
                print!("{yaml}");
            } else {
                let output_path = output.as_ref().unwrap_or(&input);
                std::fs::write(output_path, yaml)
                    .map_err(|e| format!("failed to write {}: {e}", output_path.display()))?;
            }
            Ok(())
        }
    }
}
