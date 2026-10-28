//! E-09 fixtures (ENG-11, D-34): parse whatever the user's real probes produced.
//!
//! `spikes/e09/run.sh` writes, per endpoint name, `tests/fixtures/stream/anthropic_e09_{name}_ok.jsonl`,
//! `anthropic_e09_{name}_m{k}_ok.jsonl` (k >= 2) and `anthropic_e09_{name}_auth_error.jsonl`. Those files only
//! exist after the user ran the probe, so with none present this test prints a skip line and passes.

use agent_brain::quota::detect_claude;
use agent_brain::stream::summarize_file;
use std::path::{Path, PathBuf};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/stream")
}

/// File names in the stream fixture directory that start with `anthropic_e09_` and satisfy `pred`.
fn e09_fixtures(pred: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(fixtures_dir()) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("anthropic_e09_") && n.ends_with(".jsonl") && pred(n))
                .unwrap_or(false)
        })
        .collect();
    out.sort();
    out
}

#[test]
fn e09_fixtures_parse() {
    let ok = e09_fixtures(|n| n.ends_with("_ok.jsonl"));
    let errors = e09_fixtures(|n| n.ends_with("_auth_error.jsonl"));
    if ok.is_empty() && errors.is_empty() {
        println!("bỏ qua: chưa có fixture E-09");
        return;
    }

    for path in &ok {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let s = summarize_file(path, None, None).unwrap_or_else(|e| panic!("{name}: đọc lỗi: {e}"));
        assert!(s.result_seen(), "{name}: không có result");
        assert!(s.usage.input_tokens > 0, "{name}: input_tokens phải > 0");
        assert!(s.usage.output_tokens > 0, "{name}: output_tokens phải > 0");
        println!("{name}: ok (in={} out={})", s.usage.input_tokens, s.usage.output_tokens);
    }

    for path in &errors {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let s = summarize_file(path, None, None).unwrap_or_else(|e| panic!("{name}: đọc lỗi: {e}"));
        assert!(s.result_seen(), "{name}: không có result");
        assert!(s.is_error, "{name}: result phải là lỗi");
        assert!(
            detect_claude(&s, None, "").is_none(),
            "{name}: lỗi xác thực không được tính là hết quota"
        );
        println!("{name}: auth error ok");
    }
}
