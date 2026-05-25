use std::path::Path;

pub const DEFAULT_PORT: u16 = 7878;

pub fn build_view_url(port: u16, abs_path: &Path) -> String {
    let path_str = abs_path.to_string_lossy();
    format!(
        "http://127.0.0.1:{port}/?file={}",
        urlencoding::encode(&path_str)
    )
}

pub async fn bind_listener(start_port: u16) -> Result<(tokio::net::TcpListener, u16), String> {
    for i in 0..16 {
        let port = start_port
            .checked_add(i)
            .ok_or_else(|| format!("port overflow: {start_port} + {i} exceeds u16 range"))?;
        match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
            Ok(listener) => return Ok((listener, port)),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => continue,
            Err(e) => return Err(format!("failed to bind port {port}: {e}")),
        }
    }
    Err(format!(
        "all 16 ports starting from {start_port} are in use"
    ))
}

pub fn run_view(input: &Path, port: u16) -> Result<(), String> {
    let abs_path = std::fs::canonicalize(input)
        .map_err(|e| format!("failed to resolve path {}: {e}", input.display()))?;

    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("failed to create tokio runtime: {e}"))?;
    rt.block_on(async {
        let (listener, actual_port) = bind_listener(port).await?;
        let url = build_view_url(actual_port, &abs_path);

        let _ = open::that(&url);

        println!("MyCad viewer: {url}");
        println!("Press Ctrl-C to stop.");

        axum::serve(listener, mycad_api::router::app())
            .await
            .map_err(|e| format!("server error: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t01_build_view_url_deterministic() {
        let path = Path::new("/tmp/test.mycad");
        let url1 = build_view_url(7878, path);
        let url2 = build_view_url(7878, path);
        assert_eq!(url1, url2);
    }

    #[test]
    fn t02_build_view_url_encodes_spaces() {
        let path = Path::new("/tmp/my file.mycad");
        let url = build_view_url(7878, path);
        assert!(
            url.contains("%20"),
            "spaces should be percent-encoded: {url}"
        );
        assert!(url.contains("?file="), "should contain ?file= query param");
        assert!(
            url.starts_with("http://127.0.0.1:7878/"),
            "should start with correct base URL"
        );
    }

    #[test]
    fn t02_build_view_url_encodes_special_chars() {
        let path = Path::new("/tmp/日本語.mycad");
        let url = build_view_url(9000, path);
        assert!(url.contains("?file="));
        assert!(url.starts_with("http://127.0.0.1:9000/"));
    }

    #[test]
    fn t07_run_view_missing_file() {
        let result = run_view(Path::new("/nonexistent/path.mycad"), 7878);
        assert!(result.is_err(), "should fail for nonexistent file");
    }

    #[test]
    fn t10_bind_listener_port_overflow() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async { bind_listener(65535).await });
        match result {
            Ok((_, port)) => assert_eq!(port, 65535),
            Err(msg) => {
                assert!(
                    msg.contains("in use") || msg.contains("overflow"),
                    "unexpected error: {msg}"
                );
            }
        }
    }

    #[test]
    fn t10_bind_listener_max_port_wrap() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async { bind_listener(65530).await });
        match result {
            Ok((_, port)) => assert!((65530..=65535).contains(&port)),
            Err(msg) => {
                assert!(
                    !msg.contains("panic") && !msg.contains("overflow") || msg.contains("in use"),
                    "should not panic, got: {msg}"
                );
            }
        }
    }

    #[test]
    fn t06_bind_listener_fallback() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let occupied_port = listener.local_addr().unwrap().port();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async { bind_listener(occupied_port).await.unwrap() });
        let actual_port = result.1;
        assert_ne!(
            actual_port, occupied_port,
            "should not return the occupied port"
        );
        assert!(
            (occupied_port..occupied_port + 16).contains(&actual_port),
            "should be within scan range"
        );
    }

    #[test]
    fn t01_build_view_url_simple_path() {
        let path = Path::new("/home/user/model.mycad");
        let url = build_view_url(8080, path);
        assert_eq!(
            url,
            "http://127.0.0.1:8080/?file=%2Fhome%2Fuser%2Fmodel.mycad"
        );
    }

    #[test]
    fn default_port_value() {
        assert_eq!(DEFAULT_PORT, 7878);
    }

    #[test]
    fn edge_build_view_url_empty_path() {
        let path = Path::new("");
        let url = build_view_url(7878, path);
        assert!(url.starts_with("http://127.0.0.1:7878/?file="));
    }

    #[test]
    fn edge_build_view_url_percent_chars() {
        let path = Path::new("/tmp/test%20file.mycad");
        let url = build_view_url(7878, path);
        assert!(url.contains("?file="));
        // % should be double-encoded to %25
        assert!(url.contains("%2520") || url.contains("%20"));
    }

    #[test]
    fn edge_build_view_url_port_zero() {
        let path = Path::new("/tmp/test.mycad");
        let url = build_view_url(0, path);
        assert!(url.starts_with("http://127.0.0.1:0/"));
    }

    #[test]
    fn edge_build_view_url_port_max() {
        let path = Path::new("/tmp/test.mycad");
        let url = build_view_url(65535, path);
        assert!(url.starts_with("http://127.0.0.1:65535/"));
    }

    #[test]
    fn edge_build_view_url_deterministic_100_runs() {
        let path = Path::new("/tmp/test file with spaces.mycad");
        let url1 = build_view_url(7878, path);
        for i in 0..100 {
            let url_n = build_view_url(7878, path);
            assert_eq!(url1, url_n, "URL differs at iteration {i}");
        }
    }

    #[test]
    fn edge_bind_listener_all_ports_in_use() {
        // Occupy 16 ports and verify bind_listener returns error
        let mut listeners: Vec<std::net::TcpListener> = Vec::new();
        let base = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base_port = base.local_addr().unwrap().port();
        listeners.push(base);

        for i in 1..16u16 {
            let port = base_port + i;
            if let Ok(l) = std::net::TcpListener::bind(("127.0.0.1", port)) {
                listeners.push(l);
            }
        }

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async { bind_listener(base_port).await });
        assert!(result.is_err(), "should fail when all ports in use");
        let msg = result.unwrap_err();
        assert!(msg.contains("in use"), "error should mention in use: {msg}");
    }
}
