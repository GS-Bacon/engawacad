use std::path::PathBuf;
use std::process::{Command, ExitCode};

use serde::Deserialize;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let task = args.first().map(|s| s.as_str()).unwrap_or("help");

    match task {
        "ci" => ci(),
        "web" => web(),
        "gen-ts" => gen_ts(),
        "acceptance" => acceptance(),
        "help" | "--help" | "-h" => {
            println!("Usage: cargo xtask <TASK>");
            println!();
            println!("Tasks:");
            println!("  ci          Run web build, then fmt check, clippy, tests, build, TS drift, web checks, release verification");
            println!("  web         Generate TypeScript types and build web frontend (vite build)");
            println!("  gen-ts      Generate TypeScript types to web/src/generated/");
            println!("  acceptance  Run acceptance tests (add --fuzz for fuzz harness)");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("Unknown task: {other}");
            eprintln!("Run `cargo xtask help` for available tasks.");
            ExitCode::FAILURE
        }
    }
}

fn web() -> ExitCode {
    let web_dir = workspace_root().join("web");

    println!("=== Generating TypeScript types ===");
    if gen_ts() != ExitCode::SUCCESS {
        eprintln!("FAILED: TypeScript type generation");
        return ExitCode::FAILURE;
    }

    let lockfile = web_dir.join("package-lock.json");
    if !lockfile.exists() {
        eprintln!("FAILED: package-lock.json not found in web/");
        return ExitCode::FAILURE;
    }

    let npm_steps: &[(&str, &[&str])] = &[
        ("Installing web dependencies", &["npm", "ci"]),
        ("TypeScript type check", &["npx", "tsc", "--noEmit"]),
        ("Running vitest", &["npx", "vitest", "run"]),
        (
            "Building web frontend (vite build)",
            &["npx", "vite", "build"],
        ),
    ];

    for (label, cmd) in npm_steps {
        println!("\n--- {label} ---");
        let status = Command::new(cmd[0])
            .args(&cmd[1..])
            .current_dir(&web_dir)
            .status()
            .expect("failed to execute command");

        if !status.success() {
            eprintln!("FAILED: {label}");
            return ExitCode::FAILURE;
        }
    }

    println!("\n=== Web build complete ===");
    ExitCode::SUCCESS
}

fn gen_ts() -> ExitCode {
    gen_ts_to(&workspace_root().join("web/src/generated"))
}

pub(crate) fn gen_ts_to(out_dir: &std::path::Path) -> ExitCode {
    use engawa_api::transport::BodyMesh;
    use engawa_api::ErrorResponse;
    use engawa_format::feature::EntityRef;
    use engawa_format::Document;
    use engawa_kernel::tessellation::TriangleMesh;
    use ts_rs::TS;

    if out_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(out_dir) {
            eprintln!("Failed to clean output directory: {e}");
            return ExitCode::FAILURE;
        }
    }
    if let Err(e) = std::fs::create_dir_all(out_dir) {
        eprintln!("Failed to create output directory: {e}");
        return ExitCode::FAILURE;
    }

    std::env::set_var("TS_RS_EXPORT_DIR", out_dir);
    let cfg = ts_rs::Config::from_env();

    type ExportFn = Box<dyn FnOnce(&ts_rs::Config) -> Result<(), ts_rs::ExportError>>;
    let roots: Vec<(&str, ExportFn)> = vec![
        ("Document", Box::new(Document::export_all)),
        ("EntityRef", Box::new(EntityRef::export_all)),
        ("TriangleMesh", Box::new(TriangleMesh::export_all)),
        ("BodyMesh", Box::new(BodyMesh::export_all)),
        ("ErrorResponse", Box::new(ErrorResponse::export_all)),
    ];

    for (name, export_fn) in roots {
        println!("Exporting {name}...");
        if let Err(e) = export_fn(&cfg) {
            eprintln!("Failed to export {name}: {e}");
            return ExitCode::FAILURE;
        }
    }

    println!("TypeScript types exported to {}", out_dir.display());
    ExitCode::SUCCESS
}

fn acceptance() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let fuzz = args.iter().any(|a| a == "--fuzz");
    let record = args.iter().any(|a| a == "--record");

    // --workers N  or  --workers=N
    let workers: String = args
        .windows(2)
        .find(|w| w[0] == "--workers")
        .map(|w| w[1].clone())
        .or_else(|| {
            args.iter()
                .find(|a| a.starts_with("--workers="))
                .map(|a| a.trim_start_matches("--workers=").to_owned())
        })
        .unwrap_or_else(|| "1".to_owned());

    println!("=== Running acceptance tests ===");
    let status = Command::new("cargo")
        .args([
            "test",
            "-p",
            "engawa-api",
            "--test",
            "post_features_acceptance",
        ])
        .status()
        .expect("failed to execute cargo test");
    if !status.success() {
        eprintln!("FAILED: acceptance tests");
        return ExitCode::FAILURE;
    }

    if fuzz {
        println!("\n=== Running fuzz tests ===");
        let status = Command::new("cargo")
            .args([
                "test",
                "-p",
                "engawa-api",
                "--test",
                "fuzz_features",
                "--",
                "--include-ignored",
            ])
            .status()
            .expect("failed to execute fuzz tests");
        if !status.success() {
            eprintln!("FAILED: fuzz tests");
            return ExitCode::FAILURE;
        }
    }

    let record_label = if record { " [recording]" } else { "" };
    println!("\n=== Running Playwright E2E tests (workers={workers}){record_label} ===");
    if which("npx").is_some() {
        let pw_web_dir = workspace_root().join("web");
        let mut cmd = Command::new("npx");
        cmd.args(["playwright", "test", "--workers", &workers])
            .current_dir(&pw_web_dir);
        if record {
            cmd.env("PLAYWRIGHT_VIDEO", "1");
        }
        let status = cmd.status().expect("failed to execute playwright test");
        if !status.success() {
            eprintln!("FAILED: Playwright E2E tests");
            if !record {
                return ExitCode::FAILURE;
            }
            // In record mode, continue to generate the promo video even when some tests
            // fail — those failures are expected (known kernel bugs) and should appear
            // as RED tiles in the dashboard video.
        }
        if record {
            tile_videos(&pw_web_dir);
        }
    } else {
        eprintln!("WARNING: npx not found — skipping Playwright E2E tests (install Node.js >= 20)");
    }

    println!("\n=== Acceptance tests passed ===");
    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------
