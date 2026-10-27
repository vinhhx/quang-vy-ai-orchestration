//! Nhân sự (D-19): `~/.config/agent-brain/staff.json`, `{id: {display_name, notes?}}`.
//! Không có nhân sự có sẵn; file thiếu nghĩa là danh sách rỗng.

use crate::teams::FieldError;
use std::path::{Path, PathBuf};

pub const MAX_FILE_BYTES: u64 = 262_144;
pub const MAX_ID_BYTES: usize = 32;
pub const MAX_NAME_CHARS: usize = 64;
pub const MAX_NOTES_CHARS: usize = 500;

#[derive(Debug, Clone, PartialEq)]
pub struct Staff {
    pub id: String,
    pub display_name: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LoadedStaff {
    pub staff: Vec<Staff>,
    pub errors: Vec<String>,
}

impl LoadedStaff {
    pub fn get(&self, id: &str) -> Option<&Staff> {
        self.staff.iter().find(|s| s.id == id)
    }

    /// The display name, or the id itself when the person is not in the list.
    pub fn display_name_of(&self, id: &str) -> String {
        self.get(id)
            .map(|s| s.display_name.clone())
            .unwrap_or_else(|| id.to_string())
    }
}

/// Same slug rule as `workflows::validate_name`, capped at 32 bytes.
pub fn validate_id(id: &str) -> bool {
    id.len() <= MAX_ID_BYTES && crate::workflows::validate_name(id)
}

fn field_error(field: &str, message: impl Into<String>) -> FieldError {
    FieldError {
        field: field.to_string(),
        message: message.into(),
    }
}

pub fn validate_entry(id: &str, value: &serde_json::Value) -> Result<Staff, Vec<FieldError>> {
    let mut errors = Vec::new();

    if !validate_id(id) {
        errors.push(field_error(
            "id",
            "Mã chỉ gồm chữ thường không dấu, số và dấu gạch ngang.",
        ));
    }

    let obj = match value.as_object() {
        Some(o) => o,
        None => {
            errors.push(field_error("id", "Nhân sự phải là một object."));
            return Err(errors);
        }
    };

    for key in obj.keys() {
        if key != "display_name" && key != "notes" {
            errors.push(field_error(key, format!("Khóa lạ: {key}")));
        }
    }

    let mut display_name = String::new();
    match obj.get("display_name") {
        Some(serde_json::Value::String(s)) if !s.trim().is_empty() => {
            let trimmed = s.trim();
            if trimmed.chars().count() > MAX_NAME_CHARS {
                errors.push(field_error(
                    "display_name",
                    format!("Tên hiển thị dài quá {MAX_NAME_CHARS} ký tự."),
                ));
            } else {
                display_name = trimmed.to_string();
            }
        }
        _ => errors.push(field_error("display_name", "Nhập tên hiển thị.")),
    }

    let mut notes = None;
    match obj.get("notes") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.chars().count() > MAX_NOTES_CHARS {
                errors.push(field_error(
                    "notes",
                    format!("Ghi chú dài quá {MAX_NOTES_CHARS} ký tự."),
                ));
            } else if !trimmed.is_empty() {
                notes = Some(trimmed.to_string());
            }
        }
        Some(_) => errors.push(field_error("notes", "Ghi chú phải là văn bản.")),
    }

    if errors.is_empty() {
        Ok(Staff {
            id: id.to_string(),
            display_name,
            notes,
        })
    } else {
        Err(errors)
    }
}

pub fn staff_path() -> PathBuf {
    crate::config::config_dir().join("staff.json")
}

pub fn load_from(text: Option<&str>) -> LoadedStaff {
    let mut loaded = LoadedStaff::default();
    let Some(file_text) = text else {
        return loaded;
    };

    if file_text.len() as u64 > MAX_FILE_BYTES {
        loaded
            .errors
            .push("staff.json lớn quá 256 KiB — bỏ qua file.".to_string());
        return loaded;
    }

    let parsed: serde_json::Value = match serde_json::from_str(file_text) {
        Ok(v) => v,
        Err(e) => {
            loaded.errors.push(format!(
                "Dòng {}, cột {}: staff.json không phải JSON hợp lệ.",
                e.line(),
                e.column()
            ));
            return loaded;
        }
    };

    let Some(obj) = parsed.as_object() else {
        loaded
            .errors
            .push("Dòng 1: staff.json phải là một object, mỗi khóa là mã một nhân sự.".to_string());
        return loaded;
    };

    let lines = crate::workflows::key_lines(file_text);
    for (id, value) in obj {
        let line = lines.get(id).copied().unwrap_or(1);
        match validate_entry(id, value) {
            Ok(s) => loaded.staff.push(s),
            Err(errs) => {
                for e in errs {
                    loaded
                        .errors
                        .push(format!("Dòng {line}: nhân sự '{id}': {}", e.message));
                }
            }
        }
    }
    loaded
}

