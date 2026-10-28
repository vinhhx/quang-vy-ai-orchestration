//! ENG-05 / D-32: scrubbing the key of an engine out of everything agent-brain writes or shows.
//!
//! A key lives in one place only: the environment of the child that needs it. A model that
//! echoes its environment, or a gateway that quotes the caller's key in an error, must still
//! leave no key in `stream.jsonl`, `events.raw_json`, `stderr.txt`, `runs.error`, the terminal
//! or the dashboard job output. Every function here takes the list of secrets of ONE run (the
//! `redact` of that run's `EnginePlan`), so two runs on different engines never see each
//! other's key.

use std::io::Write;
use std::path::Path;

/// What a secret is replaced with.
pub const REDACTED: &str = "[đã ẩn]";

/// A secret shorter than this is ignored: it would match unrelated text all over a stream.
pub const MIN_SECRET_BYTES: usize = 8;

/// The byte patterns to look for: every secret of at least `MIN_SECRET_BYTES`, plus its JSON
/// string-escaped spelling when that differs (a stream line is JSON), longest first so that a
/// secret containing another one is replaced whole.
fn patterns(secrets: &[String]) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    for s in secrets {
        if s.len() < MIN_SECRET_BYTES {
            continue;
        }
        out.push(s.as_bytes().to_vec());
        if let Ok(quoted) = serde_json::to_string(s) {
            let inner = &quoted[1..quoted.len() - 1];
            if inner != s {
                out.push(inner.as_bytes().to_vec());
            }
        }
    }
    out.sort_by_key(|p| std::cmp::Reverse(p.len()));
    out.dedup();
    out
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() || from > hay.len() - needle.len() {
        return None;
    }
    let first = needle[0];
    let last_start = hay.len() - needle.len();
    let mut i = from;
    while i <= last_start {
        if hay[i] == first && &hay[i..i + needle.len()] == needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn replace_all(input: &[u8], needle: &[u8]) -> Option<Vec<u8>> {
    let mut at = find(input, needle, 0)?;
    let mut out = Vec::with_capacity(input.len());
    let mut copied = 0;
    loop {
        out.extend_from_slice(&input[copied..at]);
        out.extend_from_slice(REDACTED.as_bytes());
        copied = at + needle.len();
        match find(input, needle, copied) {
            Some(next) => at = next,
            None => break,
        }
    }
    out.extend_from_slice(&input[copied..]);
    Some(out)
}

/// `line` with every occurrence of every secret (of at least 8 bytes) replaced by `[đã ẩn]`.
/// A line that holds no secret comes back byte for byte.
pub fn redact_line(line: &[u8], secrets: &[String]) -> Vec<u8> {
    let mut cur: Option<Vec<u8>> = None;
    for pat in patterns(secrets) {
        let src: &[u8] = cur.as_deref().unwrap_or(line);
        if let Some(next) = replace_all(src, &pat) {
            cur = Some(next);
        }
    }
    cur.unwrap_or_else(|| line.to_vec())
}

/// `text` with every secret replaced; used for texts built from run data (error, messages).
pub fn redact_str(text: &str, secrets: &[String]) -> String {
    String::from_utf8_lossy(&redact_line(text.as_bytes(), secrets)).into_owned()
}

/// Rewrites `path` with every secret replaced. A no-op for an empty list, a missing file or a
/// file that holds no secret. Otherwise the new content goes to a temp file in the same
/// directory (mode 0600) that is renamed over the original.
pub fn redact_file(path: &Path, secrets: &[String]) -> std::io::Result<()> {
    if patterns(secrets).is_empty() {
        return Ok(());
    }
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let redacted = redact_line(&bytes, secrets);
    if redacted == bytes {
        return Ok(());
    }
    let mut tmp_name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    tmp_name.push(".redact.tmp");
    let tmp = path.with_file_name(tmp_name);
    let written = (|| {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(&redacted)?;
        f.flush()?;
        std::fs::rename(&tmp, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "test-planted-gw-key-0123456789";

    fn key() -> Vec<String> {
        vec![KEY.to_string()]
    }

    #[test]
    fn redact_line_replaces_every_occurrence() {
        let line = format!("x {KEY} y");
        assert_eq!(
            redact_line(line.as_bytes(), &key()),
            "x [đã ẩn] y".as_bytes()
        );
        let twice = format!("{KEY}|{KEY}\n");
        assert_eq!(
            redact_line(twice.as_bytes(), &key()),
            "[đã ẩn]|[đã ẩn]\n".as_bytes()
        );
    }

    #[test]
    fn redact_line_ignores_short_secrets_and_clean_lines() {
        let short = vec!["abc1234".to_string()];
        assert_eq!(redact_line(b"abc1234 here", &short), b"abc1234 here");
        let line = b"{\"type\":\"assistant\"}\n";
        assert_eq!(redact_line(line, &key()), line);
        assert_eq!(redact_line(line, &[]), line);
        assert_eq!(redact_line(b"", &key()), b"");
    }

    #[test]
    fn redact_line_handles_two_secrets_and_overlap() {
        let a = "test-planted-gw-key-0123456789".to_string();
        let b = "test-planted-gw-key-0123456789-longer".to_string();
        let line = format!("{b} and {a}");
        assert_eq!(
            redact_line(line.as_bytes(), &[a, b]),
            "[đã ẩn] and [đã ẩn]".as_bytes()
        );
    }

    #[test]
    fn redact_line_catches_the_json_escaped_spelling() {
        let secret = "key\"with\\quote-0123456789".to_string();
        let escaped = serde_json::to_string(&secret).unwrap();
        let inner = &escaped[1..escaped.len() - 1];
        let line = format!("{{\"t\":\"{inner}\"}}");
        let out = redact_line(line.as_bytes(), &[secret]);
        assert_eq!(out, "{\"t\":\"[đã ẩn]\"}".as_bytes());
    }

    #[test]
    fn redact_file_rewrites_and_tolerates_missing() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("none.txt");
        assert!(redact_file(&missing, &key()).is_ok());
        assert!(!missing.exists(), "a missing file must not be created");

        let p = dir.path().join("stderr.txt");
        std::fs::write(&p, format!("a {KEY}\nb\n")).unwrap();
        redact_file(&p, &[]).unwrap();
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            format!("a {KEY}\nb\n")
        );
        redact_file(&p, &key()).unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "a [đã ẩn]\nb\n");
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["stderr.txt"], "no temp file may stay behind");
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