// Playwright JSON report structures (partial deserialization)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct PwReport {
    suites: Vec<PwSuite>,
}

#[derive(Deserialize)]
struct PwSuite {
    #[serde(default)]
    specs: Vec<PwSpec>,
    #[serde(default)]
    suites: Vec<PwSuite>,
}

#[derive(Deserialize)]
struct PwSpec {
    title: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    tests: Vec<PwTest>,
}

#[derive(Deserialize)]
struct PwTest {
    #[serde(default)]
    results: Vec<PwResult>,
}

#[derive(Deserialize)]
struct PwResult {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    attachments: Vec<PwAttachment>,
}

#[derive(Deserialize)]
struct PwAttachment {
    name: String,
    #[serde(default)]
    path: Option<String>,
}

// ---------------------------------------------------------------------------
// Tile data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Stage {
    One,
    Two,
}

#[derive(Debug, Clone)]
struct TileInput {
    /// Short label for drawtext overlay (ASCII only)
    label: String,
    /// true = draw green border; false = red
    passed: bool,
    stage: Stage,
    video: PathBuf,
}

/// Extract stage from Playwright tags (`@stage1` / `@stage2`).
/// Falls back to scanning the title string if tags are empty.
fn classify(tags: &[String], title: &str) -> Option<Stage> {
    for tag in tags {
        if tag.contains("stage1") {
            return Some(Stage::One);
        }
        if tag.contains("stage2") {
            return Some(Stage::Two);
        }
    }
    // Fallback: look for @stage1/@stage2 in the title text
    if title.contains("@stage1") {
        return Some(Stage::One);
    }
    if title.contains("@stage2") {
        return Some(Stage::Two);
    }
    None
}

/// Recursively collect TileInput from the nested suite tree.
fn collect_tiles(suite: &PwSuite, out: &mut Vec<TileInput>) {
    for spec in &suite.specs {
        // Use the last result of the first test variant
        let Some(test) = spec.tests.first() else {
            continue;
        };
        let Some(result) = test.results.last() else {
            continue;
        };

        // Find the video attachment
        let video_path = result
            .attachments
            .iter()
            .find(|a| a.name == "video")
            .and_then(|a| a.path.as_deref())
            .map(PathBuf::from);
        let Some(video) = video_path else { continue }; // API tests have no video
        if !video.exists() {
            continue;
        }

        let passed = result.status.as_deref() == Some("passed");
        let stage = classify(&spec.tags, &spec.title);
        let Some(stage) = stage else { continue }; // untagged → skip

        // Shorten the label: strip @stageN suffix and leading test code
        let label = shorten_label(&spec.title);

        out.push(TileInput {
            label,
            passed,
            stage,
            video,
        });
    }
    for child in &suite.suites {
        collect_tiles(child, out);
    }
}

/// Parse a Playwright JSON report and return TileInput list.
/// Pure function — no I/O.
pub(crate) fn parse_report(json: &str) -> Result<Vec<TileInput>, String> {
    let report: PwReport =
        serde_json::from_str(json).map_err(|e| format!("JSON parse error: {e}"))?;
    let mut tiles = Vec::new();
    for suite in &report.suites {
        collect_tiles(suite, &mut tiles);
    }
    Ok(tiles)
}

/// Strip `@stageN` suffix and leading test ID (e.g. "T03 ") from a title.
pub(crate) fn shorten_label(title: &str) -> String {
    // Remove @stage1 / @stage2 annotation
    let s = title
        .replace("@stage1", "")
        .replace("@stage2", "")
        .trim()
        .to_owned();
    // Trim to at most 36 characters for drawtext legibility
    if s.chars().count() > 36 {
        let truncated: String = s.chars().take(34).collect();
        format!("{truncated}..")
    } else {
        s
    }
}

/// Escape a string for use in ffmpeg drawtext `text=` value.
/// Colons, backslashes, single-quotes, percent signs, and newlines must be escaped.
pub(crate) fn escape_drawtext(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '\\' => vec!['\\', '\\'],
            ':' => vec!['\\', ':'],
            '\'' => vec!['\\', '\''],
            '%' => vec!['%', '%'],
            '\n' | '\r' => vec![' '],
            other => vec![other],
        })
        .collect()
}

/// Find a suitable TrueType font for ffmpeg drawtext.
/// Returns None if no font is found (drawtext will be skipped).
pub(crate) fn find_font() -> Option<PathBuf> {
    let candidates = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ];
    candidates.iter().map(PathBuf::from).find(|p| p.exists())
}

/// Build the xstack layout string (pure: positions only, no filter_complex header).
/// Each position is `x_y` using cumulative `w0+w1+...` / `h0+h4+...` form.
pub(crate) fn build_xstack_layout(n: usize, cols: usize) -> String {
    let mut positions: Vec<String> = Vec::with_capacity(n);
    for i in 0..n {
        let col = i % cols;
        let row = i / cols;
        let x = if col == 0 {
            "0".to_owned()
        } else {
            (0..col)
                .map(|c| format!("w{c}"))
                .collect::<Vec<_>>()
                .join("+")
        };
        // xstack では N*h0 形式の乗算が効かないため h0+h1+...hN-1 で累積する
        let y = if row == 0 {
            "0".to_owned()
        } else {
            (0..row)
                .map(|r| format!("h{}", r * cols))
                .collect::<Vec<_>>()
                .join("+")
        };
        positions.push(format!("{x}_{y}"));
    }
    positions.join("|")
}

