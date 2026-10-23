//! Bảng công việc: suy diễn thuần từ dữ kiện GSD (không file, không database, không đồng hồ).
//!
//! `board_scan` đọc file rồi đưa `Facts` vào đây; `derive` dựng hạng mục, cạnh, trạng thái,
//! wave và các số đếm. Mọi vòng lặp đều tuần tự (không đệ quy) để chuỗi phụ thuộc dài
//! không làm tràn stack.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub const STATUS_XONG: &str = "xong";
pub const STATUS_SAN_SANG: &str = "san_sang";
pub const STATUS_BI_CHAN: &str = "bi_chan";

pub const MAX_ITEMS: usize = 2_500;
pub const MAX_EDGES: usize = 20_000;
pub const MAX_PHASE_LINK_PAIRS: usize = 256;
pub const MAX_WARNINGS: usize = 200;

/// Số phase dạng `5`, `5.4`, `05.10`: so sánh từng thành phần theo số, không bao giờ qua f64.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhaseNum(pub Vec<u64>);

impl PhaseNum {
    pub fn parse(s: &str) -> Option<PhaseNum> {
        let t = s.trim();
        if t.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        for p in t.split('.') {
            if p.is_empty() || p.len() > 6 || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            parts.push(p.parse::<u64>().ok()?);
        }
        Some(PhaseNum(parts))
    }

