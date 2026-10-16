use agent_brain::stream::summarize_file;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[test]
fn stream_fixture_visits_all_cases() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let contract_path = manifest_dir.join("../tests/contract/stream.json");
    let contract_str = std::fs::read_to_string(&contract_path).expect("read stream.json");
    let contract: serde_json::Value =
        serde_json::from_str(&contract_str).expect("parse stream.json");

    let cases = contract["cases"].as_array().expect("cases array");
    assert_eq!(cases.len(), 9, "Expected exactly 9 contract cases");
}

#[test]
fn stream_contract_cases() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let contract_path = manifest_dir.join("../tests/contract/stream.json");
    let repo_root = manifest_dir.join("..");
    let contract_str = std::fs::read_to_string(&contract_path).expect("read stream.json");
    let contract: serde_json::Value =
        serde_json::from_str(&contract_str).expect("parse stream.json");

    let cases = contract["cases"].as_array().expect("cases array");
    let mut failures = Vec::new();

    for case in cases {
        let cid = case["id"].as_str().unwrap();
        let file_rel = case["file"].as_str().unwrap();
        let file_path = repo_root.join(file_rel);

        let plan_files: Option<Vec<String>> = case
            .get("plan_files")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            });

        let root: Option<PathBuf> = case
            .get("root")
            .and_then(|v| v.as_str())
            .map(PathBuf::from);

        let summary = match summarize_file(&file_path, plan_files, root) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{cid}: summarize_file failed with io error: {e}"));
                continue;
            }
        };

        let expect = &case["expect"];
        let mut case_errors = Vec::new();

        // 1. session_id
        let exp_session_id = expect["session_id"].as_str().unwrap_or("");
        if summary.session_id != exp_session_id {
            case_errors.push(format!(
                "session_id: got {:?}, expect {:?}",
                summary.session_id, exp_session_id
            ));
        }

        // 2. model
        let exp_model = expect["model"].as_str().unwrap_or("");
        let got_model = summary.model.as_deref().unwrap_or("");
        if got_model != exp_model {
            case_errors.push(format!(
                "model: got {:?}, expect {:?}",
                got_model, exp_model
            ));
        }

        // 3. assistant_turns
        let exp_assistant_turns = expect["assistant_turns"].as_i64().unwrap_or(0);
        if summary.assistant_turns != exp_assistant_turns {
            case_errors.push(format!(
                "assistant_turns: got {}, expect {}",
                summary.assistant_turns, exp_assistant_turns
            ));
        }

        // 4. turns_before_first_edit
        let exp_turns_before_first_edit = expect["turns_before_first_edit"].as_i64();
        if summary.turns_before_first_edit != exp_turns_before_first_edit {
            case_errors.push(format!(
                "turns_before_first_edit: got {:?}, expect {:?}",
                summary.turns_before_first_edit, exp_turns_before_first_edit
            ));
        }

        // 5. first_edit_file
        let exp_first_edit_file = expect["first_edit_file"].as_str();
        if summary.first_edit_file.as_deref() != exp_first_edit_file {
            case_errors.push(format!(
                "first_edit_file: got {:?}, expect {:?}",
                summary.first_edit_file, exp_first_edit_file
            ));
        }

        // 6. reads
        let exp_reads: Vec<String> = expect["reads"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        if summary.reads != exp_reads {
            case_errors.push(format!(
                "reads: got {:?}, expect {:?}",
                summary.reads, exp_reads
            ));
        }

        // 7. reads_outside_plan
        let exp_reads_outside_plan: Option<Vec<String>> = expect
            .get("reads_outside_plan")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            });
        if summary.reads_outside_plan != exp_reads_outside_plan {
            case_errors.push(format!(
                "reads_outside_plan: got {:?}, expect {:?}",
                summary.reads_outside_plan, exp_reads_outside_plan
            ));
        }

        // 8. edits
        let exp_edits: Vec<String> = expect["edits"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        if summary.edits != exp_edits {
            case_errors.push(format!(
                "edits: got {:?}, expect {:?}",
                summary.edits, exp_edits
            ));
        }

        // 9. tool_counts
        let exp_tool_counts: BTreeMap<String, i64> = expect["tool_counts"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_i64().unwrap()))
            .collect();
        if summary.tool_counts != exp_tool_counts {
            case_errors.push(format!(
                "tool_counts: got {:?}, expect {:?}",
                summary.tool_counts, exp_tool_counts
            ));
        }

        // 10. permission_denials
        let exp_permission_denials = expect["permission_denials"].as_i64().unwrap_or(0);
        if summary.permission_denials != exp_permission_denials {
            case_errors.push(format!(
                "permission_denials: got {}, expect {}",
                summary.permission_denials, exp_permission_denials
            ));
        }

        // 11. format_ok
        let exp_format_ok = expect["format_ok"].as_bool().unwrap_or(false);
        if summary.format_ok != exp_format_ok {
            case_errors.push(format!(
                "format_ok: got {}, expect {}",
                summary.format_ok, exp_format_ok
            ));
        }

        // 12. bad_lines
        let exp_bad_lines = expect["bad_lines"].as_i64().unwrap_or(0);
        if summary.bad_lines != exp_bad_lines {
            case_errors.push(format!(
                "bad_lines: got {}, expect {}",
                summary.bad_lines, exp_bad_lines
            ));
        }

        // 13. total_lines
        let exp_total_lines = expect["total_lines"].as_i64().unwrap_or(0);
        if summary.total_lines != exp_total_lines {
            case_errors.push(format!(
                "total_lines: got {}, expect {}",
                summary.total_lines, exp_total_lines
            ));
        }

        // 14. result: subtype, is_error, num_turns, total_cost_usd
        if expect["result"].is_null() {
            if summary.subtype.is_some() || summary.format_ok {
                case_errors.push(format!(
                    "result: expected null, but got subtype {:?}, format_ok {}",
                    summary.subtype, summary.format_ok
                ));
            }
        } else {
            let res = &expect["result"];
            let exp_subtype = res["subtype"].as_str();
            if summary.subtype.as_deref() != exp_subtype {
                case_errors.push(format!(
                    "result.subtype: got {:?}, expect {:?}",
                    summary.subtype, exp_subtype
                ));
            }
            let exp_is_error = res["is_error"].as_bool().unwrap_or(false);
            if summary.is_error != exp_is_error {
                case_errors.push(format!(
                    "result.is_error: got {}, expect {}",
                    summary.is_error, exp_is_error
                ));
            }
            let exp_num_turns = res["num_turns"].as_i64();
            if summary.num_turns != exp_num_turns {
                case_errors.push(format!(
                    "result.num_turns: got {:?}, expect {:?}",
                    summary.num_turns, exp_num_turns
                ));
            }
            let exp_cost = res["total_cost_usd"].as_f64().unwrap_or(0.0);
            if (summary.cost_usd - exp_cost).abs() > 1e-6 {
                case_errors.push(format!(
                    "result.total_cost_usd: got {}, expect {}",
                    summary.cost_usd, exp_cost
                ));
            }
        }

        // 15. expect_rust: usage_source and usage
        let expect_rust = &case["expect_rust"];
        let exp_usage_source = expect_rust["usage_source"].as_str().unwrap();
        if summary.usage_source.as_str() != exp_usage_source {
            case_errors.push(format!(
                "usage_source: got {:?}, expect {:?}",
                summary.usage_source.as_str(),
                exp_usage_source
            ));
        }
        let exp_usage = &expect_rust["usage"];
        let exp_in = exp_usage["input_tokens"].as_i64().unwrap_or(0);
        let exp_out = exp_usage["output_tokens"].as_i64().unwrap_or(0);
        let exp_cr = exp_usage["cache_read_input_tokens"].as_i64().unwrap_or(0);
        let exp_cw = exp_usage["cache_creation_input_tokens"].as_i64().unwrap_or(0);

        if summary.usage.input_tokens != exp_in
            || summary.usage.output_tokens != exp_out
            || summary.usage.cache_read_input_tokens != exp_cr
            || summary.usage.cache_creation_input_tokens != exp_cw
        {
            case_errors.push(format!(
                "usage: got ({}, {}, {}, {}), expect ({}, {}, {}, {})",
                summary.usage.input_tokens,
                summary.usage.output_tokens,
                summary.usage.cache_read_input_tokens,
                summary.usage.cache_creation_input_tokens,
                exp_in,
                exp_out,
                exp_cr,
                exp_cw
            ));
        }

        if !case_errors.is_empty() {
            failures.push(format!("Case {cid} failures:\n  {}", case_errors.join("\n  ")));
        }
    }

    assert!(
        failures.is_empty(),
        "stream contract failures ({} cases failed):\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
