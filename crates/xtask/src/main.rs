use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let task = args.first().map(|s| s.as_str()).unwrap_or("help");

    match task {
        "ci" => ci(),
        "help" | "--help" | "-h" => {
            println!("Usage: cargo xtask <TASK>");
            println!();
            println!("Tasks:");
            println!("  ci    Run fmt check, clippy, tests, and build");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("Unknown task: {other}");
            eprintln!("Run `cargo xtask help` for available tasks.");
            ExitCode::FAILURE
        }
    }
}

fn ci() -> ExitCode {
    let steps: &[(&str, &[&str])] = &[
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

    for (label, cmd) in steps {
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

    println!("\n=== All CI checks passed ===");
    ExitCode::SUCCESS
}
