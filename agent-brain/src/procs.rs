#[derive(Debug, Clone, Copy)]
pub struct ProcStat {
    pub state: char,
    pub ppid: i32,
    pub pgrp: i32,
    pub start: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcId {
    pub pid: i32,
    pub start: u64,
}

pub fn read_stat(pid: i32) -> Option<ProcStat> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rp = s.rfind(')')?; // comm may contain ')' and spaces
    let f: Vec<&str> = s.get(rp + 2..)?.split_ascii_whitespace().collect();
    // f[0]=state(field 3), f[1]=ppid(field 4), f[2]=pgrp(field 5), f[19]=starttime(field 22)
    Some(ProcStat {
        state: f.first()?.chars().next()?,
        ppid: f.get(1)?.parse().ok()?,
        pgrp: f.get(2)?.parse().ok()?,
        start: f.get(19)?.parse().ok()?,
    })
}

pub fn proc_id(pid: i32) -> Option<ProcId> {
    let s = read_stat(pid)?;
    Some(ProcId {
        pid,
        start: s.start,
    })
}

pub fn alive(p: ProcId) -> bool {
    matches!(read_stat(p.pid), Some(s) if s.start == p.start && s.state != 'Z' && s.state != 'X')
}
