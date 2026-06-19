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
    /// Replace an existing feature's params (ID-stable).
    Edit {
        /// Input .engawa file
        input: PathBuf,
        /// Existing feature id to replace
        feature_id: String,
        /// Path to a YAML file containing the new Feature (must share id)
        feature: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Truncate the feature history at <feature_id>, removing it and all later features.
    Rollback {
        /// Input .engawa file
        input: PathBuf,
        /// Feature id to roll back to (this feature and all later ones are removed)
        feature_id: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Suppress a feature (mark as inert, but keep in history).
    Suppress {
        /// Input .engawa file
        input: PathBuf,
        /// Feature id to suppress
        feature_id: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Restore a suppressed feature.
    Restore {
        /// Input .engawa file
        input: PathBuf,
        /// Feature id to restore
        feature_id: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Move a feature to immediately before another feature.
    Reorder {
        /// Input .engawa file
        input: PathBuf,
        /// Feature id to move
        feature_id: String,
        /// Id of the feature to move this one immediately before
        #[arg(long)]
        before: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
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
        EntryOp::Edit {
            input,
            feature_id,
            feature,
            output,
            dry_run,
        } => {
            let doc = Document::from_path(&input)
                .map_err(|e| format!("failed to read {}: {e}", input.display()))?;
            let feature_yaml = std::fs::read_to_string(&feature)
                .map_err(|e| format!("failed to read feature file {}: {e}", feature.display()))?;
            let new_feature: Feature = serde_yaml::from_str(&feature_yaml)
                .map_err(|e| format!("failed to parse feature YAML: {e}"))?;
            let updated = engawa_build::FeatureCrud::edit(&doc, &feature_id, new_feature)
                .map_err(|e| format!("edit failed: {e}"))?;
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
        EntryOp::Rollback {
            input,
            feature_id,
            output,
            dry_run,
        } => {
            let doc = Document::from_path(&input)
                .map_err(|e| format!("failed to read {}: {e}", input.display()))?;
            let updated = engawa_build::FeatureCrud::rollback(&doc, &feature_id)
                .map_err(|e| format!("rollback failed: {e}"))?;
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
        EntryOp::Suppress {
            input,
            feature_id,
            output,
            dry_run,
        } => {
            let doc = Document::from_path(&input)
                .map_err(|e| format!("failed to read {}: {e}", input.display()))?;
            let updated = engawa_build::FeatureCrud::suppress(&doc, &feature_id, true)
                .map_err(|e| format!("suppress failed: {e}"))?;
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
        EntryOp::Restore {
            input,
            feature_id,
            output,
            dry_run,
        } => {
            let doc = Document::from_path(&input)
                .map_err(|e| format!("failed to read {}: {e}", input.display()))?;
            let updated = engawa_build::FeatureCrud::suppress(&doc, &feature_id, false)
                .map_err(|e| format!("restore failed: {e}"))?;
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
        EntryOp::Reorder {
            input,
            feature_id,
            before,
            output,
            dry_run,
        } => {
            let doc = Document::from_path(&input)
                .map_err(|e| format!("failed to read {}: {e}", input.display()))?;
            let updated = engawa_build::FeatureCrud::reorder(&doc, &feature_id, &before)
                .map_err(|e| format!("reorder failed: {e}"))?;
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
