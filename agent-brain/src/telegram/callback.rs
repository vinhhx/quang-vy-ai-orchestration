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
    /// Two-step approval of a Telegram-created task's plan: ask, confirm, cancel.
    ApproveAsk,
    ApproveConfirm,
    ApproveCancel,
    ReturnLead,
    MoreRound,
    End,
    KillAsk,
    KillConfirm,
    KillCancel,
    NoNote,
    CancelNote,
    /// `Không chạy lại` on a quota re-run confirmation (Phase 6, D-28).
    TeamCancel,
    /// `Thử lại sau` on a quota message (Phase 6, D-17).
    RetryLater,
}

pub const ALL_OPS: [Op; 14] = [
    Op::Approve,
    Op::ApproveAsk,
    Op::ApproveConfirm,
    Op::ApproveCancel,
    Op::ReturnLead,
    Op::MoreRound,
    Op::End,
    Op::KillAsk,
    Op::KillConfirm,
    Op::KillCancel,
    Op::NoNote,
    Op::CancelNote,
    Op::TeamCancel,
    Op::RetryLater,
];

impl Op {
    pub fn code(self) -> char {
        match self {
            Op::Approve => 'a',
            Op::ApproveAsk => 'p',
            Op::ApproveConfirm => 'y',
            Op::ApproveCancel => 'q',
            Op::ReturnLead => 'r',
            Op::MoreRound => 'm',
            Op::End => 'e',
            Op::KillAsk => 'k',
            Op::KillConfirm => 'K',
            Op::KillCancel => 'x',
            Op::NoNote => 'n',
            Op::CancelNote => 'c',
            Op::TeamCancel => 'v',
            Op::RetryLater => 'l',
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

/// Most team buttons one quota message carries (Phase 6, D-17); the rest are chosen on the dashboard.
pub const MAX_TEAM_BUTTONS: usize = 8;

/// The two team buttons that carry an index: pick a team (`t`), confirm the re-run (`u`).
/// `v` (cancel) and `l` (later) are plain `Op`s of the 05.3 grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamOp {
    Pick,
    Confirm,
}

impl TeamOp {
    pub fn code(self) -> char {
        match self {
            TeamOp::Pick => 't',
            TeamOp::Confirm => 'u',
        }
    }

    fn from_code(c: &str) -> Option<TeamOp> {
        match c {
            "t" => Some(TeamOp::Pick),
            "u" => Some(TeamOp::Confirm),
            _ => None,
        }
    }
}

/// `{t|u}:{task}:{idx}:{ver4}:{rev}`: `idx` is a position in the team list the server holds
/// (never a name), `ver` the 4-hex version of that list, `rev` the task's 16-hex `rev`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamCb {
    pub op: TeamOp,
    pub task_id: i64,
    pub idx: usize,
    pub ver: String,
    pub rev: String,
}

const VER_LEN: usize = 4;

pub fn encode_team(cb: &TeamCb) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        cb.op.code(),
        cb.task_id,
        cb.idx,
        cb.ver,
        cb.rev
    )
}

/// First 4 hex characters of a hash of the team names, in list order. The same list gives the
/// same version for the lifetime of the process; any added, removed or reordered team changes it.
pub fn team_list_ver(names: &[String]) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    names.join("\n").hash(&mut h);
    format!("{:016x}", h.finish())[..VER_LEN].to_string()
}

fn valid_ver(s: &str) -> bool {
    s.len() == VER_LEN && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// One decimal digit below `MAX_TEAM_BUTTONS`.
fn parse_team_idx(s: &str) -> Option<usize> {
    if s.len() != 1 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let v: usize = s.parse().ok()?;
    (v < MAX_TEAM_BUTTONS).then_some(v)
}

/// Strict parse: anything not produced by `encode_team` returns `None`.
pub fn decode_team(data: &str) -> Option<TeamCb> {
    if data.is_empty() || data.len() > MAX_LEN {
        return None;
    }
    let parts: Vec<&str> = data.split(':').collect();
    if parts.len() != 5 {
        return None;
    }
    let op = TeamOp::from_code(parts[0])?;
    let task_id = parse_id(parts[1])?;
    let idx = parse_team_idx(parts[2])?;
    if !valid_ver(parts[3]) || !valid_rev(parts[4]) {
        return None;
    }
    Some(TeamCb {
        op,
        task_id,
        idx,
        ver: parts[3].to_string(),
        rev: parts[4].to_string(),
    })
}

/// Operations of the `/task` intake buttons (Phase 05.4). They live in their own
/// grammar (`decode_intake`), so `decode` and the 05.3 buttons stay as they were.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntakeOp {
    Project,
    Workflow,
    Toggle,
    All,
    None,
    Go,
    Confirm,
    Cancel,
    /// The team step after the workflow step (Phase 6, TEAM-03).
    Team,
}