// Tile dimensions used for the per-clip scale/pad step.
const TILE_W: u32 = 480;
const TILE_H: u32 = 360;
const TILE_FPS: u32 = 30;
const TILE_COLS: usize = 4;
/// Border thickness in pixels for the pass/fail colour frame.
const BORDER_PX: u32 = 8;
/// Canvas dimensions for the final concatenated video.
/// Must be the same for every segment so concat succeeds.
const CANVAS_W: u32 = TILE_W * TILE_COLS as u32;
// Canvas height is computed at runtime from row count per stage.

/// Build a filter_complex string that normalises each input, applies a
/// pass/fail colour border + label, then assembles them into an xstack grid.
///
/// `slow` — if true, `setpts=2.0*PTS` is applied (stage2 slow-motion effect).
/// `font` — path to .ttf; if None the drawtext step is omitted.
pub(crate) fn build_tile_filtergraph(
    tiles: &[TileInput],
    cols: usize,
    w: u32,
    h: u32,
    fps: u32,
    slow: bool,
    font: Option<&std::path::Path>,
) -> String {
    let n = tiles.len();
    let mut parts: Vec<String> = Vec::new();

    for (i, tile) in tiles.iter().enumerate() {
        let label = escape_drawtext(&tile.label);
        // Green for pass, red for fail
        let border_color = if tile.passed { "0x4CAF50" } else { "0xE53935" };

        let mut chain = format!(
            "[{i}:v] scale={w}:{h}:force_original_aspect_ratio=decrease,\
             pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:black,\
             fps={fps},\
             setsar=1"
        );
        if slow {
            chain.push_str(",setpts=2.0*PTS");
        }
        // Longest tile determines final duration — pad shorter ones
        chain.push_str(",tpad=stop_mode=clone:stop_duration=0");
        // Pass/fail border
        chain.push_str(&format!(
            ",drawbox=x=0:y=0:w=iw:h=ih:color={border_color}@1:t={BORDER_PX}"
        ));
        // Label (only if font is available)
        if let Some(font_path) = font {
            let font_str = font_path.to_str().unwrap_or("");
            chain.push_str(&format!(
                ",drawtext=fontfile='{font_str}':text='{label}'\
                 :x=10:y=10:fontsize=18:fontcolor=white\
                 :box=1:boxcolor=black@0.6:boxborderw=4"
            ));
        }
        chain.push_str(&format!(" [v{i}]"));
        parts.push(chain);
    }

    if n == 1 {
        // Single tile: just output directly
        return parts.remove(0).replace(" [v0]", " [out]");
    }

    let layout = build_xstack_layout(n, cols);
    let input_labels: String = (0..n).map(|i| format!("[v{i}]")).collect();
    parts.push(format!(
        "{input_labels} xstack=inputs={n}:layout={layout}:fill=black [out]"
    ));
    parts.join(";\n")
}

/// Generate a title card mp4 via ffmpeg lavfi (no input image required).
fn build_title_card(
    text: &str,
    canvas_w: u32,
    canvas_h: u32,
    fps: u32,
    duration_secs: f32,
    font: Option<&std::path::Path>,
    output: &std::path::Path,
) -> bool {
    let size = format!("{canvas_w}x{canvas_h}");
    let mut vf_parts = vec![format!("fps={fps},setsar=1,format=yuv420p")];
    if let Some(fp) = font {
        let esc = escape_drawtext(text);
        let font_str = fp.to_str().unwrap_or("");
        vf_parts.push(format!(
            "drawtext=fontfile='{font_str}':text='{esc}'\
             :x=(w-tw)/2:y=(h-th)/2:fontsize=36:fontcolor=white\
             :box=1:boxcolor=black@0.4:boxborderw=8"
        ));
    }
    let vf = vf_parts.join(",");
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("color=c=0x1A1A2E:s={size}:d={duration_secs}"),
            "-vf",
            &vf,
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            output.to_str().unwrap(),
        ])
        .status();
    matches!(status, Ok(s) if s.success())
}

/// Render a single stage's tiles into a padded mp4 on the shared canvas.
fn run_stage(
    tiles: &[TileInput],
    canvas_w: u32,
    canvas_h: u32,
    fps: u32,
    slow: bool,
    font: Option<&std::path::Path>,
    output: &std::path::Path,
) -> bool {
    if tiles.is_empty() {
        return false;
    }
    let n = tiles.len();
    println!(
        "  Building stage grid: {} tile(s), canvas {}x{}",
        n, canvas_w, canvas_h
    );

    // Build xstack grid
    let grid_tmp = output.with_extension("grid.mp4");
    {
        let filter = build_tile_filtergraph(tiles, TILE_COLS, TILE_W, TILE_H, fps, slow, font);
        let mut args: Vec<String> = vec!["-y".into()];
        for tile in tiles {
            args.push("-i".into());
            args.push(tile.video.to_str().unwrap().to_owned());
        }
        args.extend([
            "-filter_complex".into(),
            filter,
            "-map".into(),
            "[out]".into(),
            "-c:v".into(),
            "libx264".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            grid_tmp.to_str().unwrap().to_owned(),
        ]);
        let status = Command::new("ffmpeg").args(&args).status();
        if !matches!(status, Ok(s) if s.success()) {
            eprintln!("WARNING: ffmpeg stage grid 生成に失敗しました");
            return false;
        }
    }

    // Pad grid to the shared canvas size (different stages may have different
    // row counts, which would make their grids different heights)
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            grid_tmp.to_str().unwrap(),
            "-vf",
            &format!(
                "scale={canvas_w}:{canvas_h}:force_original_aspect_ratio=decrease,\
                 pad={canvas_w}:{canvas_h}:(ow-iw)/2:(oh-ih)/2:0x111111,\
                 setsar=1,fps={fps},format=yuv420p"
            ),
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            output.to_str().unwrap(),
        ])
        .status();

    let _ = std::fs::remove_file(&grid_tmp);
    matches!(status, Ok(s) if s.success())
}

