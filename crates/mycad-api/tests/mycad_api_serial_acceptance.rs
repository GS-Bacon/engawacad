//! Acceptance tests for #153: mycad-api integration test の serial_test 導入。

use serial_test::file_serial;
use std::net::TcpListener;

#[test]
#[file_serial(mycad_api_port_7878)]
fn t_boundary_parallel_safety() {
    // serial_test が port 7878 を握る全テストを直列化することを確認:
    // この test が動いている間、他の serial(mycad_api_port_7878) テストは block される。
    // bind が成功することで「他テストが解放した直後の状態」を確認する。
    let listener = TcpListener::bind("127.0.0.1:7878")
        .expect("port 7878 should be bindable when serial_test serializes access");
    drop(listener);
}