pub const ALL_INTAKE_OPS: [IntakeOp; 9] = [
    IntakeOp::Project,
    IntakeOp::Workflow,
    IntakeOp::Toggle,
    IntakeOp::All,
    IntakeOp::None,
    IntakeOp::Go,
    IntakeOp::Confirm,
    IntakeOp::Cancel,
    IntakeOp::Team,
];

impl IntakeOp {
    pub fn code(self) -> char {
        match self {
            IntakeOp::Project => 'P',
            IntakeOp::Workflow => 'W',
            IntakeOp::Toggle => 'T',
            IntakeOp::All => 'A',
            IntakeOp::None => 'N',
            IntakeOp::Go => 'G',
            IntakeOp::Confirm => 'C',
            IntakeOp::Cancel => 'X',
            IntakeOp::Team => 'D',
        }
    }

    pub fn from_code(c: char) -> Option<IntakeOp> {
        ALL_INTAKE_OPS.iter().copied().find(|op| op.code() == c)
    }

    /// Choosing from a server-held list carries the index into it.
    pub fn needs_index(self) -> bool {
        matches!(
            self,
            IntakeOp::Project | IntakeOp::Workflow | IntakeOp::Toggle | IntakeOp::Team
        )
    }
}

/// Largest index an intake button can carry.
pub const MAX_INTAKE_INDEX: usize = 99;
const FLOW_LEN: usize = 8;

/// `i{op}:{flow}` or `i{op}:{flow}:{index}`: `flow` is the 8-hex id of the open
/// flow, `index` a position in a list the server holds (never text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntakeCb {
    pub op: IntakeOp,
    pub flow: String,
    pub index: Option<usize>,
}

pub fn encode_intake(cb: &IntakeCb) -> String {
    match (cb.op.needs_index(), cb.index) {
        (true, Some(n)) => format!("i{}:{}:{}", cb.op.code(), cb.flow, n),
        _ => format!("i{}:{}", cb.op.code(), cb.flow),
    }
}

