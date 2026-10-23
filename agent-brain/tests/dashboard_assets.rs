mod common;

use common::Sandbox;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir).expect("read dir") {
            let entry = entry.expect("dir entry");
            let p = entry.path();
            if p.is_dir() {
                files.extend(collect_files(&p));
            } else if p.is_file() {
                files.push(p);
            }
        }
    }
    files
}

#[test]
fn dash09_no_external_loads() {
    let assets_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dashboard");
    let files = collect_files(&assets_dir);

    let forbidden = [
        "src=\"http",
        "src='http",
        "href=\"http",
        "href='http",
        "url(http",
        "url(\"http",
        "url('http",
        "@import url(",
        "//cdn",
        "fetch(\"http",
        "fetch('http",
        "eventsource(\"http",
        "eventsource('http",
    ];

    for f in files {
        if f.components().any(|c| c.as_os_str() == "vendor")
            || f.file_name() == Some(OsStr::new("VENDOR.md"))
        {
            continue;
        }

        let bytes = fs::read(&f).unwrap_or_else(|e| panic!("failed to read {:?}: {e}", f));
        let content = String::from_utf8_lossy(&bytes).to_ascii_lowercase();

        for pat in &forbidden {
            assert!(
                !content.contains(pat),
                "File {:?} contains forbidden external load pattern: {:?}",
                f,
                pat
            );
        }
    }
}

#[test]
fn dash09_css_is_prebuilt_tailwind() {
    let assets_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dashboard");
    let css_path = assets_dir.join("app.css");
    let content = fs::read_to_string(&css_path).expect("read app.css");
    assert!(
        content.starts_with("/*! tailwindcss v4.3.3"),
        "app.css does not start with Tailwind banner"
    );
    assert!(
        content.len() > 1000,
        "app.css is too small ({} bytes)",
        content.len()
    );
}

#[test]
fn dash09_vendor_alpine_is_pinned_build() {
    let assets_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dashboard");
    let alpine_path = assets_dir.join("vendor/alpine.csp.min.js");
    let bytes = fs::read(&alpine_path).expect("read alpine.csp.min.js");
    assert_eq!(bytes.len(), 72198, "vendor/alpine.csp.min.js size mismatch");

    let vendor_md = fs::read_to_string(assets_dir.join("VENDOR.md")).expect("read VENDOR.md");
    assert!(
        vendor_md.contains("3.17.4"),
        "VENDOR.md does not name version 3.17.4"
    );
}

#[test]
fn assets_table_matches_tree() {
    let assets_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dashboard");
    let files = collect_files(&assets_dir);

    let mut expected_map = std::collections::HashMap::new();
    for f in &files {
        let rel = f.strip_prefix(&assets_dir).expect("strip prefix");
        if rel.to_string_lossy() == "VENDOR.md" {
            continue;
        }
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let asset_path = if rel_str == "index.html" {
            "/".to_string()
        } else {
            format!("/{rel_str}")
        };
        let bytes = fs::read(f).expect("read asset file");
        expected_map.insert(asset_path, bytes);
    }

    assert_eq!(
        agent_brain::dashboard::assets::ASSETS.len(),
        expected_map.len(),
        "ASSETS count mismatch: table has {}, tree has {}",
        agent_brain::dashboard::assets::ASSETS.len(),
        expected_map.len()
    );

    for asset in agent_brain::dashboard::assets::ASSETS {
        let file_bytes = expected_map.get(asset.path).unwrap_or_else(|| {
            panic!(
                "Asset path {} in ASSETS not found in filesystem",
                asset.path
            )
        });
        assert_eq!(
            asset.body,
            file_bytes.as_slice(),
            "Body mismatch for {}",
            asset.path
        );
        assert!(
            asset.mime.starts_with("text/html")
                || asset.mime.starts_with("text/css")
                || asset.mime.starts_with("text/javascript"),
            "Unexpected mime type for {}: {}",
            asset.path,
            asset.mime
        );
    }
}

#[test]
fn asset_references_resolve() {
    let assets_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dashboard");

    for name in &["overview", "assign", "task", "stats", "projects", "board"] {
        let js_path = format!("/js/{name}.js");
        let html_path = format!("/screens/{name}.html");
        assert!(
            agent_brain::dashboard::assets::lookup(&js_path).is_some(),
            "Missing ASSETS entry for {js_path}"
        );
        assert!(
            agent_brain::dashboard::assets::lookup(&html_path).is_some(),
            "Missing ASSETS entry for {html_path}"
        );
    }

    let html_files = collect_files(&assets_dir);
    let re = regex::Regex::new(r#"(?:src|href)="(/[^"]*)""#).expect("regex");
    for f in html_files {
        if f.extension().and_then(|s| s.to_str()) != Some("html") {
            continue;
        }
        let content = fs::read_to_string(&f).expect("read html");
        for cap in re.captures_iter(&content) {
            let ref_path = &cap[1];
            assert!(
                agent_brain::dashboard::assets::lookup(ref_path).is_some(),
                "Reference to {ref_path} in {:?} does not resolve to an ASSETS entry",
                f
            );
        }
    }
}

#[test]
fn assets_served_after_handshake() {
    let sb = Sandbox::new();
    let srv = sb.serve(&[]);

    for asset in agent_brain::dashboard::assets::ASSETS {
        let resp = srv.get(asset.path);
        assert_eq!(
            resp.status, 200,
            "Serving asset {} returned status {}",
            asset.path, resp.status
        );
        let ct = resp
            .header("content-type")
            .unwrap_or_else(|| panic!("Asset {} missing Content-Type", asset.path));
        assert_eq!(
            ct, asset.mime,
            "Asset {} mime mismatch: expected {}, got {}",
            asset.path, asset.mime, ct
        );
        assert_eq!(
            resp.body, asset.body,
            "Asset {} body served mismatch",
            asset.path
        );
    }
}