/// Concatenate segment files into a single mp4.
fn concat_segments(segments: &[PathBuf], output: &std::path::Path) -> bool {
    let n = segments.len();
    if n == 0 {
        return false;
    }
    if n == 1 {
        return std::fs::copy(&segments[0], output)
            .map(|_| true)
            .unwrap_or(false);
    }

    let mut args: Vec<String> = vec!["-y".into()];
    for seg in segments {
        args.push("-i".into());
        args.push(seg.to_str().unwrap().to_owned());
    }
    let filter: String = (0..n).map(|i| format!("[{i}:v]")).collect::<String>()
        + &format!("concat=n={n}:v=1:a=0[out]");
    args.extend([
        "-filter_complex".into(),
        filter,
        "-map".into(),
        "[out]".into(),
        "-c:v".into(),
        "libx264".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        output.to_str().unwrap().to_owned(),
    ]);
    let status = Command::new("ffmpeg").args(&args).status();
    matches!(status, Ok(s) if s.success())
}

fn tile_videos(web_dir: &std::path::Path) {
    let Some(_ffmpeg) = which("ffmpeg") else {
        eprintln!("WARNING: ffmpeg が見つかりません — タイル合成をスキップします");
        return;
    };

    let results_dir = web_dir.join("test-results");
    let report_path = results_dir.join("report.json");

    if !report_path.exists() {
        eprintln!(
            "WARNING: test-results/report.json が見つかりません — タイル合成をスキップします"
        );
        return;
    }

    let json = match std::fs::read_to_string(&report_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("WARNING: report.json 読み込みエラー: {e}");
            return;
        }
    };

    let tiles = match parse_report(&json) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("WARNING: report.json パースエラー: {e}");
            return;
        }
    };

    let stage1: Vec<&TileInput> = tiles.iter().filter(|t| t.stage == Stage::One).collect();
    let stage2: Vec<&TileInput> = tiles.iter().filter(|t| t.stage == Stage::Two).collect();

    println!(
        "\n=== Tiling {} stage-1 and {} stage-2 video(s) with FFmpeg ===",
        stage1.len(),
        stage2.len()
    );

    if stage1.is_empty() && stage2.is_empty() {
        eprintln!("WARNING: @stage1/@stage2 タグ付き録画が見つかりませんでした");
        return;
    }

    let font = find_font();
    if font.is_none() {
        eprintln!("INFO: フォントが見つかりません — ラベルなしで合成します");
    }

    // Compute canvas height from the larger stage's row count
    let max_tiles = stage1.len().max(stage2.len()).max(1);
    let rows = max_tiles.div_ceil(TILE_COLS);
    let canvas_h = TILE_H * rows as u32;
    let canvas_w = CANVAS_W;

    let mut segments: Vec<PathBuf> = Vec::new();
    let tmp_dir = results_dir.join("_promo_tmp");
    let _ = std::fs::create_dir_all(&tmp_dir);

    // Stage 1
    if !stage1.is_empty() {
        let title_path = tmp_dir.join("title1.mp4");
        let stage_tiles: Vec<TileInput> = stage1.iter().map(|t| (*t).clone()).collect();
        let stage_path = tmp_dir.join("stage1.mp4");

        if build_title_card(
            "Stage 1 — Primitives & Booleans",
            canvas_w,
            canvas_h,
            TILE_FPS,
            2.5,
            font.as_deref(),
            &title_path,
        ) {
            segments.push(title_path);
        }

        if run_stage(
            &stage_tiles,
            canvas_w,
            canvas_h,
            TILE_FPS,
            false,
            font.as_deref(),
            &stage_path,
        ) {
            segments.push(stage_path);
        }
    }

    // Stage 2
    if !stage2.is_empty() {
        let title_path = tmp_dir.join("title2.mp4");
        let stage_tiles: Vec<TileInput> = stage2.iter().map(|t| (*t).clone()).collect();
        let stage_path = tmp_dir.join("stage2.mp4");

        if build_title_card(
            "Stage 2 — Extrude / Cut / Boolean",
            canvas_w,
            canvas_h,
            TILE_FPS,
            2.5,
            font.as_deref(),
            &title_path,
        ) {
            segments.push(title_path);
        }

        if run_stage(
            &stage_tiles,
            canvas_w,
            canvas_h,
            TILE_FPS,
            true, // slow-motion for stage2
            font.as_deref(),
            &stage_path,
        ) {
            segments.push(stage_path);
        }
    }

    if segments.is_empty() {
        eprintln!("WARNING: 生成できたセグメントがありません — 出力をスキップします");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return;
    }

    let output = results_dir.join("acceptance-promo.mp4");
    println!(
        "=== Concatenating {} segment(s) → {} ===",
        segments.len(),
        output.display()
    );

    if concat_segments(&segments, &output) {
        println!("Promo video saved to {}", output.display());
    } else {
        eprintln!("WARNING: concat に失敗しました");
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

fn ci() -> ExitCode {
    let Some(_node) = which("node") else {
        eprintln!("FAILED: Node.js (>=20) and npm are required for CI.");
        eprintln!("Install from https://nodejs.org/ or via nvm, then re-run `cargo xtask ci`.");
        return ExitCode::FAILURE;
    };

    // Web build first so cargo steps embed real assets
    println!("\n=== Building web frontend ===");
    if web() != ExitCode::SUCCESS {
        eprintln!("FAILED: Web frontend build");
        return ExitCode::FAILURE;
    }

    let cargo_steps: &[(&str, &[&str])] = &[
        (
            "Checking formatting",
            &["cargo", "fmt", "--all", "--", "--check"],
        ),
        (
            "Running clippy",
            &["cargo", "clippy", "--workspace", "--", "-D", "warnings"],
        ),
        ("Running tests", &["cargo", "test", "--workspace"]),
        ("Building", &["cargo", "build", "--workspace"]),
    ];

    for (label, cmd) in cargo_steps {
        println!("\n=== {label} ===");
        let status = Command::new(cmd[0])
            .args(&cmd[1..])
            .status()
            .expect("failed to execute command");

        if !status.success() {
            eprintln!("FAILED: {label}");
            return ExitCode::FAILURE;
        }
    }

    println!("\n=== Checking TS drift ===");
    let tracked = Command::new("git")
        .args(["ls-files", "web/src/generated/"])
        .output()
        .expect("failed to run git ls-files");
    if tracked.stdout.is_empty() {
        println!("No committed TS files found — skipping drift check (run `cargo xtask gen-ts` and commit the results).");
    } else {
        let output = Command::new("git")
            .args([
                "status",
                "--porcelain",
                "--untracked-files=all",
                "--",
                "web/src/generated/",
            ])
            .output()
            .expect("failed to run git status");

        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.trim().is_empty() {
            eprintln!("FAILED: TypeScript types are out of sync with committed versions");
            eprintln!("Run `cargo xtask gen-ts` and commit the results.");
            eprintln!("Drift detected:\n{stdout}");
            return ExitCode::FAILURE;
        }
    }

    // Release verification: ensure release build embeds real assets
    println!("\n=== Release verification ===");
    println!("Building release binary...");
    let status = Command::new("cargo")
        .args(["build", "-p", "engawa-cli", "--release"])
        .status()
        .expect("failed to execute cargo build --release");
    if !status.success() {
        eprintln!("FAILED: Release build");
        return ExitCode::FAILURE;
    }

    println!("Running release smoke test...");
    let status = Command::new("cargo")
        .args(["test", "-p", "engawa-api", "--release", "static_assets"])
        .status()
        .expect("failed to execute release smoke test");
    if !status.success() {
        eprintln!("FAILED: Release smoke test");
        return ExitCode::FAILURE;
    }

    // Playwright end-to-end tests
    println!("\n=== Running Playwright tests ===");
    let Some(_npx) = which("npx") else {
        eprintln!("FAILED: 'npx' not found. Ensure Node.js >= 20 is installed.");
        return ExitCode::FAILURE;
    };
    let pw_web_dir = workspace_root().join("web");
    let pw_status = Command::new("npx")
        .args(["playwright", "test"])
        .current_dir(&pw_web_dir)
        .status()
        .expect("failed to execute playwright test");
    if !pw_status.success() {
        eprintln!("FAILED: Playwright tests");
        return ExitCode::FAILURE;
    }

    // 3ai シェルスクリプトのユニットテスト
    println!("\n=== Running 3ai shell tests ===");
    let Some(bats) = which("bats") else {
        eprintln!("FAILED: 'bats' not found. Install with: sudo apt install bats");
        return ExitCode::FAILURE;
    };
    let tests_dir = workspace_root().join("tests/3ai");
    let status = Command::new(bats)
        .args(["dispatch-glm.bats", "state.bats", "guard-crates.bats"])
        .current_dir(&tests_dir)
        .status()
        .expect("failed to execute bats");
    if !status.success() {
        eprintln!("FAILED: 3ai shell tests");
        return ExitCode::FAILURE;
    }

    // #261 F01 (Codex r2 high): .claude/ 配下の bun:test は repo root の `bun test`
    // 自動探索に乗らないため、ここで明示実行して回帰スイートに組み込む。
    println!("\n=== Running 3ailoop bun tests ===");
    let Some(bun) = which("bun") else {
        eprintln!("FAILED: 'bun' not found. Install: https://bun.sh/");
        return ExitCode::FAILURE;
    };
    let bun_status = Command::new(&bun)
        .args([
            "test",
            "./.claude/skills/3ailoop/scripts/loop-split-detector.test.ts",
            // #262: dispatch-glm-review.ts の dispatch-failure 検出回帰スイート。
            "./.claude/skills/3ai/scripts/__tests__/dispatch-glm-review.test.ts",
            // ADR-013 自動 accept フロー (regen-tracker / pause-detector rescan /
            // decision-matrix-lint / auto-accept) を回帰スイートに組み込む。
            "./.claude/skills/3ailoop/scripts/loop-adr-regen-tracker.test.ts",
            "./.claude/skills/3ailoop/scripts/loop-adr-pause-detector.test.ts",
            "./.claude/skills/3ailoop/scripts/loop-adr-decision-matrix-lint.test.ts",
            "./.claude/skills/3ailoop/scripts/loop-adr-auto-accept.test.ts",
            // #284: pause-streak tracker と cycle-record の same-state pause 検出を回帰スイートへ。
            "./.claude/skills/3ailoop/scripts/loop-pause-streak-tracker.test.ts",
            "./.claude/skills/3ailoop/scripts/loop-cycle-record.test.ts",
            // #285: watcher の段階通知 (decideWatcherPauseWarning) を回帰スイートへ。
            "./.claude/skills/3ailoop/scripts/loop-tmux-watcher.test.ts",
        ])
        .current_dir(workspace_root())
        .status()
        .expect("failed to execute bun test");
    if !bun_status.success() {
        eprintln!("FAILED: 3ailoop bun tests");
        return ExitCode::FAILURE;
    }

    println!("\n=== All CI checks passed ===");
    ExitCode::SUCCESS
}

fn workspace_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    PathBuf::from(manifest_dir)
        .parent()
        .expect("no parent")
        .parent()
        .expect("no grandparent")
        .to_owned()
}