pub fn load_path(path: &Path) -> LoadedStaff {
    if !path.exists() {
        return load_from(None);
    }
    match std::fs::metadata(path) {
        Ok(m) if m.len() > MAX_FILE_BYTES => {
            let mut l = load_from(None);
            l.errors
                .push("staff.json lớn quá 256 KiB — bỏ qua file.".to_string());
            return l;
        }
        Ok(_) => {}
        Err(e) => {
            let mut l = load_from(None);
            l.errors.push(format!("Không đọc được staff.json: {e}"));
            return l;
        }
    }
    match std::fs::read_to_string(path) {
        Ok(text) => load_from(Some(&text)),
        Err(e) => {
            let mut l = load_from(None);
            l.errors.push(format!("Không đọc được staff.json: {e}"));
            l
        }
    }
}

pub fn load() -> LoadedStaff {
    load_path(&staff_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staff_rules() {
        let ok = load_from(Some(r#"{"ha": {"display_name": "Hà", "notes": "Zalo"}}"#));
        assert!(ok.errors.is_empty(), "{:?}", ok.errors);
        assert_eq!(ok.staff.len(), 1);
        assert_eq!(ok.staff[0].id, "ha");
        assert_eq!(ok.staff[0].display_name, "Hà");
        assert_eq!(ok.staff[0].notes.as_deref(), Some("Zalo"));
        assert_eq!(ok.display_name_of("ha"), "Hà");
        assert_eq!(ok.display_name_of("khong-co"), "khong-co");

        // Empty / absent notes are None; names are trimmed.
        let l = load_from(Some(
            r#"{"a": {"display_name": "  An  ", "notes": "  "}, "b": {"display_name": "Bình"}}"#,
        ));
        assert!(l.errors.is_empty(), "{:?}", l.errors);
        assert_eq!(l.staff[0].display_name, "An");
        assert_eq!(l.staff[0].notes, None);
        assert_eq!(l.staff[1].notes, None);
        assert_eq!(
            l.staff.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );

        // Bad id.
        let bad = load_from(Some(r#"{"Ha": {"display_name": "Hà"}}"#));
        assert!(bad.staff.is_empty());
        assert_eq!(
            bad.errors,
            ["Dòng 1: nhân sự 'Ha': Mã chỉ gồm chữ thường không dấu, số và dấu gạch ngang."]
        );

        // 33-byte id is too long, 32 is fine.
        let long33 = "a".repeat(33);
        let long32 = "a".repeat(32);
        assert!(!validate_id(&long33));
        assert!(validate_id(&long32));

        // Missing display_name.
        let miss = load_from(Some("{\n\"ha\": {\"notes\": \"x\"}}"));
        assert_eq!(miss.errors, ["Dòng 2: nhân sự 'ha': Nhập tên hiển thị."]);

        // 501-char notes, valid ones still load.
        let notes = "n".repeat(501);
        let text = format!(
            r#"{{"ha": {{"display_name": "Hà", "notes": "{notes}"}}, "ok": {{"display_name": "Ổn"}}}}"#
        );
        let l = load_from(Some(&text));
        assert_eq!(l.staff.len(), 1);
        assert_eq!(l.staff[0].id, "ok");
        assert_eq!(l.errors.len(), 1);
        assert!(
            l.errors[0].ends_with("nhân sự 'ha': Ghi chú dài quá 500 ký tự."),
            "{:?}",
            l.errors
        );
        assert!(l.errors[0].starts_with("Dòng 1: nhân sự 'ha': "));

        // File level.
        assert!(load_from(None).staff.is_empty());
        let big = " ".repeat(262_145);
        assert_eq!(
            load_from(Some(&big)).errors,
            ["staff.json lớn quá 256 KiB — bỏ qua file."]
        );
        let malformed = load_from(Some(r#"{"a": "#));
        assert!(malformed.errors[0].starts_with("Dòng 1, cột"));
        assert!(malformed.errors[0].ends_with("staff.json không phải JSON hợp lệ."));
        assert_eq!(
            load_from(Some("[]")).errors,
            ["Dòng 1: staff.json phải là một object, mỗi khóa là mã một nhân sự."]
        );
    }

    #[test]
    fn staff_load_path_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let l = load_path(&dir.path().join("staff.json"));
        assert!(l.staff.is_empty());
        assert!(l.errors.is_empty());
    }
}
