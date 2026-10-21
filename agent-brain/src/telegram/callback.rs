//! Callback-data grammar of the bot's inline buttons (Phase 05.3, D-11/D-15).
//!
//! `{op}:{task}:{rev}` or `{op}:{task}:{run}:{rev}`; `op` is one character,
//! ids are positive decimal integers and `rev` is the task's 16-hex `rev`.
//! Telegram limits callback_data to 64 bytes; the longest value is 58 bytes.

pub const MAX_LEN: usize = 64;

const REV_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Approve,
    ReturnLead,
    MoreRound,
    End,
    KillAsk,
    KillConfirm,
    KillCancel,
    NoNote,
    CancelNote,
}

pub const ALL_OPS: [Op; 9] = [
    Op::Approve,
    Op::ReturnLead,
    Op::MoreRound,
    Op::End,
    Op::KillAsk,
    Op::KillConfirm,
    Op::KillCancel,
    Op::NoNote,
    Op::CancelNote,
];

impl Op {
    pub fn code(self) -> char {
        match self {
            Op::Approve => 'a',
            Op::ReturnLead => 'r',
            Op::MoreRound => 'm',
            Op::End => 'e',
            Op::KillAsk => 'k',
            Op::KillConfirm => 'K',
            Op::KillCancel => 'x',
            Op::NoNote => 'n',
            Op::CancelNote => 'c',
        }
    }

    pub fn from_code(c: char) -> Option<Op> {
        ALL_OPS.iter().copied().find(|op| op.code() == c)
    }

    /// Kill operations carry the run id.
    pub fn needs_run(self) -> bool {
        matches!(self, Op::KillAsk | Op::KillConfirm | Op::KillCancel)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callback {
    pub op: Op,
    pub task_id: i64,
    pub run_id: Option<i64>,
    pub rev: String,
}

pub fn encode(cb: &Callback) -> String {
    match (cb.op.needs_run(), cb.run_id) {
        (true, Some(run)) => format!("{}:{}:{}:{}", cb.op.code(), cb.task_id, run, cb.rev),
        _ => format!("{}:{}:{}", cb.op.code(), cb.task_id, cb.rev),
    }
}

fn parse_id(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let v: i64 = s.parse().ok()?;
    (v > 0).then_some(v)
}

fn valid_rev(s: &str) -> bool {
    s.len() == REV_LEN && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Strict parse: anything not produced by `encode` returns `None`.
pub fn decode(data: &str) -> Option<Callback> {
    if data.is_empty() || data.len() > MAX_LEN {
        return None;
    }
    let parts: Vec<&str> = data.split(':').collect();
    let mut chars = parts.first()?.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let op = Op::from_code(c)?;
    let want = if op.needs_run() { 4 } else { 3 };
    if parts.len() != want {
        return None;
    }
    let task_id = parse_id(parts[1])?;
    let run_id = if op.needs_run() {
        Some(parse_id(parts[2])?)
    } else {
        None
    };
    let rev = parts[want - 1];
    if !valid_rev(rev) {
        return None;
    }
    Some(Callback {
        op,
        task_id,
        run_id,
        rev: rev.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const REV: &str = "0123456789abcdef";

    #[test]
    fn round_trip_every_op() {
        for op in ALL_OPS {
            let cb = Callback {
                op,
                task_id: 42,
                run_id: op.needs_run().then_some(7),
                rev: REV.to_string(),
            };
            let data = encode(&cb);
            assert!(data.len() <= MAX_LEN, "{data}");
            assert_eq!(decode(&data), Some(cb), "{data}");
            assert_eq!(Op::from_code(op.code()), Some(op));
        }
        assert_eq!(
            encode(&Callback {
                op: Op::KillAsk,
                task_id: 3,
                run_id: Some(9),
                rev: REV.to_string()
            }),
            format!("k:3:9:{REV}")
        );
        assert_eq!(
            encode(&Callback {
                op: Op::Approve,
                task_id: 1,
                run_id: None,
                rev: REV.to_string()
            }),
            format!("a:1:{REV}")
        );
    }

    #[test]
    fn longest_value_fits() {
        let cb = Callback {
            op: Op::KillConfirm,
            task_id: i64::MAX,
            run_id: Some(i64::MAX),
            rev: REV.to_string(),
        };
        let data = encode(&cb);
        assert_eq!(data.len(), 58);
        assert_eq!(decode(&data), Some(cb));
    }

    #[test]
    fn rejects_malformed() {
        let long = format!("a:1:{REV}{}", "0".repeat(65 - 4 - REV.len()));
        assert_eq!(long.len(), 65);
        for bad in [
            "a:1:XYZ",
            "a:0:0123456789abcdef",
            "a:-1:0123456789abcdef",
            "k:1:0123456789abcdef",
            "a:1:2:0123456789abcdef",
            "z:1:0123456789abcdef",
            "",
            long.as_str(),
        ] {
            assert_eq!(decode(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn rejects_other_shapes() {
        for bad in [
            "a:1:0123456789ABCDEF",
            "a:1:0123456789abcde",
            "aa:1:0123456789abcdef",
            "a:+1:0123456789abcdef",
            "a: 1:0123456789abcdef",
            "a:99999999999999999999:0123456789abcdef",
            "K:1:0:0123456789abcdef",
            "a:1:0123456789abcdef:",
        ] {
            assert_eq!(decode(bad), None, "{bad:?}");
        }
    }
}