fn which(name: &str) -> Option<PathBuf> {
    let paths = std::env::var("PATH").ok()?;
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, OnceLock};

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn export_to_temp_dir() -> PathBuf {
        let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

        use engawa_api::transport::BodyMesh;
        use engawa_api::ErrorResponse;
        use engawa_format::feature::EntityRef;
        use engawa_format::Document;
        use engawa_kernel::tessellation::TriangleMesh;
        use ts_rs::TS;

        let dir = tempfile::tempdir().unwrap().keep();
        std::env::set_var("TS_RS_EXPORT_DIR", &dir);
        let cfg = ts_rs::Config::from_env();

        Document::export_all(&cfg).unwrap();
        EntityRef::export_all(&cfg).unwrap();
        TriangleMesh::export_all(&cfg).unwrap();
        BodyMesh::export_all(&cfg).unwrap();
        ErrorResponse::export_all(&cfg).unwrap();

        dir
    }

    fn read_generated(dir: &std::path::Path, name: &str) -> String {
        fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("failed to read {name}: {e}"))
    }

    #[test]
    fn t01_determinism() {
        let dir1 = export_to_temp_dir();
        let dir2 = export_to_temp_dir();

        let files = [
            "Document.ts",
            "Component.ts",
            "Feature.ts",
            "SketchPlane.ts",
            "SketchElement.ts",
            "Transform.ts",
            "ComponentRef.ts",
            "EntityKind.ts",
            "EntityRef.ts",
            "TriangleMesh.ts",
            "BodyMesh.ts",
            "ErrorResponse.ts",
        ];

        for name in &files {
            let c1 = read_generated(&dir1, name);
            let c2 = read_generated(&dir2, name);
            assert_eq!(c1, c2, "{name} differs between runs");
        }
    }

    static FEATURE_GOLDEN: &str = r#"// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { PlaneRef } from "./PlaneRef";