fn valid_flow(s: &str) -> bool {
    s.len() == FLOW_LEN && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Decimal `0..=99`, no sign, no leading zero (except `0` itself).
fn parse_intake_index(s: &str) -> Option<usize> {
    if s.is_empty() || s.len() > 2 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if s.len() == 2 && s.starts_with('0') {
        return None;
    }
    let v: usize = s.parse().ok()?;
    (v <= MAX_INTAKE_INDEX).then_some(v)
}

/// Strict parse: anything not produced by `encode_intake` returns `None`.
pub fn decode_intake(data: &str) -> Option<IntakeCb> {
    if data.is_empty() || data.len() > MAX_LEN {
        return None;
    }
    let parts: Vec<&str> = data.split(':').collect();
    let head = parts.first()?;
    let mut chars = head.chars();
    if chars.next()? != 'i' {
        return None;
    }
    let op = IntakeOp::from_code(chars.next()?)?;
    if chars.next().is_some() {
        return None;
    }
    let want = if op.needs_index() { 3 } else { 2 };
    if parts.len() != want || !valid_flow(parts[1]) {
        return None;
    }
    let index = if op.needs_index() {
        Some(parse_intake_index(parts[2])?)
    } else {
        None
    };
    Some(IntakeCb {
        op,
        flow: parts[1].to_string(),
        index,
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

    const FLOW: &str = "0123abcd";

    #[test]
    fn intake_round_trip_every_op() {
        for op in ALL_INTAKE_OPS {
            let indexes: &[Option<usize>] = if op.needs_index() {
                &[Some(0), Some(7), Some(99)]
            } else {
                &[None]
            };
            for index in indexes {
                let cb = IntakeCb {
                    op,
                    flow: FLOW.to_string(),
                    index: *index,
                };
                let data = encode_intake(&cb);
                assert!(data.len() <= MAX_LEN, "{data}");
                assert_eq!(decode_intake(&data), Some(cb), "{data}");
            }
            assert_eq!(IntakeOp::from_code(op.code()), Some(op));
        }
        assert_eq!(
            encode_intake(&IntakeCb {
                op: IntakeOp::Project,
                flow: FLOW.to_string(),
                index: Some(3)
            }),
            "iP:0123abcd:3"
        );
        assert_eq!(
            encode_intake(&IntakeCb {
                op: IntakeOp::Cancel,
                flow: FLOW.to_string(),
                index: None
            }),
            "iX:0123abcd"
        );
    }

    #[test]
    fn intake_rejects_malformed() {
        for bad in [
            "iP:0123abcd",
            "iC:0123abcd:1",
            "iZ:0123abcd",
            "ip:0123abcd:1",
            "iP:0123ABCD:1",
            "iP:0123abc:1",
            "iP:0123abcd:01",
            "iP:0123abcd:100",
            "iP:0123abcd:-1",
            "i:0123abcd",
            "iP:0123abcd:1:2",
            "iPP:0123abcd:1",
            "iP:0123abcd:+1",
            "iP:0123abcd:",
            "iX:0123abcd:",
            "iP:0123abcde:1",
            "",
        ] {
            assert_eq!(decode_intake(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn intake_grammar_is_separate_from_the_05_3_grammar() {
        assert_eq!(decode("iP:0123abcd:1"), None);
        assert_eq!(decode_intake("a:1:0123456789abcdef"), None);
    }

    const VER: &str = "a1b2";

    #[test]
    fn team_grammar_round_trip() {
        for (op, idx) in [
            (TeamOp::Pick, 0),
            (TeamOp::Pick, 7),
            (TeamOp::Confirm, 0),
            (TeamOp::Confirm, 7),
        ] {
            let cb = TeamCb {
                op,
                task_id: 42,
                idx,
                ver: VER.to_string(),
                rev: REV.to_string(),
            };
            let data = encode_team(&cb);
            assert_eq!(decode_team(&data), Some(cb), "{data}");
        }
        assert_eq!(
            encode_team(&TeamCb {
                op: TeamOp::Pick,
                task_id: 1,
                idx: 2,
                ver: VER.to_string(),
                rev: REV.to_string()
            }),
            format!("t:1:2:{VER}:{REV}")
        );
    }

    #[test]
    fn team_grammar_rejects() {
        let long = format!("t:1:0:{VER}:{REV}{}", "0".repeat(65 - 11 - REV.len()));
        assert_eq!(long.len(), 65);
        for bad in [
            format!("t:1:8:{VER}:{REV}"),
            format!("t:1:10:{VER}:{REV}"),
            format!("t:1:-1:{VER}:{REV}"),
            format!("t:1:0:a1b:{REV}"),
            format!("t:1:0:A1B2:{REV}"),
            format!("t:1:0:a1b2c:{REV}"),
            format!("t:1:0:{VER}:0123456789ABCDEF"),
            format!("t:1:0:{VER}:0123456789abcde"),
            format!("t:1:0:{VER}:{REV}:1"),
            format!("t:1:{VER}:{REV}"),
            format!("t:0:0:{VER}:{REV}"),
            format!("x:1:0:{VER}:{REV}"),
            format!("tt:1:0:{VER}:{REV}"),
            format!("t:1:0:{VER}:"),
            String::new(),
            long,
        ] {
            assert_eq!(decode_team(&bad), None, "{bad:?}");
        }
        // The team grammar is separate from the 05.3 one.
        assert_eq!(decode(&format!("t:1:0:{VER}:{REV}")), None);
        assert_eq!(decode(&format!("u:1:0:{VER}:{REV}")), None);
        assert_eq!(decode_team(&format!("a:1:{REV}")), None);
    }

    #[test]
    fn team_max_len() {
        let cb = TeamCb {
            op: TeamOp::Confirm,
            task_id: i64::MAX,
            idx: 7,
            ver: VER.to_string(),
            rev: REV.to_string(),
        };
        let data = encode_team(&cb);
        assert!(data.len() <= MAX_LEN, "{data}");
        assert_eq!(decode_team(&data), Some(cb));
    }

    #[test]
    fn team_ver_follows_the_list() {
        let a = vec!["mac-dinh".to_string(), "tiet-kiem".to_string()];
        let b = vec![
            "mac-dinh".to_string(),
            "tiet-kiem".to_string(),
            "doi-b".to_string(),
        ];
        let swapped = vec!["tiet-kiem".to_string(), "mac-dinh".to_string()];
        assert_eq!(team_list_ver(&a), team_list_ver(&a));
        assert!(valid_ver(&team_list_ver(&a)));
        assert_ne!(team_list_ver(&a), team_list_ver(&b));
        assert_ne!(team_list_ver(&a), team_list_ver(&swapped));
    }

    #[test]
    fn new_ops_decode() {
        assert_eq!(
            decode(&format!("v:3:{REV}")),
            Some(Callback {
                op: Op::TeamCancel,
                task_id: 3,
                run_id: None,
                rev: REV.to_string()
            })
        );
        assert_eq!(
            decode(&format!("l:3:{REV}")),
            Some(Callback {
                op: Op::RetryLater,
                task_id: 3,
                run_id: None,
                rev: REV.to_string()
            })
        );
        assert_eq!(decode(&format!("v:3:1:{REV}")), None);
        // Every 05.3 code still decodes.
        for c in ["a", "p", "y", "q", "r", "m", "e", "n", "c"] {
            assert!(decode(&format!("{c}:1:{REV}")).is_some(), "{c}");
        }
        for c in ["k", "K", "x"] {
            assert!(decode(&format!("{c}:1:2:{REV}")).is_some(), "{c}");
        }
    }

    #[test]
    fn op_codes_are_unique() {
        let mut codes: Vec<char> = ALL_OPS.iter().map(|o| o.code()).collect();
        codes.extend([TeamOp::Pick.code(), TeamOp::Confirm.code()]);
        let n = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), n, "a callback code is used twice");
        assert_eq!(ALL_OPS.len(), 14);
        for op in ALL_OPS {
            assert_eq!(Op::from_code(op.code()), Some(op));
        }
        // None of the codes is the intake prefix.
        assert!(!codes.contains(&'i'));
    }
}
