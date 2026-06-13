use serial_test::file_serial;
use std::io::Read;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Wait up to `timeout` for port 7878 to become bindable. Defensive in case the
/// previous test's mycad-api still has the socket in TIME_WAIT.
fn wait_port_free(timeout: Duration) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if TcpListener::bind("127.0.0.1:7878").is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn example_simple_box() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("examples/simple_box.mycad")
}

/// Spawn `kill -KILL <pid>` after `delay` so subsequent stderr reads see EOF
/// and the test never hangs even if mycad-api emits nothing to stderr
/// (Codex #145 F01 — guards against the bare blocking-read regression).
fn kill_after(pid: u32, delay: Duration) {
    thread::spawn(move || {
        thread::sleep(delay);
        let _ = Command::new("kill")
            .arg("-KILL")
            .arg(pid.to_string())
            .status();
    });
}

/// Release-profile binary path. `cargo test` builds with the test profile so
/// `CARGO_BIN_EXE_mycad-api` resolves to the debug binary; the path Playwright
/// actually runs is the release one (Codex #145 F02 — exercise that path here).
fn release_bin() -> Option<PathBuf> {
    let target_dir: PathBuf = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|p| p.parent())
                .map(|root| root.join("target"))
                .expect("workspace target")
        });
    let bin = target_dir.join("release/mycad-api");
    bin.exists().then_some(bin)
}

#[test]
#[file_serial(engawa_api_port_7878)]
fn t01_startup_log() {
    wait_port_free(Duration::from_secs(10));

    let example = example_simple_box();
    let mut child = Command::new(env!("CARGO_BIN_EXE_mycad-api"))
        .arg(&example)
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("failed to spawn mycad-api");

    let pid = child.id();
    let mut stderr = child.stderr.take().expect("stderr pipe");
    kill_after(pid, Duration::from_secs(3));

    let mut buf = String::new();
    let _ = stderr.read_to_string(&mut buf);
    let _ = child.wait();

    assert!(
        buf.contains("listening on 127.0.0.1:7878"),
        "expected 'listening on 127.0.0.1:7878' on stderr; got {} bytes: {:?}",
        buf.len(),
        buf
    );
}

#[test]
#[file_serial(engawa_api_port_7878)]
fn t02_release_bin_runs() {
    wait_port_free(Duration::from_secs(10));

    // Build the release binary from current source so we never validate a stale
    // artefact (Codex #145 F02). cargo's incremental build keeps this cheap when
    // the binary is already up to date.
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf();
    let build_status = Command::new("cargo")
        .args(["build", "-p", "mycad-api", "--release", "--quiet"])
        .current_dir(&workspace)
        .status()
        .expect("failed to spawn cargo build --release");
    assert!(
        build_status.success(),
        "cargo build -p mycad-api --release failed"
    );

    let bin = release_bin().expect("target/release/mycad-api should exist after build");
    let example = example_simple_box();
    let mut child = Command::new(&bin)
        .arg(&example)
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("failed to spawn release mycad-api");

    let pid = child.id();
    let stderr = child.stderr.take().expect("stderr pipe");
    // Give us 10s before SIGKILL so the connection check below has a window even
    // when cold (Codex #145 F01 round 4).
    kill_after(pid, Duration::from_secs(10));

    // Drain stderr on a side thread; it completes when kill_after closes the pipe.
    let stderr_handle = thread::spawn(move || {
        let mut s = stderr;
        let mut buf = String::new();
        let _ = s.read_to_string(&mut buf);
        buf
    });

    // Confirm the server actually accepts TCP connections — not just that the
    // startup log printed (Codex #145 F01 round 4 — guard against a regression
    // where mycad-api logs then dies before serving).
    let addr: SocketAddr = "127.0.0.1:7878".parse().expect("addr");
    let connect_deadline = Instant::now() + Duration::from_secs(8);
    let mut connected = false;
    while Instant::now() < connect_deadline {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok() {
            connected = true;
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    let buf = stderr_handle.join().unwrap_or_default();
    let _ = child.wait();

    assert!(
        connected,
        "release mycad-api did not accept TCP connections on 127.0.0.1:7878 within 8s; \
         stderr: {:?}",
        buf
    );
    assert!(
        buf.contains("listening on 127.0.0.1:7878"),
        "expected 'listening on 127.0.0.1:7878' on stderr from release binary; got: {:?}",
        buf
    );

    let release_deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < release_deadline {
        if TcpListener::bind("127.0.0.1:7878").is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("port 7878 did not release within 5s after kill");
}

#[test]
fn t03_boundary_invalid_path() {
    let status = Command::new(env!("CARGO_BIN_EXE_mycad-api"))
        .arg("/nonexistent/path.mycad")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("failed to spawn");
    assert!(!status.success(), "expected non-zero exit for invalid path");
}

#[test]
fn t04_degen_no_args() {
    let status = Command::new(env!("CARGO_BIN_EXE_mycad-api"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("failed to spawn");
    assert!(!status.success(), "expected non-zero exit for no args");
}