import type { SketchElement } from "./SketchElement";
import type { SketchPlane } from "./SketchPlane";
import type { Variable } from "./Variable";

/**
 * A feature — one step in the operation history.
 */
export type Feature = { "type": "create_box", id: string, width: number, height: number, depth: number, suppressed?: boolean, } | { "type": "create_cylinder", id: string, radius: number, height: number, origin?: [number, number, number], suppressed?: boolean, } | { "type": "create_sphere", id: string, radius: number, center?: [number, number, number], suppressed?: boolean, } | { "type": "create_sketch", id: string, plane: SketchPlane, offset?: number, variables?: Array<Variable>, profile: Array<SketchElement>, plane_ref?: PlaneRef | null, suppressed?: boolean, } | { "type": "extrude", id: string, sketch: string, depth: number, fuse_target?: string | null, suppressed?: boolean, } | { "type": "extrude_cut", id: string, sketch: string, depth: number, target: string, suppressed?: boolean, } | { "type": "cut", id: string, target: string, tool: string, suppressed?: boolean, } | { "type": "fuse", id: string, target: string, tool: string, suppressed?: boolean, } | { "type": "intersect", id: string, target: string, tool: string, suppressed?: boolean, } | { "type": "sketch_offset", id: string, 
/**
 * Reference to CreateSketch.id
 */
sketch: string, 
/**
 * Element IDs to offset; empty = all elements
 */
selection?: Array<string>, 
/**
 * Signed offset distance (positive = left/outer, negative = right/inner)
 */
distance: number, suppressed?: boolean, };
"#;

    #[test]
    fn t02_feature_tagged_union() {
        let dir = export_to_temp_dir();
        let actual = read_generated(&dir, "Feature.ts");
        assert_eq!(actual, FEATURE_GOLDEN);

        assert!(
            actual.contains(r#""type": "create_box""#),
            "missing create_box tag"
        );
        assert!(
            actual.contains(r#""type": "create_cylinder""#),
            "missing create_cylinder tag"
        );
        assert!(
            actual.contains(r#""type": "create_sphere""#),
            "missing create_sphere tag"
        );
        assert!(
            actual.contains(r#""type": "create_sketch""#),
            "missing create_sketch tag"
        );
        assert!(
            actual.contains(r#""type": "extrude""#),
            "missing extrude tag"
        );
        assert!(
            actual.contains(r#""type": "extrude_cut""#),
            "missing extrude_cut tag"
        );
        assert!(actual.contains(r#""type": "cut""#), "missing cut tag");
        assert!(actual.contains(r#""type": "fuse""#), "missing fuse tag");
        assert!(
            actual.contains(r#""type": "intersect""#),
            "missing intersect tag"
        );
        assert!(
            actual.contains(r#""type": "sketch_offset""#),
            "missing sketch_offset tag"
        );
    }

    static TRIANGLE_MESH_GOLDEN: &str = "// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.\n\n/**\n * A triangle mesh for rendering.\n */\nexport type TriangleMesh = { \n/**\n * Vertex positions (x, y, z).\n */\npositions: Array<[number, number, number]>, \n/**\n * Normal vectors per vertex.\n */\nnormals: Array<[number, number, number]>, \n/**\n * Triangle indices (every 3 indices form a triangle).\n */\nindices: Array<number>, \n/**\n * Per-triangle face id string. Length always equals `triangle_count()`.\n * Unnamed faces (`Face.name == None`) produce an empty string.\n */\nface_ids: Array<string>, };\n";

    #[test]
    fn t03_triangle_mesh() {
        let dir = export_to_temp_dir();
        let actual = read_generated(&dir, "TriangleMesh.ts");
        assert_eq!(actual, TRIANGLE_MESH_GOLDEN);
    }

    static DOCUMENT_GOLDEN: &str = "// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.\nimport type { Component } from \"./Component\";\nimport type { Variable } from \"./Variable\";\n\n/**\n * The top-level document representing a EngawaCAD design file.\n */\nexport type Document = { \n/**\n * Format schema version. Increment when the .engawa file format changes in a breaking way.\n */\nschema_version: number, \n/**\n * Kernel version that created this document.\n */\nversion: string, variables?: Array<Variable>, \n/**\n * The root component (assembly or single part).\n */\nroot_component: Component, };\n";

    static COMPONENT_GOLDEN: &str = "// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.\nimport type { ComponentRef } from \"./ComponentRef\";\nimport type { Feature } from \"./Feature\";\nimport type { RefPlane } from \"./RefPlane\";\nimport type { Transform } from \"./Transform\";\n\n/**\n * A component in the design hierarchy.\n * Can contain features (inline part definition), children (sub-components),\n * or be a reference to an external file/library.\n */\nexport type Component = { \n/**\n * Human-readable name.\n */\nname: string, \n/**\n * Transform relative to the parent component.\n */\ntransform?: Transform, \n/**\n * External reference (if this component is not defined inline).\n */\nref?: ComponentRef | null, \n/**\n * Ordered list of features (the operation history).\n */\nfeatures?: Array<Feature>, \n/**\n * Child components.\n */\nchildren?: Array<Component>, \n/**\n * Reference planes for sketch creation.\n * If omitted during serialization, defaults to the canonical three (Front, Top, Right).\n */\nref_planes?: Array<RefPlane>, };\n";

    #[test]
    fn t04_document_and_component() {
        let dir = export_to_temp_dir();
        let doc = read_generated(&dir, "Document.ts");
        assert_eq!(doc, DOCUMENT_GOLDEN);

        let comp = read_generated(&dir, "Component.ts");
        assert_eq!(comp, COMPONENT_GOLDEN);

        assert!(comp.contains("transform?:"), "transform should be optional");
        assert!(
            comp.contains("ref?:"),
            "ref field should be renamed and optional"
        );
        assert!(comp.contains("features?:"), "features should be optional");
        assert!(comp.contains("children?:"), "children should be optional");
        assert!(
            comp.contains("ref_planes?:"),
            "ref_planes should be optional"
        );
    }

    static COMPONENT_REF_GOLDEN: &str = r#"// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.

export type ComponentRef = string;
"#;

    #[test]
    fn t05_component_ref_is_string() {
        let dir = export_to_temp_dir();
        let actual = read_generated(&dir, "ComponentRef.ts");
        assert_eq!(actual, COMPONENT_REF_GOLDEN);
    }

    static ERROR_RESPONSE_GOLDEN: &str = r#"// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.

export type ErrorResponse = { error: string, };
"#;

    #[test]
    fn t09_api_transport_dto() {
        let dir = export_to_temp_dir();
        let err_resp = read_generated(&dir, "ErrorResponse.ts");
        assert_eq!(err_resp, ERROR_RESPONSE_GOLDEN);
    }

    static ENTITY_REF_GOLDEN: &str = "// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.\nimport type { EntityKind } from \"./EntityKind\";\n\n/**\n * A reference to a topological entity, either directly named or derived from an operation.\n */\nexport type EntityRef = { \"ref\": \"named\", feature_id: string, kind: EntityKind, role: string, } | { \"ref\": \"derived\", kind: EntityKind, op: string, from: Array<EntityRef>, selector: string, };\n";

    #[test]
    fn t10_entity_ref_explicit_root() {
        let dir = export_to_temp_dir();
        let actual = read_generated(&dir, "EntityRef.ts");
        assert_eq!(actual, ENTITY_REF_GOLDEN);
    }
    static BODY_MESH_GOLDEN: &str = "// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.\nimport type { TriangleMesh } from \"./TriangleMesh\";\n\nexport type BodyMesh = { feature_id: string, mesh: TriangleMesh, };\n";

    #[test]
    fn t15_body_mesh_golden() {
        let dir = export_to_temp_dir();
        let actual = read_generated(&dir, "BodyMesh.ts");
        assert_eq!(actual, BODY_MESH_GOLDEN);
    }

    #[test]
    fn t06_gen_ts_produces_all_files() {
        let dir = export_to_temp_dir();
        let expected = [
            "Document.ts",
            "Component.ts",
            "Feature.ts",
            "Transform.ts",
            "ComponentRef.ts",
            "EntityKind.ts",
            "EntityRef.ts",
            "TriangleMesh.ts",
            "BodyMesh.ts",
            "ErrorResponse.ts",
        ];
        for name in &expected {
            let path = dir.join(name);
            assert!(path.exists(), "expected file {name} to be generated");
            let content = fs::read_to_string(&path).unwrap();
            assert!(!content.is_empty(), "{name} should not be empty");
        }
    }

    #[test]
    fn t01_determinism_100_runs() {
        let dir1 = export_to_temp_dir();
        let reference_files: Vec<(String, String)> = [
            "Document.ts",
            "Component.ts",
            "Feature.ts",
            "SketchPlane.ts",
            "SketchElement.ts",
            "Transform.ts",
            "ComponentRef.ts",
            "EntityKind.ts",
            "EntityRef.ts",
            "TriangleMesh.ts",
            "ErrorResponse.ts",
            "BodyMesh.ts",
        ]
        .iter()
        .map(|name| (name.to_string(), read_generated(&dir1, name)))
        .collect();

        for i in 0..100 {
            let dir = export_to_temp_dir();
            for (name, expected) in &reference_files {
                let actual = read_generated(&dir, name);
                assert_eq!(actual, *expected, "run {i}: {name} differs");
            }
        }
    }

    /// T07: gen_ts_to() が workspace_root / web/src/generated を経由せず
    /// 指定ディレクトリへ実際にファイルを書き出すことを検証する（実経路テスト）。
    #[test]
    fn t07_real_path_gen_ts_to() {
        let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let code = gen_ts_to(dir.path());
        assert_eq!(code, ExitCode::SUCCESS, "gen_ts_to should succeed");
        for name in &[
            "Document.ts",
            "Feature.ts",
            "TriangleMesh.ts",
            "BodyMesh.ts",
            "ErrorResponse.ts",
        ] {
            assert!(
                dir.path().join(name).exists(),
                "{name} missing from gen_ts_to output"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Promo pipeline pure-function tests
    // -----------------------------------------------------------------------

    #[test]
    fn t_xstack_layout_basic() {
        // 4 tiles, 4 cols → single row
        let layout = build_xstack_layout(4, 4);
        assert_eq!(layout, "0_0|w0_0|w0+w1_0|w0+w1+w2_0");
    }

    #[test]
    fn t_xstack_layout_two_rows() {
        // 5 tiles, 4 cols → 2 rows
        let layout = build_xstack_layout(5, 4);
        // Tile 4 is row 1, col 0 → x=0, y=h0
        assert!(layout.ends_with("|0_h0"));
    }

    #[test]
    fn t_xstack_layout_single() {
        let layout = build_xstack_layout(1, 4);
        assert_eq!(layout, "0_0");
    }

    #[test]
    fn t_escape_drawtext_colon() {
        assert_eq!(escape_drawtext("T01: simple_box"), "T01\\: simple_box");
    }

    #[test]
    fn t_escape_drawtext_percent() {
        assert_eq!(escape_drawtext("50%"), "50%%");
    }

    #[test]
    fn t_escape_drawtext_backslash() {
        assert_eq!(escape_drawtext("a\\b"), "a\\\\b");
    }

    #[test]
    fn t_escape_drawtext_newline() {
        assert_eq!(escape_drawtext("a\nb"), "a b");
    }

    #[test]
    fn t_shorten_label_strips_stage_tag() {
        assert_eq!(
            shorten_label("T03 console no error - cylinder @stage1"),
            "T03 console no error - cylinder"
        );
    }

    #[test]
    fn t_shorten_label_truncates() {
        let long = "T03 console no error - boolean_intersect_cyl_sphere_extra_long @stage1";
        let result = shorten_label(long);
        assert!(
            result.chars().count() <= 38,
            "label should be ≤38 chars, got: {result}"
        );
        assert!(result.ends_with(".."));
    }

    #[test]
    fn t_classify_stage_tag() {
        assert_eq!(classify(&["@stage1".to_owned()], "title"), Some(Stage::One));
        assert_eq!(classify(&["@stage2".to_owned()], "title"), Some(Stage::Two));
        assert_eq!(classify(&[], "title with @stage1 text"), Some(Stage::One));
        assert_eq!(classify(&[], "untagged title"), None);
    }

    #[test]
    fn t_parse_report_empty_suites() {
        let json = r#"{"suites":[]}"#;
        let tiles = parse_report(json).unwrap();
        assert!(tiles.is_empty());
    }

    #[test]
    fn t_parse_report_no_video_skipped() {
        // Spec with no video attachment should be skipped
        let json = r#"{
            "suites": [{
                "specs": [{
                    "title": "T01 simple @stage1",
                    "tags": ["@stage1"],
                    "tests": [{
                        "results": [{
                            "status": "passed",
                            "attachments": []
                        }]
                    }]
                }],
                "suites": []
            }]
        }"#;
        let tiles = parse_report(json).unwrap();
        assert!(tiles.is_empty(), "no video → should be skipped");
    }

    #[test]
    fn t_parse_report_video_nonexistent_path_skipped() {
        // video attachment with a path that doesn't exist on disk should be skipped
        let json = r#"{
            "suites": [{
                "specs": [{
                    "title": "T01 simple @stage1",
                    "tags": ["@stage1"],
                    "tests": [{
                        "results": [{
                            "status": "passed",
                            "attachments": [{
                                "name": "video",
                                "path": "/nonexistent/path/video.webm",
                                "contentType": "video/webm"
                            }]
                        }]
                    }]
                }],
                "suites": []
            }]
        }"#;
        let tiles = parse_report(json).unwrap();
        // Path doesn't exist → skipped
        assert!(
            tiles.is_empty(),
            "nonexistent video path → should be skipped"
        );
    }

    #[test]
    fn t_build_tile_filtergraph_contains_drawbox() {
        // Build filtergraph for a single tile (passed) with no font
        let tile = TileInput {
            label: "T01 simple_box".to_owned(),
            passed: true,
            stage: Stage::One,
            video: PathBuf::from("/fake/video.webm"),
        };
        let fg = build_tile_filtergraph(&[tile], 4, 480, 360, 30, false, None);
        assert!(
            fg.contains("drawbox"),
            "must include drawbox for colour border"
        );
        assert!(fg.contains("0x4CAF50"), "pass tile must use green colour");
        assert!(!fg.contains("drawtext"), "no font → no drawtext");
        // Single tile should map to [out] directly
        assert!(fg.contains("[out]"));
    }

    #[test]
    fn t_build_tile_filtergraph_fail_tile_red() {
        let tile = TileInput {
            label: "T03 boolean_bad".to_owned(),
            passed: false,
            stage: Stage::One,
            video: PathBuf::from("/fake/video.webm"),
        };
        let fg = build_tile_filtergraph(&[tile], 4, 480, 360, 30, false, None);
        assert!(fg.contains("0xE53935"), "fail tile must use red colour");
    }

    #[test]
    fn t_build_tile_filtergraph_two_tiles_xstack() {
        let tiles = vec![
            TileInput {
                label: "A".to_owned(),
                passed: true,
                stage: Stage::One,
                video: PathBuf::from("/a.webm"),
            },
            TileInput {
                label: "B".to_owned(),
                passed: false,
                stage: Stage::Two,
                video: PathBuf::from("/b.webm"),
            },
        ];
        let fg = build_tile_filtergraph(&tiles, 4, 480, 360, 30, false, None);
        assert!(
            fg.contains("xstack=inputs=2"),
            "must use xstack for 2 tiles"
        );
        assert!(
            fg.contains("[v0]") && fg.contains("[v1]"),
            "must label per-tile outputs"
        );
    }

    #[test]
    fn t_build_tile_filtergraph_slow_motion() {
        let tile = TileInput {
            label: "P01".to_owned(),
            passed: true,
            stage: Stage::Two,
            video: PathBuf::from("/p.webm"),
        };
        let fg_slow = build_tile_filtergraph(&[tile.clone()], 4, 480, 360, 30, true, None);
        let fg_normal = build_tile_filtergraph(&[tile], 4, 480, 360, 30, false, None);
        assert!(
            fg_slow.contains("setpts=2.0*PTS"),
            "slow=true must include setpts"
        );
        assert!(
            !fg_normal.contains("setpts"),
            "slow=false must not include setpts"
        );
    }
}
