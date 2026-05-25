use std::path::Path;

fn main() {
    let web_dist = Path::new("../../web/dist");
    let index_html = web_dist.join("index.html");

    if !index_html.exists() {
        std::fs::create_dir_all(web_dist).expect("failed to create web/dist directory");
        std::fs::write(&index_html, STUB_HTML).expect("failed to write stub index.html");
    }

    println!("cargo:rerun-if-changed=../../web/dist");
    println!("cargo:rerun-if-changed=../../web/dist/index.html");

    let profile = std::env::var("PROFILE").unwrap_or_default();
    if profile == "release" {
        let content = std::fs::read_to_string(&index_html).unwrap_or_default();
        if !index_html.exists() || content.contains(STUB_SENTINEL) {
            panic!(
                "Release build detected stub frontend in web/dist.\n\
                 Run `cargo xtask web` (vite build) before release build."
            );
        }
    }
}

const STUB_SENTINEL: &str = "MYCAD frontend not built";
const STUB_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title>MyCad — frontend not built</title></head>
<body>
<h1>MYCAD frontend not built</h1>
<p>Run <code>cargo xtask web</code> to build the frontend, then restart the server.</p>
</body>
</html>
"#;
