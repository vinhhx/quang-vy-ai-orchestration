use crate::common::Sandbox;
use std::path::PathBuf;

pub fn plan_text(
    title: &str,
    rows: &[(&str, &str, &str, &str)],
    checks: &str,
    blockers: &str,
) -> String {
    let mut lines = vec![
        format!("# Plan: {title}"),
        "## Mục tiêu".to_string(),
        format!("Thực hiện {title}"),
        "## Thay đổi".to_string(),
        "| # | File | Dòng / hàm | Sửa gì | Lý do |".to_string(),
        "|---|---|---|---|---|".to_string(),
    ];
    for (i, (f, where_, what, why)) in rows.iter().enumerate() {
        lines.push(format!("| {} | {f} | {where_} | {what} | {why} |", i + 1));
    }
    lines.extend(vec![
        "## Không được đụng tới".to_string(),
        "".to_string(),
        "## Cách kiểm tra".to_string(),
        checks.to_string(),
        "## Vướng mắc (Dev ghi vào đây nếu có)".to_string(),
        blockers.to_string(),
        "".to_string(),
    ]);
    lines.join("\n")
}

pub fn lead_scenario(rows: &[(&str, &str, &str, &str)]) -> serde_json::Value {
    let mut ops = vec![serde_json::json!({
        "op": "read",
        "path": ".plan.md"
    })];
    for (f, _, _, _) in rows {
        ops.push(serde_json::json!({
            "op": "read",
            "path": f
        }));
    }
    let p_text = plan_text("Kế hoạch", rows, "python3 -m unittest", "");
    ops.push(serde_json::json!({
        "op": "write",
        "path": ".plan.md",
        "content": p_text,
        "keep_meta": true
    }));
    serde_json::json!({ "ops": ops })
}

pub fn setup_project(sb: &Sandbox, profile: &str) -> PathBuf {
    let repo = sb.make_repo(
        "web-ban-hang",
        &[("src/app.py", "def app():\n    return 1\n")],
        None,
        None,
    );
    let mut entry = sb.project_entry(&repo, "web-ban-hang", profile);
    entry["name_vi"] = serde_json::json!("Web bán hàng");
    entry["aliases"] = serde_json::json!(["shop"]);
    sb.write_map(serde_json::json!([entry]), &[]);
    repo
}
