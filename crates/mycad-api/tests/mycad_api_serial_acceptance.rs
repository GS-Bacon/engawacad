//! Acceptance tests for #153: mycad-api integration test の serial_test 導入。

use serial_test::file_serial;
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

/// Wait up to `timeout` for port 7878 to become bindable.
/// Defensive against the previous test's mycad-api leaving the socket in TIME_WAIT
/// (Codex B-6 F02 指摘対応: startup_log_acceptance.rs と同等の wait helper)。
fn wait_port_free(timeout: Duration) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if TcpListener::bind("127.0.0.1:7878").is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[test]
#[file_serial(mycad_api_port_7878)]
fn t_boundary_parallel_safety() {
    // file_serial が port 7878 を握る全テスト (別 binary 含む) を file-lock で直列化することを確認。
    // TIME_WAIT による偽 EADDRINUSE を避けるため、startup_log_acceptance 系と同じ wait helper を先に呼ぶ。
    wait_port_free(Duration::from_secs(10));

    let listener = TcpListener::bind("127.0.0.1:7878").expect(
        "port 7878 should be bindable when file_serial serializes access across test binaries",
    );
    drop(listener);
}