    /// Thành phần đầu đệm 2 chữ số, phần sau giữ nguyên: [5,4] → "05.4", [5,10] → "05.10".
    pub fn display(&self) -> String {
        let mut it = self.0.iter();
        let mut out = match it.next() {
            Some(first) => format!("{first:02}"),
            None => String::new(),
        };
        for p in it {
            out.push('.');
            out.push_str(&p.to_string());
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoadmapPlanLine {
    pub id: String,
    pub title: String,
    pub checked: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhaseFact {
    pub num: PhaseNum,
    pub name: String,
    pub milestone: Option<String>,
    pub depends_on: Vec<PhaseNum>,
    pub checkbox: Option<bool>,
    pub roadmap_plans: Vec<RoadmapPlanLine>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanFact {
    pub id: String,
    pub phase: PhaseNum,
    pub number: u64,
    pub title: String,
    pub fm_wave: Option<u32>,
    pub depends_on: Vec<String>,
    pub requirements: Vec<String>,
    pub done: bool,
    /// Đường dẫn tương đối trong repo: ".planning/phases/<dir>/<id>-PLAN.md"
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Warning {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub phases: Vec<PhaseFact>,
    pub plans: Vec<PlanFact>,
    pub current_phase: Option<PhaseNum>,
    pub warnings: Vec<Warning>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EpicSel {
    Default,
    All,
    Phase(PhaseNum),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    pub epic: EpicSel,
    pub milestone: Option<String>,
}

// Thứ tự field khai báo dưới đây CHÍNH LÀ thứ tự JSON.
#[derive(Debug, Clone, Serialize)]
pub struct View {
    pub epic: String,
    pub default_epic: String,
    pub epics: Vec<EpicView>,
    pub milestone: Option<String>,
    pub milestones: Vec<String>,
    pub items: Vec<ItemView>,
    pub edges: Vec<EdgeView>,
    pub external: Vec<ExternalView>,
    pub counts: Counts,
    pub warnings: Vec<Warning>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EpicView {
    pub id: String,
    pub name: String,
    pub milestone: Option<String>,
    pub total: usize,
    pub done: usize,
    pub placeholder: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemView {
    pub key: String,
    pub title: String,
    pub epic: String,
    pub epic_name: String,
    pub milestone: Option<String>,
    pub wave: u32,
    pub fm_wave: Option<u32>,
    pub status: &'static str,
    pub critical: bool,
    pub placeholder: bool,
    pub requirements: Vec<String>,
    pub plan_path: Option<String>,
    pub waits_on: Vec<String>,
    pub waited_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeView {
    pub from: String,
    pub to: String,
    /// "plan" | "phase"
    pub kind: &'static str,
    pub critical: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExternalView {
    pub key: String,
    pub title: String,
    pub epic: String,
    pub status: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Counts {
    pub total: usize,
    pub xong: usize,
    pub san_sang: usize,
    pub bi_chan: usize,
    pub waves: usize,
}

/// Chữ điều khiển → khoảng trắng, cắt tại `max_chars` ký tự (đúng ranh giới ký tự), rồi trim.
pub fn clean_text(s: &str, max_chars: usize) -> String {
    let replaced: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(max_chars)
        .collect();
    replaced.trim().to_string()
}

struct Node {
    key: String,
    title: String,
    epic: PhaseNum,
    number: u64,
    fm_wave: Option<u32>,
    requirements: Vec<String>,
    placeholder: bool,
    done: bool,
    plan_path: Option<String>,
    depends: Vec<String>,
}

/// Đường dẫn dùng trong cảnh báo: file plan, hoặc ROADMAP cho hạng mục giữ chỗ.
fn node_path(nd: &Node) -> String {
    nd.plan_path
        .clone()
        .unwrap_or_else(|| ".planning/ROADMAP.md".to_string())
}

/// Mã plan đúng dạng `^\d+(\.\d+)*-\d+$`, không dùng regex để khỏi phụ thuộc vào đầu vào lạ.
fn is_plan_id(s: &str) -> bool {
    let Some((phase, number)) = s.rsplit_once('-') else {
        return false;
    };
    let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    digits(number) && phase.split('.').all(digits)
}

pub fn derive(facts: &Facts, filter: &Filter) -> View {
    // Epic: mỗi phase một epic; phase chỉ có trong thư mục plan được tạo tên từ số phase.
    let mut epic_info: BTreeMap<PhaseNum, (String, Option<String>)> = BTreeMap::new();
    for ph in &facts.phases {
        epic_info
            .entry(ph.num.clone())
            .or_insert_with(|| (ph.name.clone(), ph.milestone.clone()));
    }
    for pl in &facts.plans {
        epic_info
            .entry(pl.phase.clone())
            .or_insert_with(|| (pl.phase.display(), None));
    }

    // Hạng mục: mỗi PlanFact một hạng mục, phase không có plan có một hạng mục giữ chỗ.
    let mut nodes: Vec<Node> = Vec::new();
    for pl in &facts.plans {
        nodes.push(Node {
            key: pl.id.clone(),
            title: pl.title.clone(),
            epic: pl.phase.clone(),
            number: pl.number,
            fm_wave: pl.fm_wave,
            requirements: pl.requirements.clone(),
            placeholder: false,
            done: pl.done,
            plan_path: Some(pl.path.clone()),
            depends: pl.depends_on.clone(),
        });
    }
    let phases_with_plans: HashSet<&PhaseNum> = facts.plans.iter().map(|p| &p.phase).collect();
    let mut placeholder_seen: HashSet<&PhaseNum> = HashSet::new();
    for ph in &facts.phases {
        if phases_with_plans.contains(&ph.num) || !placeholder_seen.insert(&ph.num) {
            continue;
        }
        nodes.push(Node {
            key: ph.num.display(),
            title: ph.name.clone(),
            epic: ph.num.clone(),
            number: 0,
            fm_wave: None,
            requirements: Vec::new(),
            placeholder: true,
            // Giữ chỗ xong khi ô đầu danh sách là [x], hoặc (A7) khi mục phase trong ROADMAP
            // có ít nhất một dòng plan và mọi dòng đều [x] (phase thập phân không có ô đầu danh sách).
            done: ph.checkbox == Some(true)
                || (!ph.roadmap_plans.is_empty() && ph.roadmap_plans.iter().all(|l| l.checked)),
            plan_path: None,
            depends: Vec::new(),
        });
    }
    nodes.sort_by(|a, b| (&a.epic, a.number, &a.key).cmp(&(&b.epic, b.number, &b.key)));

    // Cảnh báo của derive theo đúng thứ tự bước (cảnh báo của scanner đứng trước khi trả về).
    let mut notes: Vec<Warning> = Vec::new();
    let mut truncated = false;

    // Khoá trùng: giữ hạng mục đầu theo thứ tự, cảnh báo ở đường dẫn của file bị bỏ.
    let mut first_path: HashMap<String, String> = HashMap::new();
    let mut kept: Vec<Node> = Vec::with_capacity(nodes.len());
    for nd in nodes {
        match first_path.get(&nd.key) {
            Some(first) => {
                if let Some(path) = &nd.plan_path {
                    notes.push(Warning {
                        path: clean_text(path, 300),
                        message: format!(
                            "mã plan {} trùng với {} — bỏ qua file này",
                            nd.key,
                            clean_text(first, 300)
                        ),
                    });
                }
            }
            None => {
                first_path.insert(nd.key.clone(), nd.plan_path.clone().unwrap_or_default());
                kept.push(nd);
            }
        }
    }
    let mut nodes = kept;

    // Giới hạn số hạng mục; khoá bị bỏ được nhớ để phụ thuộc trỏ tới chúng không bị cảnh báo.
    let mut dropped_keys: HashSet<String> = HashSet::new();
    if nodes.len() > MAX_ITEMS {
        for nd in nodes.drain(MAX_ITEMS..) {
            dropped_keys.insert(nd.key);
        }
        truncated = true;
        notes.push(Warning {
            path: ".planning".to_string(),
            message: format!("quá {MAX_ITEMS} hạng mục — chỉ hiện {MAX_ITEMS} hạng mục đầu"),
        });
    }
    let n = nodes.len();

    let index: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, nd)| (nd.key.as_str(), i))
        .collect();

    // Cạnh tường minh theo thứ tự chèn: (hạng mục bị chờ, hạng mục chờ, cạnh nối phase?).
    let mut edge_list: Vec<(usize, usize, bool)> = Vec::new();
    for (i, nd) in nodes.iter().enumerate() {
        let mut seen: HashSet<usize> = HashSet::new();
        for raw in &nd.depends {
            let name = raw.trim().trim_matches(|c| c == '"' || c == '\'').trim();
            if name == nd.key {
                notes.push(Warning {
                    path: node_path(nd),
                    message: format!("depends_on chứa chính plan {} (bỏ qua)", nd.key),
                });
                continue;
            }
            let found = if is_plan_id(name) {
                index.get(name).copied()
            } else {
                None
            };
            match found {
                Some(d) => {
                    if seen.insert(d) {
                        edge_list.push((d, i, false));
                    }
                }
                None if dropped_keys.contains(name) => {}
                None => notes.push(Warning {
                    path: node_path(nd),
                    message: format!(
                        "depends_on trỏ tới plan không tồn tại: {} (bỏ qua cạnh này)",
                        clean_text(name, 64)
                    ),
                }),
            }
        }
    }

    // D-10: `Depends on: Phase X` của ROADMAP nối mỗi hạng mục cuối của X (không hạng mục nào
    // trong X chờ nó) với mỗi hạng mục gốc của phase này (không có cạnh tường minh đi vào).
    // Hạng mục giữ chỗ vừa là gốc vừa là cuối. Cạnh phase đứng sau cạnh tường minh.
    {
        let mut has_in = vec![false; n];
        let mut has_dependent_in_epic = vec![false; n];
        let mut present: HashSet<(usize, usize)> = HashSet::new();
        for &(from, to, _) in &edge_list {
            has_in[to] = true;
            if nodes[from].epic == nodes[to].epic {
                has_dependent_in_epic[from] = true;
            }
            present.insert((from, to));
        }
        let mut members: HashMap<&PhaseNum, Vec<usize>> = HashMap::new();
        for (i, nd) in nodes.iter().enumerate() {
            members.entry(&nd.epic).or_default().push(i);
        }
        let known: HashSet<&PhaseNum> = facts.phases.iter().map(|p| &p.num).collect();
        let mut phase_order: Vec<&PhaseFact> = facts.phases.iter().collect();
        phase_order.sort_by(|a, b| a.num.cmp(&b.num));
        let mut phase_done: HashSet<&PhaseNum> = HashSet::new();
        for p in phase_order {
            if !phase_done.insert(&p.num) {
                continue;
            }
            let mut linked: HashSet<&PhaseNum> = HashSet::new();
            for x in &p.depends_on {
                if !linked.insert(x) {
                    continue;
                }
                let message = if x == &p.num {
                    format!(
                        "Phase {}: Depends on nhắc chính phase {} (bỏ qua)",
                        p.num.display(),
                        x.display()
                    )
                } else if !known.contains(x) {
                    format!(
                        "Phase {}: Depends on nhắc phase {} không có trong ROADMAP.md (bỏ qua)",
                        p.num.display(),
                        x.display()
                    )
                } else {
                    String::new()
                };
                if !message.is_empty() {
                    notes.push(Warning {
                        path: ".planning/ROADMAP.md".to_string(),
                        message,
                    });
                    continue;
                }
                let roots: Vec<usize> = members
                    .get(&p.num)
                    .map(|m| m.iter().copied().filter(|&i| !has_in[i]).collect())
                    .unwrap_or_default();
                let sinks: Vec<usize> = members
                    .get(x)
                    .map(|m| {
                        m.iter()
                            .copied()
                            .filter(|&i| !has_dependent_in_epic[i])
                            .collect()
                    })
                    .unwrap_or_default();
                let pairs = sinks.len() * roots.len();
                if pairs > MAX_PHASE_LINK_PAIRS {
                    notes.push(Warning {
                        path: ".planning/ROADMAP.md".to_string(),
                        message: format!(
                            "Phase {}: liên kết từ phase {} có {pairs} cặp (giới hạn {MAX_PHASE_LINK_PAIRS}) — bỏ qua liên kết này",
                            p.num.display(),
                            x.display()
                        ),
                    });
                    continue;
                }
                for &s in &sinks {
                    for &r in &roots {
                        if present.insert((s, r)) {
                            edge_list.push((s, r, true));
                        }
                    }
                }
            }
        }
    }

    if edge_list.len() > MAX_EDGES {
        edge_list.truncate(MAX_EDGES);
        truncated = true;
        notes.push(Warning {
            path: ".planning".to_string(),
            message: format!("quá {MAX_EDGES} cạnh — bỏ các cạnh sau"),
        });
    }

    // deps[i] = các hạng mục mà i phải chờ, theo thứ tự hạng mục.
    let mut deps: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut phase_edges: HashSet<(usize, usize)> = HashSet::new();
    for &(from, to, by_phase) in &edge_list {
        deps[to].push(from);
        if by_phase {
            phase_edges.insert((from, to));
        }
    }
    for d in &mut deps {
        d.sort_unstable();
    }

    // Thứ tự topo bằng DFS tuần tự; cạnh quay về nút đang trên stack bị bỏ, mỗi vòng mất đúng
    // một cạnh và có một cảnh báo.
    let mut state = vec![0u8; n]; // 0 mới, 1 đang trên stack, 2 xong
    let mut stack_pos = vec![0usize; n];
    let mut topo: Vec<usize> = Vec::with_capacity(n);
    for root in 0..n {
        if state[root] != 0 {
            continue;
        }
        state[root] = 1;
        stack_pos[root] = 0;
        let mut stack: Vec<(usize, usize)> = vec![(root, 0)];
        while let Some(&(v, i)) = stack.last() {
            if i < deps[v].len() {
                let d = deps[v][i];
                match state[d] {
                    1 => {
                        deps[v].remove(i);
                        phase_edges.remove(&(d, v));
                        let names: Vec<&str> = stack[stack_pos[d]..]
                            .iter()
                            .map(|&(u, _)| nodes[u].key.as_str())
                            .collect();
                        let chain = if names.len() > 6 {
                            format!(
                                "{} cần {} cần {} cần … cần {} cần {}",
                                names[0],
                                names[1],
                                names[2],
                                names[names.len() - 2],
                                names[names.len() - 1]
                            )
                        } else {
                            names.join(" cần ")
                        };
                        notes.push(Warning {
                            path: node_path(&nodes[v]),
                            message: format!(
                                "vòng phụ thuộc: {chain} cần {dep} — đã bỏ phụ thuộc của {item} vào {dep}",
                                dep = nodes[d].key,
                                item = nodes[v].key
                            ),
                        });
                    }
                    2 => {
                        if let Some(top) = stack.last_mut() {
                            top.1 += 1;
                        }
                    }
                    _ => {
                        if let Some(top) = stack.last_mut() {
                            top.1 += 1;
                        }
                        state[d] = 1;
                        stack_pos[d] = stack.len();
                        stack.push((d, 0));
                    }
                }
            } else {
                state[v] = 2;
                topo.push(v);
                stack.pop();
            }
        }
    }

    // D-08: wave ghi trong front matter (tính từ 1) so với đường dài nhất qua cạnh plan cùng
    // phase + 1, trên toàn đồ thị, không phụ thuộc bộ lọc. Chỉ cảnh báo, không dời hạng mục.
    let mut intra = vec![0u32; n];
    for &v in &topo {
        if nodes[v].placeholder {
            continue;
        }
        for &d in &deps[v] {
            if !nodes[d].placeholder
                && nodes[d].epic == nodes[v].epic
                && !phase_edges.contains(&(d, v))
            {
                intra[v] = intra[v].max(intra[d] + 1);
            }
        }
    }
    for v in 0..n {
        if let Some(w) = nodes[v].fm_wave {
            let expected = intra[v] + 1;
            if w != expected {
                notes.push(Warning {
                    path: node_path(&nodes[v]),
                    message: format!(
                        "wave: ghi {w}, tính ra {expected} — chỉ cảnh báo, hạng mục giữ vị trí tính từ phụ thuộc"
                    ),
                });
            }
        }
    }

    // Trạng thái trên đồ thị đầy đủ (D-04).
    let status: Vec<&'static str> = (0..n)
        .map(|i| {
            if nodes[i].done {
                STATUS_XONG
            } else if deps[i].iter().all(|&d| nodes[d].done) {
                STATUS_SAN_SANG
            } else {
                STATUS_BI_CHAN
            }
        })
        .collect();

    // Danh sách epic (tổng và số đã xong tính trên toàn bộ đồ thị, không theo bộ lọc).
    let epics: Vec<EpicView> = epic_info
        .iter()
        .map(|(num, (name, milestone))| {
            let members: Vec<usize> = (0..n).filter(|&i| &nodes[i].epic == num).collect();
            EpicView {
                id: num.display(),
                name: name.clone(),
                milestone: milestone.clone(),
                total: members.len(),
                done: members
                    .iter()
                    .filter(|&&i| status[i] == STATUS_XONG)
                    .count(),
                placeholder: members.len() == 1 && nodes[members[0]].placeholder,
            }
        })
        .collect();

    // Epic mặc định (D-10): current_phase của STATE.md nếu là một epic, nếu không thì epic đầu
    // tiên (theo số phase) chưa xong hết, nếu không còn thì "all".
    let default_sel: Option<PhaseNum> = facts
        .current_phase
        .clone()
        .filter(|p| epic_info.contains_key(p))
        .or_else(|| {
            epic_info
                .keys()
                .zip(epics.iter())
                .find(|(_, e)| e.done < e.total)
                .map(|(num, _)| num.clone())
        });
    let default_epic = default_sel
        .as_ref()
        .map(PhaseNum::display)
        .unwrap_or_else(|| "all".to_string());

    // Epic đang xem. Epic đúng dạng nhưng không tồn tại: rơi về mặc định kèm cảnh báo.
    let mut extra_warnings: Vec<Warning> = Vec::new();
    let selected: Option<PhaseNum> = match &filter.epic {
        EpicSel::All => None,
        EpicSel::Default => default_sel.clone(),
        EpicSel::Phase(p) if epic_info.contains_key(p) => Some(p.clone()),
        EpicSel::Phase(p) => {
            extra_warnings.push(Warning {
                path: ".planning/ROADMAP.md".to_string(),
                message: format!(
                    "Không có epic {} — đang xem epic mặc định {}",
                    p.display(),
                    default_epic
                ),
            });
            default_sel.clone()
        }
    };
    let epic_str = selected
        .as_ref()
        .map(PhaseNum::display)
        .unwrap_or_else(|| "all".to_string());
    // Milestone (D-04b): các milestone riêng biệt theo thứ tự xuất hiện trong ROADMAP; lọc theo
    // milestone chỉ áp dụng khi tên khớp một milestone có thật, nếu không thì bỏ qua kèm cảnh báo.
    let mut distinct_milestones: Vec<String> = Vec::new();
    for ph in &facts.phases {
        if let Some(m) = &ph.milestone {
            if !distinct_milestones.contains(m) {
                distinct_milestones.push(m.clone());
            }
        }
    }
    let applied_milestone: Option<String> = match &filter.milestone {
        Some(m) if distinct_milestones.contains(m) => Some(m.clone()),
        Some(m) => {
            extra_warnings.push(Warning {
                path: ".planning/ROADMAP.md".to_string(),
                message: format!(
                    "Không có milestone {} — đang xem mọi milestone",
                    clean_text(m, 64)
                ),
            });
            None
        }
        None => None,
    };
    let visible: Vec<bool> = nodes
        .iter()
        .map(|nd| {
            let in_epic = match &selected {
                Some(p) => &nd.epic == p,
                None => true,
            };
            let in_milestone = match &applied_milestone {
                Some(m) => {
                    epic_info.get(&nd.epic).and_then(|(_, ms)| ms.as_deref()) == Some(m.as_str())
                }
                None => true,
            };
            in_epic && in_milestone
        })
        .collect();

    // Wave trên đồ thị con nhìn thấy (D-08).
    let mut wave = vec![0u32; n];
    for &v in &topo {
        if !visible[v] {
            continue;
        }
        let mut w = 0u32;
        for &d in &deps[v] {
            if visible[d] {
                w = w.max(wave[d] + 1);
            }
        }
        wave[v] = w;
    }

    // Đường găng trên đồ thị con nhìn thấy (D-09): chuỗi dài nhất qua các hạng mục chưa xong,
    // mỗi hạng mục chưa xong tính 1, hạng mục đã xong tính 0. Hòa thì lấy hạng mục đứng trước
    // theo thứ tự hạng mục (so sánh lớn hơn hẳn), không đệ quy.
    let mut outs: Vec<Vec<usize>> = vec![Vec::new(); n];
    for to in 0..n {
        if !visible[to] {
            continue;
        }
        for &from in &deps[to] {
            if visible[from] {
                outs[from].push(to);
            }
        }
    }
    let mut longest = vec![0u32; n];
    let mut next: Vec<Option<usize>> = vec![None; n];
    for &v in topo.iter().rev() {
        if !visible[v] {
            continue;
        }
        let mut best = 0u32;
        let mut nx = None;
        for &o in &outs[v] {
            if longest[o] > best {
                best = longest[o];
                nx = Some(o);
            }
        }
        longest[v] = best + u32::from(status[v] != STATUS_XONG);
        next[v] = nx;
    }
    let mut start: Option<usize> = None;
    let mut best_len = 0u32;
    for v in 0..n {
        let is_root = visible[v] && deps[v].iter().all(|&d| !visible[d]);
        if is_root && longest[v] > best_len {
            best_len = longest[v];
            start = Some(v);
        }
    }
    let mut chain: Vec<usize> = Vec::new();
    let mut cursor = start;
    while let Some(v) = cursor {
        chain.push(v);
        cursor = next[v];
    }
    // Chỉ tô hạng mục chưa xong trên chuỗi, và cạnh nối hai hạng mục liên tiếp cùng được tô.
    let mut crit_node = vec![false; n];
    for &v in &chain {
        crit_node[v] = status[v] != STATUS_XONG;
    }
    let mut crit_edge: HashSet<(usize, usize)> = HashSet::new();
    for pair in chain.windows(2) {
        if crit_node[pair[0]] && crit_node[pair[1]] {
            crit_edge.insert((pair[0], pair[1]));
        }
    }

    // Láng giềng trên đồ thị đầy đủ sau khi phá vòng, theo thứ tự hạng mục.
    let mut full_outs: Vec<Vec<usize>> = vec![Vec::new(); n];
    for to in 0..n {
        for &from in &deps[to] {
            full_outs[from].push(to);
        }
    }
    let keys_of =
        |list: &[usize]| -> Vec<String> { list.iter().map(|&i| nodes[i].key.clone()).collect() };
    // Láng giềng trực tiếp nằm ngoài khung nhìn, không trùng, theo thứ tự hạng mục.
    let mut outside: BTreeSet<usize> = BTreeSet::new();
    for v in (0..n).filter(|&v| visible[v]) {
        outside.extend(deps[v].iter().copied().filter(|&d| !visible[d]));
        outside.extend(full_outs[v].iter().copied().filter(|&o| !visible[o]));
    }
    let external: Vec<ExternalView> = outside
        .iter()
        .map(|&i| ExternalView {
            key: nodes[i].key.clone(),
            title: nodes[i].title.clone(),
            epic: nodes[i].epic.display(),
            status: status[i],
        })
        .collect();

    let mut order: Vec<usize> = (0..n).filter(|&i| visible[i]).collect();
    order.sort_by_key(|&i| (wave[i], i));
    let items: Vec<ItemView> = order
        .iter()
        .map(|&i| {
            let nd = &nodes[i];
            let name = epic_info
                .get(&nd.epic)
                .map(|(name, _)| name.clone())
                .unwrap_or_else(|| nd.epic.display());
            let milestone = epic_info.get(&nd.epic).and_then(|(_, m)| m.clone());
            ItemView {
                key: nd.key.clone(),
                title: nd.title.clone(),
                epic: nd.epic.display(),
                epic_name: name,
                milestone,
                wave: wave[i],
                fm_wave: nd.fm_wave,
                status: status[i],
                critical: crit_node[i],
                placeholder: nd.placeholder,
                requirements: nd.requirements.clone(),
                plan_path: nd.plan_path.clone(),
                waits_on: keys_of(&deps[i]),
                waited_by: keys_of(&full_outs[i]),
            }
        })
        .collect();

    let mut edge_pairs: Vec<(usize, usize)> = Vec::new();
    for to in 0..n {
        for &from in &deps[to] {
            if visible[from] && visible[to] {
                edge_pairs.push((from, to));
            }
        }
    }
    edge_pairs.sort_unstable();
    let edges: Vec<EdgeView> = edge_pairs
        .iter()
        .map(|&(from, to)| EdgeView {
            from: nodes[from].key.clone(),
            to: nodes[to].key.clone(),
            kind: if phase_edges.contains(&(from, to)) {
                "phase"
            } else {
                "plan"
            },
            critical: crit_edge.contains(&(from, to)),
        })
        .collect();

    let count_of = |s: &str| items.iter().filter(|it| it.status == s).count();
    let waves: BTreeSet<u32> = items.iter().map(|it| it.wave).collect();
    let counts = Counts {
        total: items.len(),
        xong: count_of(STATUS_XONG),
        san_sang: count_of(STATUS_SAN_SANG),
        bi_chan: count_of(STATUS_BI_CHAN),
        waves: waves.len(),
    };

    // Cảnh báo chọn epic (đặt đầu để không bị cắt), rồi của scanner, rồi của derive theo bước.
    let mut warnings = extra_warnings;
    warnings.extend(facts.warnings.iter().cloned());
    warnings.extend(notes);
    warnings.truncate(MAX_WARNINGS);

    View {
        epic: epic_str,
        default_epic,
        epics,
        milestone: applied_milestone,
        milestones: if distinct_milestones.len() >= 2 {
            distinct_milestones
        } else {
            Vec::new()
        },
        items,
        edges,
        external,
        counts,
        warnings,
        truncated: facts.truncated || truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pn(s: &str) -> PhaseNum {
        PhaseNum::parse(s).expect("phase num")
    }

    fn ph(num: &str, name: &str) -> PhaseFact {
        PhaseFact {
            num: pn(num),
            name: name.to_string(),
            milestone: None,
            depends_on: Vec::new(),
            checkbox: None,
            roadmap_plans: Vec::new(),
        }
    }

    fn plan(phase: &str, number: u64, deps: &[&str], done: bool) -> PlanFact {
        let id = format!("{}-{:02}", pn(phase).display(), number);
        PlanFact {
            path: format!(".planning/phases/{phase}-x/{id}-PLAN.md"),
            id: id.clone(),
            phase: pn(phase),
            number,
            title: id,
            fm_wave: None,
            depends_on: deps.iter().map(|d| d.to_string()).collect(),
            requirements: Vec::new(),
            done,
        }
    }

    fn all() -> Filter {
        Filter {
            epic: EpicSel::All,
            milestone: None,
        }
    }

    /// Bản tracer fixture của test HTTP, dưới dạng struct thuần.
    fn tracer_facts() -> Facts {
        Facts {
            phases: vec![
                ph("1", "Nền tảng"),
                ph("2", "Xuất báo cáo"),
                ph("3", "Hướng dẫn"),
            ],
            plans: vec![
                plan("1", 1, &[], true),
                plan("1", 2, &["01-01"], true),
                plan("2", 1, &["01-02"], false),
                plan("2", 2, &["02-01"], false),
            ],
            ..Facts::default()
        }
    }

    fn wave_of(v: &View, key: &str) -> u32 {
        v.items.iter().find(|i| i.key == key).expect(key).wave
    }

    fn status_of(v: &View, key: &str) -> &'static str {
        v.items.iter().find(|i| i.key == key).expect(key).status
    }

    #[test]
    fn phase_num_orders_numerically_and_displays_padded() {
        assert!(pn("05.9") < pn("05.10"));
        assert_eq!(pn("5.4").display(), "05.4");
        assert_eq!(pn("05.10").display(), "05.10");
        assert_eq!(pn("1").display(), "01");
        assert_eq!(PhaseNum::parse("x.y"), None);
        assert_eq!(PhaseNum::parse(""), None);
        assert_eq!(PhaseNum::parse("1..2"), None);
        assert_eq!(PhaseNum::parse("1234567"), None);
    }

    #[test]
    fn clean_text_strips_controls_and_cuts_on_char_boundary() {
        assert_eq!(clean_text("a\nb\tc", 10), "a b c");
        assert_eq!(clean_text("  Việt Nam  ", 7), "Việt");
        assert_eq!(clean_text("\u{7}x", 10), "x");
    }

    #[test]
    fn items_two_phase_fixture() {
        let v = derive(&tracer_facts(), &all());
        let keys: Vec<&str> = v.items.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(keys, ["01-01", "03", "01-02", "02-01", "02-02"]);
        assert_eq!(status_of(&v, "01-01"), STATUS_XONG);
        assert_eq!(status_of(&v, "01-02"), STATUS_XONG);
        assert_eq!(status_of(&v, "02-01"), STATUS_SAN_SANG);
        assert_eq!(status_of(&v, "02-02"), STATUS_BI_CHAN);
        assert_eq!(status_of(&v, "03"), STATUS_SAN_SANG);
        assert_eq!(wave_of(&v, "01-01"), 0);
        assert_eq!(wave_of(&v, "03"), 0);
        assert_eq!(wave_of(&v, "01-02"), 1);
        assert_eq!(wave_of(&v, "02-01"), 2);
        assert_eq!(wave_of(&v, "02-02"), 3);
        let ph3 = v.items.iter().find(|i| i.key == "03").unwrap();
        assert!(ph3.placeholder);
        assert_eq!(ph3.plan_path, None);
        assert_eq!(ph3.title, "Hướng dẫn");
        let edges: Vec<(&str, &str)> = v
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect();
        assert_eq!(
            edges,
            [("01-01", "01-02"), ("01-02", "02-01"), ("02-01", "02-02")]
        );
        assert!(v.edges.iter().all(|e| e.kind == "plan"));
        assert_eq!(v.counts.total, 5);
        assert_eq!(v.counts.xong, 2);
        assert_eq!(v.counts.san_sang, 2);
        assert_eq!(v.counts.bi_chan, 1);
        assert_eq!(v.counts.waves, 4);
        let epics: Vec<(&str, usize, usize)> = v
            .epics
            .iter()
            .map(|e| (e.id.as_str(), e.total, e.done))
            .collect();
        assert_eq!(epics, [("01", 2, 2), ("02", 2, 0), ("03", 1, 0)]);
        assert_eq!(v.epic, "all");
    }

    #[test]
    fn waves_epic_filter_recomputes() {
        let f = Filter {
            epic: EpicSel::Phase(pn("2")),
            milestone: None,
        };
        let v = derive(&tracer_facts(), &f);
        assert_eq!(v.epic, "02");
        assert_eq!(wave_of(&v, "02-01"), 0);
        assert_eq!(wave_of(&v, "02-02"), 1);
        assert_eq!(v.items.len(), 2);
        assert_eq!(v.edges.len(), 1);
        // 02-01 vẫn sẵn sàng vì 01-02 ở epic khác đã xong; trạng thái tính trên đồ thị đầy đủ.
        assert_eq!(status_of(&v, "02-01"), STATUS_SAN_SANG);
    }

    #[test]
    fn dependency_in_other_epic_still_blocks() {
        let mut f = tracer_facts();
        f.plans[1].done = false; // 01-02 chưa xong
        let sel = Filter {
            epic: EpicSel::Phase(pn("2")),
            milestone: None,
        };
        let v = derive(&f, &sel);
        assert_eq!(status_of(&v, "02-01"), STATUS_BI_CHAN);
    }

    #[test]
    fn cycle_edge_is_dropped_and_items_stay() {
        let f = Facts {
            phases: vec![ph("1", "A")],
            plans: vec![
                plan("1", 1, &["01-02"], false),
                plan("1", 2, &["01-01"], false),
                plan("1", 3, &[], false),
            ],
            ..Facts::default()
        };
        let v = derive(&f, &all());
        assert_eq!(v.items.len(), 3);
        assert_eq!(v.edges.len(), 1);
    }

    #[test]
    fn unknown_self_duplicate_and_quoted_deps_are_handled() {
        let f = Facts {
            phases: vec![ph("1", "A")],
            plans: vec![
                plan("1", 1, &[], false),
                plan("1", 2, &["\"01-01\"", "01-01", "01-02", "01-99"], false),
            ],
            ..Facts::default()
        };
        let v = derive(&f, &all());
        assert_eq!(v.edges.len(), 1);
        assert_eq!(status_of(&v, "01-02"), STATUS_BI_CHAN);
    }

    #[test]
    fn placeholder_done_follows_checkbox() {
        let mut p = ph("4", "Sau");
        p.checkbox = Some(true);
        let f = Facts {
            phases: vec![p],
            ..Facts::default()
        };
        let v = derive(&f, &all());
        assert_eq!(status_of(&v, "04"), STATUS_XONG);
        assert!(v.epics[0].placeholder);
    }

    #[test]
    fn plan_phase_missing_from_roadmap_gets_padded_epic_name() {
        let f = Facts {
            plans: vec![plan("7", 1, &[], false)],
            ..Facts::default()
        };
        let v = derive(&f, &all());
        assert_eq!(v.epics[0].id, "07");
        assert_eq!(v.epics[0].name, "07");
    }

    fn default_filter() -> Filter {
        Filter {
            epic: EpicSel::Default,
            milestone: None,
        }
    }

    #[test]
    fn default_epic_from_state() {
        let mut f = tracer_facts();
        f.current_phase = Some(pn("2"));
        let v = derive(&f, &default_filter());
        assert_eq!(v.default_epic, "02");
        assert_eq!(v.epic, "02");
        assert_eq!(v.items.len(), 2);

        f.current_phase = Some(pn("1"));
        let v = derive(&f, &default_filter());
        assert_eq!(v.default_epic, "01");
        assert_eq!(v.epic, "01");
    }

    #[test]
    fn default_epic_first_not_done() {
        let mut f = tracer_facts();
        // current_phase không phải epic nào: dùng epic đầu tiên chưa xong.
        f.current_phase = Some(pn("9"));
        let v = derive(&f, &default_filter());
        assert_eq!(v.default_epic, "02");
        assert_eq!(v.epic, "02");

        f.current_phase = None;
        let v = derive(&f, &default_filter());
        assert_eq!(v.default_epic, "02");
    }

    #[test]
    fn default_epic_all_done_is_all() {
        let mut f = tracer_facts();
        for p in &mut f.plans {
            p.done = true;
        }
        f.phases[2].checkbox = Some(true);
        let v = derive(&f, &default_filter());
        assert_eq!(v.default_epic, "all");
        assert_eq!(v.epic, "all");
        assert_eq!(v.items.len(), 5);
    }

    #[test]
    fn default_epic_unknown_falls_back() {
        let f = tracer_facts();
        let sel = Filter {
            epic: EpicSel::Phase(pn("9")),
            milestone: None,
        };
        let v = derive(&f, &sel);
        assert_eq!(v.epic, v.default_epic);
        assert_eq!(v.epic, "02");
        assert!(v.warnings.iter().any(|w| {
            w.message == "Không có epic 09 — đang xem epic mặc định 02"
                && w.path == ".planning/ROADMAP.md"
        }));
        assert_eq!(v.items.len(), 2);
    }

    #[test]
    fn explicit_epic_does_not_follow_default() {
        let mut f = tracer_facts();
        f.current_phase = Some(pn("1"));
        let sel = Filter {
            epic: EpicSel::Phase(pn("2")),
            milestone: None,
        };
        let v = derive(&f, &sel);
        assert_eq!(v.epic, "02");
        assert_eq!(v.default_epic, "01");
        assert!(v.warnings.is_empty());
    }

    /// Dữ liệu SHOP của bản mẫu thiết kế (SHOP-01..14 thành 01-01..01-14), 01-01 và 01-02 đã xong.
    fn shop_facts() -> Facts {
        let table: [(u64, &[&str]); 14] = [
            (1, &[]),
            (2, &[]),
            (3, &[]),
            (4, &["01-01"]),
            (5, &["01-01", "01-02"]),
            (6, &[]),
            (7, &["01-03", "01-04"]),
            (8, &["01-07", "01-06"]),
            (9, &["01-08"]),
            (10, &["01-07"]),
            (11, &["01-05", "01-07"]),
            (12, &["01-08"]),
            (13, &["01-09", "01-11"]),
            (14, &["01-09"]),
        ];
        Facts {
            phases: vec![ph("1", "Xuất báo cáo đơn hàng")],
            plans: table
                .iter()
                .map(|(num, deps)| plan("1", *num, deps, *num <= 2))
                .collect(),
            ..Facts::default()
        }
    }

    fn critical_keys(v: &View) -> Vec<&str> {
        let mut keys: Vec<&str> = v
            .items
            .iter()
            .filter(|i| i.critical)
            .map(|i| i.key.as_str())
            .collect();
        keys.sort_unstable();
        keys
    }

    fn critical_edges(v: &View) -> Vec<(&str, &str)> {
        let mut edges: Vec<(&str, &str)> = v
            .edges
            .iter()
            .filter(|e| e.critical)
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect();
        edges.sort_unstable();
        edges
    }

    #[test]
    fn critical_shop_golden() {
        let v = derive(&shop_facts(), &all());
        let waves: Vec<(&str, u32)> = [
            "01-01", "01-02", "01-03", "01-04", "01-05", "01-06", "01-07",
        ]
        .iter()
        .chain(
            [
                "01-08", "01-09", "01-10", "01-11", "01-12", "01-13", "01-14",
            ]
            .iter(),
        )
        .map(|k| (*k, wave_of(&v, k)))
        .collect();
        assert_eq!(
            waves,
            [
                ("01-01", 0),
                ("01-02", 0),
                ("01-03", 0),
                ("01-04", 1),
                ("01-05", 1),
                ("01-06", 0),
                ("01-07", 2),
                ("01-08", 3),
                ("01-09", 4),
                ("01-10", 3),
                ("01-11", 3),
                ("01-12", 4),
                ("01-13", 5),
                ("01-14", 5),
            ]
        );
        for k in ["01-01", "01-02"] {
            assert_eq!(status_of(&v, k), STATUS_XONG, "{k}");
        }
        for k in ["01-03", "01-04", "01-05", "01-06"] {
            assert_eq!(status_of(&v, k), STATUS_SAN_SANG, "{k}");
        }
        for k in [
            "01-07", "01-08", "01-09", "01-10", "01-11", "01-12", "01-13", "01-14",
        ] {
            assert_eq!(status_of(&v, k), STATUS_BI_CHAN, "{k}");
        }
        assert_eq!(
            critical_keys(&v),
            ["01-04", "01-07", "01-08", "01-09", "01-13"]
        );
        assert_eq!(
            critical_edges(&v),
            [
                ("01-04", "01-07"),
                ("01-07", "01-08"),
                ("01-08", "01-09"),
                ("01-09", "01-13"),
            ]
        );

        let json = serde_json::to_value(&v).unwrap();
        let items = json["items"].as_array().unwrap();
        let keys: Vec<&str> = items.iter().map(|i| i["key"].as_str().unwrap()).collect();
        assert_eq!(
            &keys[..6],
            ["01-01", "01-02", "01-03", "01-06", "01-04", "01-05"]
        );
        let crit_json: Vec<&str> = items
            .iter()
            .filter(|i| i["critical"] == true)
            .map(|i| i["key"].as_str().unwrap())
            .collect();
        let mut crit_json_sorted = crit_json.clone();
        crit_json_sorted.sort_unstable();
        assert_eq!(
            crit_json_sorted,
            ["01-04", "01-07", "01-08", "01-09", "01-13"]
        );
        let done_on_chain = items.iter().find(|i| i["key"] == "01-01").unwrap();
        assert_eq!(done_on_chain["critical"], false);
        let crit_edges_json = json["edges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["critical"] == true)
            .count();
        assert_eq!(crit_edges_json, 4);
        let edge_0104 = json["edges"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["from"] == "01-01" && e["to"] == "01-04")
            .unwrap();
        assert_eq!(edge_0104["critical"], false);
    }

    #[test]
    fn critical_tie_goes_to_earliest() {
        let f = Facts {
            phases: vec![ph("1", "A")],
            plans: vec![
                plan("1", 1, &[], false),
                plan("1", 2, &["01-01"], false),
                plan("1", 3, &[], false),
                plan("1", 4, &["01-03"], false),
            ],
            ..Facts::default()
        };
        let v = derive(&f, &all());
        assert_eq!(critical_keys(&v), ["01-01", "01-02"]);
        assert_eq!(critical_edges(&v), [("01-01", "01-02")]);
    }

    #[test]
    fn critical_all_done_is_empty() {
        let mut f = shop_facts();
        for p in &mut f.plans {
            p.done = true;
        }
        let v = derive(&f, &all());
        assert!(critical_keys(&v).is_empty());
        assert!(critical_edges(&v).is_empty());
    }

    #[test]
    fn critical_follows_the_view() {
        // 01-08..01-14 của SHOP đổi thành 02-01..02-07 trong phase 02, giữ nguyên phụ thuộc.
        let mut f = shop_facts();
        f.phases.push(ph("2", "Giai đoạn hai"));
        let moved: [(u64, &[&str]); 7] = [
            (1, &["01-07", "01-06"]),
            (2, &["02-01"]),
            (3, &["01-07"]),
            (4, &["01-05", "01-07"]),
            (5, &["02-01"]),
            (6, &["02-02", "02-04"]),
            (7, &["02-02"]),
        ];
        f.plans.truncate(7);
        for (num, deps) in moved {
            f.plans.push(plan("2", num, deps, false));
        }
        let sel = Filter {
            epic: EpicSel::Phase(pn("2")),
            milestone: None,
        };
        let v = derive(&f, &sel);
        assert_eq!(v.items.len(), 7);
        assert_eq!(critical_keys(&v), ["02-01", "02-02", "02-06"]);
        assert_eq!(critical_edges(&v), [("02-01", "02-02"), ("02-02", "02-06")]);
    }

    #[test]
    fn critical_deterministic() {
        let a = serde_json::to_string(&derive(&shop_facts(), &all())).unwrap();
        let b = serde_json::to_string(&derive(&shop_facts(), &all())).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn critical_deep_chain_small_stack() {
        let handle = std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut plans = vec![plan("1", 1, &[], false)];
                for i in 2..=2_400u64 {
                    let prev = format!("01-{:02}", i - 1);
                    plans.push(plan("1", i, &[prev.as_str()], false));
                }
                let f = Facts {
                    phases: vec![ph("1", "Chuỗi dài")],
                    plans,
                    ..Facts::default()
                };
                let v = derive(&f, &all());
                let last = v.items.iter().map(|i| i.wave).max().unwrap();
                let crit = v.items.iter().filter(|i| i.critical).count();
                (v.items.len(), last, crit)
            })
            .unwrap();
        let (items, last_wave, crit) = handle.join().expect("derive không được tràn stack");
        assert_eq!(items, 2_400);
        assert_eq!(last_wave, 2_399);
        assert_eq!(crit, 2_400);
    }

    fn warning_at(v: &View, path: &str, message: &str) -> bool {
        v.warnings
            .iter()
            .any(|w| w.path == path && w.message == message)
    }

    fn edge_pairs_of(v: &View) -> Vec<(&str, &str)> {
        v.edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect()
    }

    fn phase_two(plans: Vec<PlanFact>) -> Facts {
        Facts {
            phases: vec![ph("2", "Giai đoạn hai")],
            plans,
            ..Facts::default()
        }
    }

    #[test]
    fn warn_unknown_dep_dropped_not_blocking() {
        let f = phase_two(vec![plan("2", 1, &["02-99"], false)]);
        let path = f.plans[0].path.clone();
        let v = derive(&f, &all());
        assert!(v.edges.is_empty());
        assert_eq!(v.items.len(), 1);
        assert_eq!(status_of(&v, "02-01"), STATUS_SAN_SANG);
        assert_eq!(v.warnings.len(), 1);
        assert!(warning_at(
            &v,
            &path,
            "depends_on trỏ tới plan không tồn tại: 02-99 (bỏ qua cạnh này)"
        ));
    }

    #[test]
    fn warn_self_and_malformed_deps() {
        let f = phase_two(vec![
            plan("2", 1, &["02-01"], false),
            plan("2", 2, &["Phase 2"], false),
        ]);
        let (p1, p2) = (f.plans[0].path.clone(), f.plans[1].path.clone());
        let v = derive(&f, &all());
        assert!(v.edges.is_empty());
        assert_eq!(status_of(&v, "02-01"), STATUS_SAN_SANG);
        assert_eq!(status_of(&v, "02-02"), STATUS_SAN_SANG);
        assert_eq!(v.warnings.len(), 2);
        assert!(warning_at(
            &v,
            &p1,
            "depends_on chứa chính plan 02-01 (bỏ qua)"
        ));
        assert!(warning_at(
            &v,
            &p2,
            "depends_on trỏ tới plan không tồn tại: Phase 2 (bỏ qua cạnh này)"
        ));
    }

    #[test]
    fn warn_duplicate_dep_collapsed() {
        let f = phase_two(vec![
            plan("2", 1, &[], false),
            plan("2", 2, &["02-01", "\"02-01\"", "02-01"], false),
        ]);
        let v = derive(&f, &all());
        assert_eq!(edge_pairs_of(&v), [("02-01", "02-02")]);
        assert!(v.warnings.is_empty());
    }

    #[test]
    fn warn_cycle_breaks_one_edge() {
        let f = phase_two(vec![
            plan("2", 1, &["02-02"], false),
            plan("2", 2, &["02-01"], false),
            plan("2", 3, &["02-01"], false),
        ]);
        let p2 = f.plans[1].path.clone();
        let v = derive(&f, &all());
        assert_eq!(v.items.len(), 3);
        assert_eq!(v.warnings.len(), 1);
        assert!(warning_at(
            &v,
            &p2,
            "vòng phụ thuộc: 02-01 cần 02-02 cần 02-01 — đã bỏ phụ thuộc của 02-02 vào 02-01"
        ));
        assert_eq!(edge_pairs_of(&v), [("02-01", "02-03"), ("02-02", "02-01")]);
        assert_eq!(status_of(&v, "02-02"), STATUS_SAN_SANG);
        assert_eq!(status_of(&v, "02-01"), STATUS_BI_CHAN);
        assert_eq!(status_of(&v, "02-03"), STATUS_BI_CHAN);
    }

    #[test]
    fn warn_cycle_long_path_is_abbreviated() {
        let mut plans = Vec::new();
        for i in 1..=10u64 {
            let next = format!("01-{:02}", if i == 10 { 1 } else { i + 1 });
            plans.push(plan("1", i, &[next.as_str()], false));
        }
        let f = Facts {
            phases: vec![ph("1", "Vòng dài")],
            plans,
            ..Facts::default()
        };
        let v = derive(&f, &all());
        assert_eq!(v.items.len(), 10);
        assert_eq!(v.edges.len(), 9);
        assert_eq!(v.warnings.len(), 1);
        assert_eq!(
            v.warnings[0].message,
            "vòng phụ thuộc: 01-01 cần 01-02 cần 01-03 cần … cần 01-09 cần 01-10 cần 01-01 — đã bỏ phụ thuộc của 01-10 vào 01-01"
        );
    }

    #[test]
    fn warn_duplicate_plan_id_keeps_first() {
        let first = ".planning/phases/01-a/01-01-PLAN.md";
        let second = ".planning/phases/01-zz/01-01-PLAN.md";
        let mut a = plan("1", 1, &[], false);
        a.path = first.to_string();
        let mut b = plan("1", 1, &[], true);
        b.path = second.to_string();
        let f = Facts {
            phases: vec![ph("1", "A")],
            plans: vec![a, b],
            ..Facts::default()
        };
        let v = derive(&f, &all());
        assert_eq!(v.items.len(), 1);
        assert_eq!(v.items[0].plan_path.as_deref(), Some(first));
        assert_eq!(status_of(&v, "01-01"), STATUS_SAN_SANG);
        assert_eq!(v.warnings.len(), 1);
        assert!(warning_at(
            &v,
            second,
            "mã plan 01-01 trùng với .planning/phases/01-a/01-01-PLAN.md — bỏ qua file này"
        ));
    }

    #[test]
    fn waves_fm_wave_mismatch_warns() {
        let mut second = plan("2", 2, &["02-01"], false);
        second.fm_wave = Some(5);
        let path = second.path.clone();
        let f = phase_two(vec![plan("2", 1, &[], false), second]);
        let v = derive(&f, &all());
        assert_eq!(v.warnings.len(), 1);
        assert!(warning_at(
            &v,
            &path,
            "wave: ghi 5, tính ra 2 — chỉ cảnh báo, hạng mục giữ vị trí tính từ phụ thuộc"
        ));
        assert_eq!(wave_of(&v, "02-02"), 1);
        assert_eq!(wave_of(&v, "02-01"), 0);
    }

    #[test]
    fn waves_fm_wave_agrees_one_based() {
        let mut first = plan("2", 1, &["01-02"], false);
        first.fm_wave = Some(1);
        let mut second = plan("2", 2, &["02-01"], false);
        second.fm_wave = Some(2);
        let mut f = phase_two(vec![first, second]);
        f.phases.insert(0, ph("1", "Một"));
        f.plans.insert(0, plan("1", 1, &[], true));
        f.plans.insert(1, plan("1", 2, &["01-01"], true));
        let v = derive(&f, &all());
        assert!(v.warnings.is_empty(), "{:?}", v.warnings);
        // wave hiển thị vẫn tính cả cạnh sang phase khác.
        assert_eq!(wave_of(&v, "02-01"), 2);
        assert_eq!(wave_of(&v, "02-02"), 3);
    }

    #[test]
    fn status_standin_checkbox_and_plan_lines() {
        let line = |checked: bool| RoadmapPlanLine {
            id: "x-01".to_string(),
            title: "t".to_string(),
            checked,
        };
        let mut p1 = ph("1", "Có dấu tích");
        p1.checkbox = Some(true);
        let mut p2 = ph("2", "Mọi dòng plan đã xong");
        p2.roadmap_plans = vec![line(true), line(true)];
        let mut p3 = ph("3", "Còn một dòng plan chưa xong");
        p3.roadmap_plans = vec![line(true), line(false)];
        let mut p4 = ph("4", "Không dấu tích, không dòng plan");
        p4.checkbox = Some(false);
        let f = Facts {
            phases: vec![p1, p2, p3, p4],
            ..Facts::default()
        };
        let v = derive(&f, &all());
        assert_eq!(status_of(&v, "01"), STATUS_XONG);
        assert_eq!(status_of(&v, "02"), STATUS_XONG);
        assert_eq!(status_of(&v, "03"), STATUS_SAN_SANG);
        assert_eq!(status_of(&v, "04"), STATUS_SAN_SANG);
    }

    #[test]
    fn items_cap_truncates() {
        let mut plans = vec![plan("1", 1, &["01-2501"], false)];
        for i in 2..=2_501u64 {
            plans.push(plan("1", i, &[], false));
        }
        let f = Facts {
            phases: vec![ph("1", "Nhiều")],
            plans,
            ..Facts::default()
        };
        let v = derive(&f, &all());
        assert_eq!(v.items.len(), 2_500);
        assert!(v.truncated);
        assert_eq!(v.warnings.len(), 1, "{:?}", v.warnings);
        assert!(warning_at(
            &v,
            ".planning",
            "quá 2500 hạng mục — chỉ hiện 2500 hạng mục đầu"
        ));
        assert!(v.items.iter().all(|i| i.key != "01-2501"));
        assert!(v.edges.is_empty());
    }

    #[test]
    fn edges_cap_truncates() {
        let mut plans: Vec<PlanFact> = Vec::new();
        for i in 1..=203u64 {
            let earlier: Vec<String> = (1..i).map(|j| format!("01-{j:02}")).collect();
            let refs: Vec<&str> = earlier.iter().map(String::as_str).collect();
            plans.push(plan("1", i, &refs, false));
        }
        let f = Facts {
            phases: vec![ph("1", "Dày")],
            plans,
            ..Facts::default()
        };
        let v = derive(&f, &all());
        assert_eq!(v.items.len(), 203);
        assert_eq!(v.edges.len(), 20_000);
        assert!(v.truncated);
        assert_eq!(v.warnings.len(), 1, "{:?}", v.warnings);
        assert!(warning_at(
            &v,
            ".planning",
            "quá 20000 cạnh — bỏ các cạnh sau"
        ));
    }

    #[test]
    fn warn_cap_200() {
        let mut f = tracer_facts();
        f.warnings = (0..250)
            .map(|i| Warning {
                path: format!(".planning/phases/x/{i}-PLAN.md"),
                message: format!("lỗi {i}"),
            })
            .collect();
        let v = derive(&f, &all());
        assert_eq!(v.warnings.len(), 200);
        assert_eq!(v.warnings[0].message, "lỗi 0");
        assert_eq!(v.warnings[199].message, "lỗi 199");
    }

    fn ph_after(num: &str, name: &str, after: &[&str]) -> PhaseFact {
        let mut p = ph(num, name);
        p.depends_on = after.iter().map(|a| pn(a)).collect();
        p
    }

    fn with_milestone(mut p: PhaseFact, milestone: &str) -> PhaseFact {
        p.milestone = Some(milestone.to_string());
        p
    }

    fn edge_kinds(v: &View) -> Vec<(&str, &str, &str)> {
        v.edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str(), e.kind))
            .collect()
    }

    /// Phase 1 (01-01, 01-02 cần 01-01), phase 2 sau phase 1 (02-01, 02-02), phase 3 sau phase 2, không plan.
    fn linked_facts() -> Facts {
        Facts {
            phases: vec![
                ph("1", "Một"),
                ph_after("2", "Hai", &["1"]),
                ph_after("3", "Ba", &["2"]),
            ],
            plans: vec![
                plan("1", 1, &[], false),
                plan("1", 2, &["01-01"], false),
                plan("2", 1, &[], false),
                plan("2", 2, &[], false),
            ],
            ..Facts::default()
        }
    }

    #[test]
    fn phase_links_sinks_to_roots() {
        let v = derive(&linked_facts(), &all());
        assert_eq!(
            edge_kinds(&v),
            [
                ("01-01", "01-02", "plan"),
                ("01-02", "02-01", "phase"),
                ("01-02", "02-02", "phase"),
                ("02-01", "03", "phase"),
                ("02-02", "03", "phase"),
            ]
        );
        for (key, w) in [
            ("01-01", 0),
            ("01-02", 1),
            ("02-01", 2),
            ("02-02", 2),
            ("03", 3),
        ] {
            assert_eq!(wave_of(&v, key), w, "{key}");
        }
        assert_eq!(status_of(&v, "01-01"), STATUS_SAN_SANG);
        for key in ["01-02", "02-01", "02-02", "03"] {
            assert_eq!(status_of(&v, key), STATUS_BI_CHAN, "{key}");
        }
        assert!(v.warnings.is_empty(), "{:?}", v.warnings);
    }

    #[test]
    fn phase_links_explicit_cross_phase_suppresses_root() {
        let mut f = linked_facts();
        f.plans[2] = plan("2", 1, &["01-01"], false);
        let v = derive(&f, &all());
        assert_eq!(
            edge_kinds(&v),
            [
                ("01-01", "01-02", "plan"),
                ("01-01", "02-01", "plan"),
                ("01-02", "02-02", "phase"),
                ("02-01", "03", "phase"),
                ("02-02", "03", "phase"),
            ]
        );
        assert_eq!(wave_of(&v, "02-01"), 1);
        assert_eq!(wave_of(&v, "02-02"), 2);
        assert_eq!(status_of(&v, "02-01"), STATUS_BI_CHAN);
    }

    #[test]
    fn phase_links_unknown_phase_warns() {
        let f = Facts {
            phases: vec![ph("1", "Một"), ph_after("2", "Hai", &["9", "2"])],
            plans: vec![plan("1", 1, &[], false), plan("2", 1, &[], false)],
            ..Facts::default()
        };
        let v = derive(&f, &all());
        assert!(v.edges.is_empty());
        assert_eq!(v.warnings.len(), 2, "{:?}", v.warnings);
        assert!(warning_at(
            &v,
            ".planning/ROADMAP.md",
            "Phase 02: Depends on nhắc phase 09 không có trong ROADMAP.md (bỏ qua)"
        ));
        assert!(warning_at(
            &v,
            ".planning/ROADMAP.md",
            "Phase 02: Depends on nhắc chính phase 02 (bỏ qua)"
        ));
        assert_eq!(status_of(&v, "02-01"), STATUS_SAN_SANG);
    }

    #[test]
    fn phase_links_fanout_cap() {
        let mut plans = Vec::new();
        for i in 1..=17u64 {
            plans.push(plan("1", i, &[], false));
        }
        for i in 1..=16u64 {
            plans.push(plan("2", i, &[], false));
        }
        let f = Facts {
            phases: vec![ph("1", "Rộng"), ph_after("2", "Sau", &["1"])],
            plans,
            ..Facts::default()
        };
        let v = derive(&f, &all());
        assert!(v.edges.is_empty());
        assert_eq!(v.warnings.len(), 1, "{:?}", v.warnings);
        assert!(warning_at(
            &v,
            ".planning/ROADMAP.md",
            "Phase 02: liên kết từ phase 01 có 272 cặp (giới hạn 256) — bỏ qua liên kết này"
        ));
    }

    #[test]
    fn items_external_and_neighbours() {
        let mut f = linked_facts();
        f.plans[0].done = true;
        f.plans[1].done = true;
        let sel = Filter {
            epic: EpicSel::Phase(pn("2")),
            milestone: None,
        };
        let v = derive(&f, &sel);
        assert_eq!(v.items.len(), 2);
        assert_eq!(wave_of(&v, "02-01"), 0);
        assert_eq!(wave_of(&v, "02-02"), 0);
        assert!(v.edges.is_empty());
        let ext: Vec<(&str, &str, &str, &str)> = v
            .external
            .iter()
            .map(|e| (e.key.as_str(), e.title.as_str(), e.epic.as_str(), e.status))
            .collect();
        assert_eq!(
            ext,
            [
                ("01-02", "01-02", "01", STATUS_XONG),
                ("03", "Ba", "03", STATUS_BI_CHAN)
            ]
        );
        let first = v.items.iter().find(|i| i.key == "02-01").unwrap();
        assert_eq!(first.waits_on, ["01-02"]);
        assert_eq!(first.waited_by, ["03"]);
        assert_eq!(status_of(&v, "02-01"), STATUS_SAN_SANG);
        // Mọi epic: cùng danh sách láng giềng, không còn mục ngoài khung nhìn.
        let all_view = derive(&f, &all());
        assert!(all_view.external.is_empty());
        let third = all_view.items.iter().find(|i| i.key == "03").unwrap();
        assert_eq!(third.waits_on, ["02-01", "02-02"]);
        assert!(third.waited_by.is_empty());
    }

    fn milestone_facts() -> Facts {
        Facts {
            phases: vec![
                with_milestone(ph("1", "Một"), "v1.0 MVP"),
                with_milestone(ph("2", "Hai"), "v1.0 MVP"),
                with_milestone(ph("3", "Ba"), "v1.1 Xuất báo cáo"),
            ],
            plans: vec![plan("1", 1, &[], true), plan("2", 1, &["01-01"], false)],
            ..Facts::default()
        }
    }

    #[test]
    fn milestones_list_and_filter() {
        let f = milestone_facts();
        let v = derive(&f, &all());
        assert_eq!(v.milestones, ["v1.0 MVP", "v1.1 Xuất báo cáo"]);
        assert_eq!(v.milestone, None);
        assert_eq!(v.items.len(), 3);
        assert_eq!(v.items[0].milestone.as_deref(), Some("v1.0 MVP"));
        let sel = Filter {
            epic: EpicSel::All,
            milestone: Some("v1.0 MVP".to_string()),
        };
        let v = derive(&f, &sel);
        assert_eq!(v.milestone.as_deref(), Some("v1.0 MVP"));
        assert_eq!(v.milestones, ["v1.0 MVP", "v1.1 Xuất báo cáo"]);
        let keys: Vec<&str> = v.items.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(keys, ["01-01", "02-01"]);
        assert!(v.warnings.is_empty(), "{:?}", v.warnings);
        let epic_ms: Vec<Option<&str>> = v.epics.iter().map(|e| e.milestone.as_deref()).collect();
        assert_eq!(
            epic_ms,
            [
                Some("v1.0 MVP"),
                Some("v1.0 MVP"),
                Some("v1.1 Xuất báo cáo")
            ]
        );
    }

    #[test]
    fn milestones_single_is_hidden() {
        let mut f = milestone_facts();
        f.phases[2].milestone = Some("v1.0 MVP".to_string());
        let v = derive(&f, &all());
        assert!(v.milestones.is_empty());
        let none = derive(&tracer_facts(), &all());
        assert!(none.milestones.is_empty());
        assert_eq!(none.milestone, None);
    }

    #[test]
    fn milestones_unknown_warns() {
        let sel = Filter {
            epic: EpicSel::All,
            milestone: Some("v9".to_string()),
        };
        let v = derive(&milestone_facts(), &sel);
        assert_eq!(v.milestone, None);
        assert_eq!(v.items.len(), 3);
        assert!(warning_at(
            &v,
            ".planning/ROADMAP.md",
            "Không có milestone v9 — đang xem mọi milestone"
        ));
    }

    #[test]
    fn items_decimal_phase_order() {
        assert!(PhaseNum::parse("05.9") < PhaseNum::parse("05.10"));
        assert_eq!(PhaseNum::parse("5.4"), PhaseNum::parse("05.4"));
        assert_eq!(pn("05.10").display(), "05.10");
        assert_eq!(pn("1").display(), "01");
        assert_eq!(PhaseNum::parse("1234567"), None);
        assert_eq!(PhaseNum::parse("5..4"), None);
        let f = Facts {
            phases: vec![ph("5.10", "Mười"), ph("5.9", "Chín")],
            plans: vec![plan("5.10", 1, &[], false), plan("5.9", 1, &[], false)],
            ..Facts::default()
        };
        let v = derive(&f, &all());
        let keys: Vec<&str> = v.items.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(keys, ["05.9-01", "05.10-01"]);
        let epics: Vec<&str> = v.epics.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(epics, ["05.9", "05.10"]);
    }
}
