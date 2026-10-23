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

pub const MAX_ITEMS: usize = 2_500; // áp dụng ở plan 05.5-03
pub const MAX_EDGES: usize = 20_000; // áp dụng ở plan 05.5-03
pub const MAX_PHASE_LINK_PAIRS: usize = 256; // áp dụng ở plan 05.5-03
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
            done: ph.checkbox == Some(true),
            plan_path: None,
            depends: Vec::new(),
        });
    }
    nodes.sort_by(|a, b| (&a.epic, a.number, &a.key).cmp(&(&b.epic, b.number, &b.key)));
    // Khoá trùng: giữ hạng mục đầu theo thứ tự (cảnh báo trùng khoá thêm ở plan 05.5-03).
    let mut seen_keys: HashSet<String> = HashSet::new();
    nodes.retain(|n| seen_keys.insert(n.key.clone()));
    let n = nodes.len();

    let index: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, nd)| (nd.key.as_str(), i))
        .collect();

    // Cạnh tường minh: deps[i] = các hạng mục mà i phải chờ.
    let mut deps: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, nd) in nodes.iter().enumerate() {
        for raw in &nd.depends {
            let name = raw.trim().trim_matches(|c| c == '"' || c == '\'').trim();
            if let Some(&d) = index.get(name) {
                if d != i && !deps[i].contains(&d) {
                    deps[i].push(d);
                }
            }
        }
        deps[i].sort_unstable();
    }

    // Thứ tự topo bằng DFS tuần tự; cạnh quay về nút đang trên stack bị bỏ (cảnh báo ở 05.5-03).
    let mut state = vec![0u8; n]; // 0 mới, 1 đang trên stack, 2 xong
    let mut topo: Vec<usize> = Vec::with_capacity(n);
    for root in 0..n {
        if state[root] != 0 {
            continue;
        }
        state[root] = 1;
        let mut stack: Vec<(usize, usize)> = vec![(root, 0)];
        while let Some(&(v, i)) = stack.last() {
            if i < deps[v].len() {
                let d = deps[v][i];
                match state[d] {
                    1 => {
                        deps[v].remove(i);
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
    let visible: Vec<bool> = nodes
        .iter()
        .map(|nd| match &selected {
            Some(p) => &nd.epic == p,
            None => true,
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
                waits_on: Vec::new(),
                waited_by: Vec::new(),
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
            kind: "plan",
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

    let mut warnings = extra_warnings;
    warnings.extend(facts.warnings.iter().cloned());
    warnings.truncate(MAX_WARNINGS);

    View {
        epic: epic_str,
        default_epic,
        epics,
        milestone: None,
        milestones: Vec::new(),
        items,
        edges,
        external: Vec::new(),
        counts,
        warnings,
        truncated: facts.truncated,
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
}
