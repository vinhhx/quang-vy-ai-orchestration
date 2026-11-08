use crate::error::{BrainError, BrainResult};
use crate::projects::Project;
use crate::stream::Usage;
use rusqlite::{params, Connection};
use std::path::Path;
use std::time::Duration;

pub const MIGRATIONS: &[&str] = &[
    r#"CREATE TABLE projects (id INTEGER PRIMARY KEY, slug TEXT NOT NULL UNIQUE, path TEXT NOT NULL,
  name_vi TEXT NOT NULL DEFAULT '', aliases_json TEXT NOT NULL DEFAULT '[]', type TEXT NOT NULL DEFAULT 'code',
  profile TEXT NOT NULL DEFAULT '', repo TEXT, missing INTEGER NOT NULL DEFAULT 0);
CREATE TABLE tasks (id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL REFERENCES projects(id), slug TEXT NOT NULL,
  title TEXT NOT NULL, branch TEXT NOT NULL, base_branch TEXT, base_commit TEXT, status TEXT NOT NULL DEFAULT 'open',
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, UNIQUE(project_id, branch));
CREATE TABLE runs (id INTEGER PRIMARY KEY, task_id INTEGER REFERENCES tasks(id), project_id INTEGER NOT NULL REFERENCES projects(id),
  role TEXT NOT NULL CHECK (role IN ('lead','dev','qa','inspect')), engine TEXT NOT NULL DEFAULT 'claude', profile TEXT NOT NULL,
  model TEXT, session_id TEXT, pid INTEGER, pgid INTEGER, pid_start INTEGER,
  status TEXT NOT NULL CHECK (status IN ('running','succeeded','failed','timed_out','killed','lost')),
  outcome TEXT, exit_code INTEGER, kill_requested_at INTEGER, started_at INTEGER NOT NULL, ended_at INTEGER,
  input_tokens INTEGER NOT NULL DEFAULT 0, output_tokens INTEGER NOT NULL DEFAULT 0,
  cache_read_tokens INTEGER NOT NULL DEFAULT 0, cache_write_tokens INTEGER NOT NULL DEFAULT 0,
  cost_usd REAL NOT NULL DEFAULT 0, num_turns INTEGER, assistant_turns INTEGER, turns_before_first_edit INTEGER,
  reads_outside_plan INTEGER, permission_denials INTEGER NOT NULL DEFAULT 0,
  usage_source TEXT NOT NULL DEFAULT 'none' CHECK (usage_source IN ('result','partial','none')),
  base_commit TEXT, run_dir TEXT, error TEXT);
CREATE INDEX runs_started ON runs(started_at);
CREATE INDEX runs_task ON runs(task_id);
CREATE INDEX runs_running ON runs(status) WHERE status = 'running';
CREATE TABLE events (id INTEGER PRIMARY KEY, run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
  seq INTEGER NOT NULL, ts INTEGER NOT NULL, kind TEXT NOT NULL, raw_json TEXT NOT NULL, UNIQUE(run_id, seq));"#,
    r#"CREATE TABLE runs_new (id INTEGER PRIMARY KEY, task_id INTEGER REFERENCES tasks(id), project_id INTEGER NOT NULL REFERENCES projects(id),
  role TEXT NOT NULL CHECK (role IN ('lead','dev','qa','inspect','plan-review','code-review')), engine TEXT NOT NULL DEFAULT 'claude', profile TEXT NOT NULL,
  model TEXT, session_id TEXT, pid INTEGER, pgid INTEGER, pid_start INTEGER,
  status TEXT NOT NULL CHECK (status IN ('running','succeeded','failed','timed_out','killed','lost')),
  outcome TEXT, exit_code INTEGER, kill_requested_at INTEGER, started_at INTEGER NOT NULL, ended_at INTEGER,
  input_tokens INTEGER NOT NULL DEFAULT 0, output_tokens INTEGER NOT NULL DEFAULT 0,
  cache_read_tokens INTEGER NOT NULL DEFAULT 0, cache_write_tokens INTEGER NOT NULL DEFAULT 0,
  cost_usd REAL NOT NULL DEFAULT 0, num_turns INTEGER, assistant_turns INTEGER, turns_before_first_edit INTEGER,
  reads_outside_plan INTEGER, permission_denials INTEGER NOT NULL DEFAULT 0,
  usage_source TEXT NOT NULL DEFAULT 'none' CHECK (usage_source IN ('result','partial','none')),
  base_commit TEXT, run_dir TEXT, error TEXT, stage TEXT, round INTEGER, metrics_json TEXT);
INSERT INTO runs_new (id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start, status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit, reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error)
  SELECT id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start, status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit, reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error FROM runs;
DROP TABLE runs;
ALTER TABLE runs_new RENAME TO runs;
CREATE INDEX runs_started ON runs(started_at);
CREATE INDEX runs_task ON runs(task_id);
CREATE INDEX runs_running ON runs(status) WHERE status = 'running';
ALTER TABLE tasks ADD COLUMN workflow_json TEXT;
ALTER TABLE tasks ADD COLUMN approved_at INTEGER;
ALTER TABLE tasks ADD COLUMN approved_commit TEXT;
ALTER TABLE tasks ADD COLUMN auto_state TEXT;
ALTER TABLE tasks ADD COLUMN auto_pid INTEGER;
ALTER TABLE tasks ADD COLUMN auto_pid_start INTEGER;
ALTER TABLE tasks ADD COLUMN auto_stop_at INTEGER;
ALTER TABLE tasks ADD COLUMN auto_stop_kind TEXT;
ALTER TABLE tasks ADD COLUMN auto_json TEXT;"#,
    "ALTER TABLE tasks ADD COLUMN origin TEXT;",
    "ALTER TABLE tasks ADD COLUMN team_json TEXT;",
    r#"CREATE TABLE intake (id INTEGER PRIMARY KEY, source TEXT NOT NULL CHECK (source IN ('ui','telegram')), project_id INTEGER REFERENCES projects(id), request TEXT NOT NULL, priority TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('urgent','normal','low')), status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued','triaging','proposed','cancelled','failed')), error TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
CREATE INDEX intake_status ON intake(status);
CREATE TABLE proposals (id INTEGER PRIMARY KEY, intake_id INTEGER NOT NULL REFERENCES intake(id) ON DELETE CASCADE, project_id INTEGER NOT NULL REFERENCES projects(id), title TEXT NOT NULL, request TEXT NOT NULL, priority TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('urgent','normal','low')), team TEXT NOT NULL, workflow TEXT NOT NULL, reason TEXT, status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected','started','failed')), task_id INTEGER REFERENCES tasks(id), error TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
CREATE INDEX proposals_status ON proposals(status);
CREATE TABLE quota_stops (task_id INTEGER PRIMARY KEY REFERENCES tasks(id), stopped_at INTEGER NOT NULL, engine TEXT NOT NULL, notified INTEGER NOT NULL DEFAULT 0);"#,
    r#"CREATE TABLE intake_new (
  id INTEGER PRIMARY KEY,
  source TEXT NOT NULL CHECK (source IN ('ui','telegram','trello','sheet')),
  project_id INTEGER REFERENCES projects(id),
  request TEXT NOT NULL,
  priority TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('urgent','normal','low')),
  status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued','triaging','proposed','cancelled','failed')),
  error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
INSERT INTO intake_new (id, source, project_id, request, priority, status, error, created_at, updated_at)
  SELECT id, source, project_id, request, priority, status, error, created_at, updated_at FROM intake;
DROP TABLE intake;
ALTER TABLE intake_new RENAME TO intake;
CREATE INDEX intake_status ON intake(status);
CREATE TABLE fixbug_refs (
  id INTEGER PRIMARY KEY,
  project_id INTEGER NOT NULL REFERENCES projects(id),
  kind TEXT NOT NULL CHECK (kind IN ('trello','sheet')),
  ext_id TEXT NOT NULL,
  key TEXT NOT NULL,
  title TEXT NOT NULL,
  url TEXT,
  intake_id INTEGER REFERENCES intake(id),
  state TEXT NOT NULL DEFAULT 'pulled' CHECK (state IN ('pulled','deployed','rejected')),
  note TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE (project_id, kind, ext_id)
);"#,
    r#"CREATE TABLE runs_new (
  id INTEGER PRIMARY KEY,
  task_id INTEGER REFERENCES tasks(id),
  project_id INTEGER NOT NULL REFERENCES projects(id),
  role TEXT NOT NULL CHECK (role IN ('lead','dev','qa','inspect','plan-review','code-review','analyst','analysis-review')),
  engine TEXT NOT NULL DEFAULT 'claude',
  profile TEXT NOT NULL,
  model TEXT,
  session_id TEXT,
  pid INTEGER,
  pgid INTEGER,
  pid_start INTEGER,
  status TEXT NOT NULL CHECK (status IN ('running','succeeded','failed','timed_out','killed','lost')),
  outcome TEXT,
  exit_code INTEGER,
  kill_requested_at INTEGER,
  started_at INTEGER NOT NULL,
  ended_at INTEGER,
  input_tokens INTEGER NOT NULL DEFAULT 0,
  output_tokens INTEGER NOT NULL DEFAULT 0,
  cache_read_tokens INTEGER NOT NULL DEFAULT 0,
  cache_write_tokens INTEGER NOT NULL DEFAULT 0,
  cost_usd REAL NOT NULL DEFAULT 0,
  num_turns INTEGER,
  assistant_turns INTEGER,
  turns_before_first_edit INTEGER,
  reads_outside_plan INTEGER,
  permission_denials INTEGER NOT NULL DEFAULT 0,
  usage_source TEXT NOT NULL DEFAULT 'none' CHECK (usage_source IN ('result','partial','none')),
  base_commit TEXT,
  run_dir TEXT,
  error TEXT,
  stage TEXT,
  round INTEGER,
  metrics_json TEXT
);
INSERT INTO runs_new (
  id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start,
  status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens,
  cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit,
  reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error, stage, round, metrics_json
)
  SELECT
    id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start,
    status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens,
    cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit,
    reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error, stage, round, metrics_json
  FROM runs;
DROP TABLE runs;
ALTER TABLE runs_new RENAME TO runs;
CREATE INDEX runs_started ON runs(started_at);
CREATE INDEX runs_task ON runs(task_id);
CREATE INDEX runs_running ON runs(status) WHERE status = 'running';"#,
    r#"ALTER TABLE tasks ADD COLUMN level TEXT NOT NULL DEFAULT 'vua' CHECK (level IN ('de','vua','kho'));
ALTER TABLE proposals ADD COLUMN level TEXT NOT NULL DEFAULT 'vua' CHECK (level IN ('de','vua','kho'));
ALTER TABLE runs ADD COLUMN member_id TEXT;
CREATE TABLE member_scores (
  id INTEGER PRIMARY KEY,
  run_id INTEGER NOT NULL UNIQUE REFERENCES runs(id),
  member_id TEXT NOT NULL,
  task_id INTEGER REFERENCES tasks(id),
  score INTEGER NOT NULL CHECK (score BETWEEN 1 AND 5),
  note TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX member_scores_member ON member_scores(member_id);"#,
    r#"CREATE TABLE task_inputs (
  id INTEGER PRIMARY KEY,
  task_id INTEGER REFERENCES tasks(id) ON DELETE CASCADE,
  proposal_id INTEGER REFERENCES proposals(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  source TEXT NOT NULL,
  content TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  CHECK (task_id IS NOT NULL OR proposal_id IS NOT NULL)
);
CREATE INDEX task_inputs_task ON task_inputs(task_id);
CREATE INDEX task_inputs_proposal ON task_inputs(proposal_id);
CREATE TABLE intake_inputs (
  id INTEGER PRIMARY KEY,
  intake_id INTEGER NOT NULL REFERENCES intake(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  content TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
ALTER TABLE proposals ADD COLUMN after_proposal_id INTEGER REFERENCES proposals(id);"#,
    r#"CREATE TABLE plan_approvals (
  id INTEGER PRIMARY KEY,
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  "commit" TEXT NOT NULL,
  approved_at INTEGER NOT NULL,
  source TEXT NOT NULL
);
CREATE INDEX plan_approvals_task ON plan_approvals(task_id);
ALTER TABLE intake ADD COLUMN split INTEGER NOT NULL DEFAULT 1;"#,
];

pub const SCHEMA_VERSION: i64 = 10;

impl From<rusqlite::Error> for BrainError {
    fn from(e: rusqlite::Error) -> Self {
        BrainError::new(format!("Lỗi CSDL: {e}"), crate::error::EXIT_ERROR)
    }
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn open(path: &Path) -> BrainResult<Connection> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(parent)?;
            }
            #[cfg(not(unix))]
            {
                std::fs::create_dir_all(parent)?;
            }
        } else {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
        }
    }

    if !path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)?;
        }
        #[cfg(not(unix))]
        {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
        }
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
    }

    let mut conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_millis(5000))?;

    let current_version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current_version > MIGRATIONS.len() as i64 {
        return Err(BrainError::error(format!(
            "CSDL {} có phiên bản {current_version}, mới hơn bản agent-brain này (hỗ trợ tới {}) — hãy cài agent-brain mới hơn.",
            path.display(),
            MIGRATIONS.len()
        )));
    }

    let start = std::time::Instant::now();
    loop {
        match conn.pragma_update(None, "journal_mode", "WAL") {
            Ok(()) => break,
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.extended_code == rusqlite::ffi::SQLITE_BUSY
                    && start.elapsed() < Duration::from_millis(5000) =>
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e.into()),
        }
    }
    conn.pragma_update(None, "synchronous", "NORMAL")?;

    if (current_version as usize) < MIGRATIONS.len() {
        conn.pragma_update(None, "foreign_keys", "OFF")?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let ver_inside: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(ver_inside as usize) {
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        }
        let bad: i64 = tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })?;
        if bad > 0 {
            return Err(BrainError::error(
                "Nâng cấp CSDL thất bại: vi phạm khóa ngoại sau khi dựng lại bảng runs",
            ));
        }
        tx.commit()?;
    }

    conn.pragma_update(None, "foreign_keys", "ON")?;

    Ok(conn)
}

pub fn sync_projects(conn: &Connection, projects: &[Project]) -> BrainResult<()> {
    for p in projects {
        let aliases_json = serde_json::to_string(&p.aliases).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT INTO projects (slug, path, name_vi, aliases_json, type, profile, repo, missing)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(slug) DO UPDATE SET
               path = excluded.path,
               name_vi = excluded.name_vi,
               aliases_json = excluded.aliases_json,
               type = excluded.type,
               profile = excluded.profile,
               repo = excluded.repo,
               missing = excluded.missing",
            params![
                &p.id,
                &p.path,
                &p.name_vi,
                &aliases_json,
                &p.kind,
                &p.profile,
                &p.repo,
                if p.missing { 1 } else { 0 },
            ],
        )?;
    }
    Ok(())
}

pub fn project_row_id(conn: &Connection, slug: &str) -> BrainResult<i64> {
    let id: i64 = conn.query_row(
        "SELECT id FROM projects WHERE slug = ?1",
        params![slug],
        |r| r.get(0),
    )?;
    Ok(id)
}

/// Dọn hàng task mồ côi nếu chưa có run nào được ghi nhận.
pub fn cleanup_orphan_task(conn: &Connection, task_id: i64) -> BrainResult<bool> {
    let has_run: bool = conn
        .query_row("SELECT 1 FROM runs WHERE task_id = ?1", [task_id], |_| Ok(()))
        .is_ok();
    if !has_run {
        let _ = conn.execute("DELETE FROM task_inputs WHERE task_id = ?1", [task_id]);
        conn.execute("DELETE FROM tasks WHERE id = ?1", [task_id])?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[derive(Debug, Clone)]
pub struct TaskRow {
    pub id: i64,
    pub project_id: i64,
    pub slug: String,
    pub title: String,
    pub branch: String,
    pub base_branch: Option<String>,
    pub base_commit: Option<String>,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub level: String,
}

pub const TASK_COLUMNS: &str =
    "id, project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at, level";

fn task_from_row(r: &rusqlite::Row) -> rusqlite::Result<TaskRow> {
    Ok(TaskRow {
        id: r.get(0)?,
        project_id: r.get(1)?,
        slug: r.get(2)?,
        title: r.get(3)?,
        branch: r.get(4)?,
        base_branch: r.get(5)?,
        base_commit: r.get(6)?,
        status: r.get(7)?,
        created_at: r.get(8)?,
        updated_at: r.get(9)?,
        level: r.get(10)?,
    })
}

pub fn upsert_task(
    conn: &Connection,
    project_id: i64,
    slug: &str,
    title: &str,
    branch: &str,
    base_branch: Option<&str>,
    base_commit: Option<&str>,
) -> BrainResult<i64> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO tasks (project_id, slug, title, branch, base_branch, base_commit, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', ?7, ?7)
         ON CONFLICT(project_id, branch) DO UPDATE SET
           title = excluded.title,
           base_branch = CASE WHEN tasks.status = 'open' THEN excluded.base_branch ELSE tasks.base_branch END,
           base_commit = CASE WHEN tasks.status = 'open' THEN excluded.base_commit ELSE tasks.base_commit END,
           status = CASE WHEN tasks.status = 'open' THEN 'open' ELSE tasks.status END,
           updated_at = excluded.updated_at",
        params![project_id, slug, title, branch, base_branch, base_commit, now],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM tasks WHERE project_id = ?1 AND branch = ?2",
        params![project_id, branch],
        |r| r.get(0),
    )?;
    Ok(id)
}

pub fn get_task(conn: &Connection, id: i64) -> BrainResult<Option<TaskRow>> {
    let sql = format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(r) = rows.next()? {
        Ok(Some(task_from_row(r)?))
    } else {
        Ok(None)
    }
}

pub fn find_task_by_branch(
    conn: &Connection,
    project_id: i64,
    branch: &str,
) -> BrainResult<Option<TaskRow>> {
    let sql = format!("SELECT {TASK_COLUMNS} FROM tasks WHERE project_id = ?1 AND branch = ?2");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![project_id, branch])?;
    if let Some(r) = rows.next()? {
        Ok(Some(task_from_row(r)?))
    } else {
        Ok(None)
    }
}

pub fn set_task_status(conn: &Connection, id: i64, status: &str) -> BrainResult<()> {
    let now = now_ms();
    conn.execute(
        "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![status, now, id],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct NewRun {
    pub task_id: Option<i64>,
    pub project_id: i64,
    pub role: String,
    pub profile: String,
    pub model: Option<String>,
    pub pid: i64,
    pub pgid: i64,
    pub pid_start: i64,
    pub base_commit: Option<String>,
    pub run_dir: Option<String>,
}

pub fn insert_run(conn: &Connection, r: &NewRun) -> BrainResult<i64> {
    insert_run_with_engine(conn, r, "claude")
}

/// Inserts a `running` row recording the engine that executes it (D-14).
pub fn insert_run_with_engine(conn: &Connection, r: &NewRun, engine: &str) -> BrainResult<i64> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO runs (task_id, project_id, role, engine, profile, model, pid, pgid, pid_start,
                           status, started_at, base_commit, run_dir)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'running', ?10, ?11, ?12)",
        params![
            r.task_id,
            r.project_id,
            r.role,
            engine,
            r.profile,
            r.model,
            r.pid,
            r.pgid,
            r.pid_start,
            now,
            r.base_commit,
            r.run_dir,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[derive(Debug, Clone)]
pub struct HumanRun {
    pub task_id: i64,
    pub project_id: i64,
    pub role: String,
    pub profile: String,
    pub stage: Option<String>,
    pub round: Option<i64>,
    pub started_at: i64,
    pub ended_at: i64,
    pub outcome: Option<String>,
    pub base_commit: Option<String>,
    pub metrics_json: String,
}

/// Stores a person's finished work as a normal finished run (D-21): engine `người`, status
/// `succeeded`, no tokens, no model, no process identity. Never inserts a `running` row, so
/// `reconcile_stale_runs` (running rows without a live pid become `lost`) never touches it.
pub fn insert_human_run(conn: &Connection, h: &HumanRun) -> BrainResult<i64> {
    conn.execute(
        "INSERT INTO runs (task_id, project_id, role, engine, profile, model, status, outcome,
                           started_at, ended_at, usage_source, base_commit, stage, round, metrics_json)
         VALUES (?1, ?2, ?3, 'người', ?4, NULL, 'succeeded', ?5, ?6, ?7, 'none', ?8, ?9, ?10, ?11)",
        params![
            h.task_id,
            h.project_id,
            h.role,
            h.profile,
            h.outcome,
            h.started_at,
            h.ended_at,
            h.base_commit,
            h.stage,
            h.round,
            h.metrics_json,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_event(
    conn: &Connection,
    run_id: i64,
    seq: i64,
    ts_ms: i64,
    kind: &str,
    raw: &str,
) -> BrainResult<()> {
    conn.execute(
        "INSERT INTO events (run_id, seq, ts, kind, raw_json) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![run_id, seq, ts_ms, kind, raw],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct RunFinish {
    pub status: String,
    pub exit_code: Option<i32>,
    pub session_id: Option<String>,
    pub usage: Usage,
    pub usage_source: String,
    pub cost_usd: f64,
    pub num_turns: Option<i64>,
    pub assistant_turns: Option<i64>,
    pub turns_before_first_edit: Option<i64>,
    pub reads_outside_plan: Option<i64>,
    pub permission_denials: i64,
    pub error: Option<String>,
}

pub fn finish_run(conn: &Connection, run_id: i64, f: &RunFinish) -> BrainResult<()> {
    let now = now_ms();
    conn.execute(
        "UPDATE runs SET
           status = ?1,
           exit_code = ?2,
           session_id = ?3,
           input_tokens = ?4,
           output_tokens = ?5,
           cache_read_tokens = ?6,
           cache_write_tokens = ?7,
           usage_source = ?8,
           cost_usd = ?9,
           num_turns = ?10,
           assistant_turns = ?11,
           turns_before_first_edit = ?12,
           reads_outside_plan = ?13,
           permission_denials = ?14,
           error = ?15,
           ended_at = ?16
         WHERE id = ?17",
        params![
            f.status,
            f.exit_code,
            f.session_id,
            f.usage.input_tokens,
            f.usage.output_tokens,
            f.usage.cache_read_input_tokens,
            f.usage.cache_creation_input_tokens,
            f.usage_source,
            f.cost_usd,
            f.num_turns,
            f.assistant_turns,
            f.turns_before_first_edit,
            f.reads_outside_plan,
            f.permission_denials,
            f.error,
            now,
            run_id,
        ],
    )?;
    Ok(())
}

pub fn set_run_outcome(conn: &Connection, run_id: i64, outcome: &str) -> BrainResult<()> {
    conn.execute(
        "UPDATE runs SET outcome = ?1 WHERE id = ?2",
        params![outcome, run_id],
    )?;
    Ok(())
}

pub fn set_run_error(conn: &Connection, run_id: i64, error: &str) -> BrainResult<()> {
    let truncated = if error.len() > 4000 {
        let mut end = 4000;
        while end > 0 && !error.is_char_boundary(end) {
            end -= 1;
        }
        &error[..end]
    } else {
        error
    };
    conn.execute(
        "UPDATE runs SET error = ?1 WHERE id = ?2",
        params![truncated, run_id],
    )?;
    Ok(())
}

pub fn mark_kill_requested(conn: &Connection, run_id: i64) -> BrainResult<()> {
    let now = now_ms();
    conn.execute(
        "UPDATE runs SET kill_requested_at = ?1 WHERE id = ?2",
        params![now, run_id],
    )?;
    Ok(())
}

pub fn mark_status(conn: &Connection, run_id: i64, status: &str) -> BrainResult<()> {
    if status == "running" {
        conn.execute(
            "UPDATE runs SET status = ?1 WHERE id = ?2",
            params![status, run_id],
        )?;
    } else {
        let now = now_ms();
        conn.execute(
            "UPDATE runs SET status = ?1, ended_at = COALESCE(ended_at, ?2) WHERE id = ?3",
            params![status, now, run_id],
        )?;
    }
    Ok(())
}

/// Đánh dấu run `lost` chỉ khi nó vẫn `running` — không ghi đè `killed`/`done`
/// mà tiến trình con vừa ghi (Kill song song với driver autopilot).
pub fn mark_lost_if_running(conn: &Connection, run_id: i64) -> BrainResult<bool> {
    let now = now_ms();
    let changed = conn.execute(
        "UPDATE runs SET status = 'lost', ended_at = COALESCE(ended_at, ?1) WHERE id = ?2 AND status = 'running'",
        params![now, run_id],
    )?;
    Ok(changed > 0)
}

#[derive(Debug, Clone)]
pub struct RunRow {
    pub id: i64,
    pub task_id: Option<i64>,
    pub project_id: i64,
    pub role: String,
    pub engine: String,
    pub profile: String,
    pub model: Option<String>,
    pub session_id: Option<String>,
    pub pid: Option<i64>,
    pub pgid: Option<i64>,
    pub pid_start: Option<i64>,
    pub status: String,
    pub outcome: Option<String>,
    pub exit_code: Option<i32>,
    pub kill_requested_at: Option<i64>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub cost_usd: f64,
    pub num_turns: Option<i64>,
    pub assistant_turns: Option<i64>,
    pub turns_before_first_edit: Option<i64>,
    pub reads_outside_plan: Option<i64>,
    pub permission_denials: i64,
    pub usage_source: String,
    pub base_commit: Option<String>,
    pub run_dir: Option<String>,
    pub error: Option<String>,
    pub stage: Option<String>,
    pub round: Option<i64>,
    pub metrics_json: Option<String>,
    pub member_id: Option<String>,
}

fn row_to_run(r: &rusqlite::Row) -> rusqlite::Result<RunRow> {
    Ok(RunRow {
        id: r.get(0)?,
        task_id: r.get(1)?,
        project_id: r.get(2)?,
        role: r.get(3)?,
        engine: r.get(4)?,
        profile: r.get(5)?,
        model: r.get(6)?,
        session_id: r.get(7)?,
        pid: r.get(8)?,
        pgid: r.get(9)?,
        pid_start: r.get(10)?,
        status: r.get(11)?,
        outcome: r.get(12)?,
        exit_code: r.get(13)?,
        kill_requested_at: r.get(14)?,
        started_at: r.get(15)?,
        ended_at: r.get(16)?,
        input_tokens: r.get(17)?,
        output_tokens: r.get(18)?,
        cache_read_tokens: r.get(19)?,
        cache_write_tokens: r.get(20)?,
        cost_usd: r.get(21)?,
        num_turns: r.get(22)?,
        assistant_turns: r.get(23)?,
        turns_before_first_edit: r.get(24)?,
        reads_outside_plan: r.get(25)?,
        permission_denials: r.get(26)?,
        usage_source: r.get(27)?,
        base_commit: r.get(28)?,
        run_dir: r.get(29)?,
        error: r.get(30)?,
        stage: r.get(31)?,
        round: r.get(32)?,
        metrics_json: r.get(33)?,
        member_id: r.get(34)?,
    })
}

const RUN_COLUMNS: &str = "id, task_id, project_id, role, engine, profile, model, session_id, pid, pgid, pid_start,
  status, outcome, exit_code, kill_requested_at, started_at, ended_at, input_tokens, output_tokens,
  cache_read_tokens, cache_write_tokens, cost_usd, num_turns, assistant_turns, turns_before_first_edit,
  reads_outside_plan, permission_denials, usage_source, base_commit, run_dir, error, stage, round, metrics_json, member_id";

pub fn get_run(conn: &Connection, id: i64) -> BrainResult<Option<RunRow>> {
    let sql = format!("SELECT {RUN_COLUMNS} FROM runs WHERE id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(r) = rows.next()? {
        Ok(Some(row_to_run(r)?))
    } else {
        Ok(None)
    }
}

pub fn running_runs(conn: &Connection) -> BrainResult<Vec<RunRow>> {
    let sql = format!("SELECT {RUN_COLUMNS} FROM runs WHERE status = 'running'");
    let mut stmt = conn.prepare(&sql)?;
    let runs = stmt
        .query_map([], row_to_run)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(runs)
}

pub fn running_run_for_task(conn: &Connection, task_id: i64) -> BrainResult<Option<RunRow>> {
    let sql = format!("SELECT {RUN_COLUMNS} FROM runs WHERE task_id = ?1 AND status = 'running'");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![task_id])?;
    if let Some(r) = rows.next()? {
        Ok(Some(row_to_run(r)?))
    } else {
        Ok(None)
    }
}

pub fn latest_run_for_task(conn: &Connection, task_id: i64) -> BrainResult<Option<RunRow>> {
    let sql = format!(
        "SELECT {RUN_COLUMNS} FROM runs WHERE task_id = ?1 ORDER BY started_at DESC, id DESC LIMIT 1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![task_id])?;
    if let Some(r) = rows.next()? {
        Ok(Some(row_to_run(r)?))
    } else {
        Ok(None)
    }
}

pub fn runs_for_task(conn: &Connection, task_id: i64) -> BrainResult<Vec<RunRow>> {
    let sql = format!(
        "SELECT {RUN_COLUMNS} FROM runs WHERE task_id = ?1 ORDER BY started_at ASC, id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let runs = stmt
        .query_map(params![task_id], row_to_run)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(runs)
}

pub fn count_runs(conn: &Connection) -> BrainResult<i64> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))?;
    Ok(count)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskAuto {
    pub workflow_json: Option<String>,
    pub approved_at: Option<i64>,
    pub approved_commit: Option<String>,
    pub auto_state: Option<String>,
    pub auto_pid: Option<i64>,
    pub auto_pid_start: Option<i64>,
    pub auto_stop_at: Option<i64>,
    pub auto_stop_kind: Option<String>,
    pub auto_json: Option<String>,
}

pub fn task_auto(conn: &Connection, task_id: i64) -> BrainResult<Option<TaskAuto>> {
    let mut stmt = conn.prepare(
        "SELECT workflow_json, approved_at, approved_commit, auto_state, auto_pid, auto_pid_start, auto_stop_at, auto_stop_kind, auto_json
         FROM tasks WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![task_id])?;
    if let Some(r) = rows.next()? {
        Ok(Some(TaskAuto {
            workflow_json: r.get(0)?,
            approved_at: r.get(1)?,
            approved_commit: r.get(2)?,
            auto_state: r.get(3)?,
            auto_pid: r.get(4)?,
            auto_pid_start: r.get(5)?,
            auto_stop_at: r.get(6)?,
            auto_stop_kind: r.get(7)?,
            auto_json: r.get(8)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn set_task_workflow_if_null(conn: &Connection, task_id: i64, json: &str) -> BrainResult<bool> {
    let changes = conn.execute(
        "UPDATE tasks SET workflow_json = ?1 WHERE id = ?2 AND workflow_json IS NULL",
        params![json, task_id],
    )?;
    Ok(changes == 1)
}

pub const ORIGIN_TELEGRAM: &str = "telegram";

/// Origin of a task (`None` for a missing task or a NULL origin).
pub fn task_origin(conn: &Connection, task_id: i64) -> BrainResult<Option<String>> {
    let mut stmt = conn.prepare("SELECT origin FROM tasks WHERE id = ?1")?;
    let mut rows = stmt.query(params![task_id])?;
    match rows.next()? {
        Some(r) => Ok(r.get(0)?),
        None => Ok(None),
    }
}

/// Sets the origin only while it is still NULL: origin is monotonic and never cleared.
pub fn mark_task_origin(conn: &Connection, task_id: i64, origin: &str) -> BrainResult<bool> {
    let changes = conn.execute(
        "UPDATE tasks SET origin = ?2 WHERE id = ?1 AND origin IS NULL",
        params![task_id, origin],
    )?;
    Ok(changes == 1)
}

/// The task's team JSON (`None` for a missing task or a NULL column).
pub fn task_team_json(conn: &Connection, task_id: i64) -> BrainResult<Option<String>> {
    let mut stmt = conn.prepare("SELECT team_json FROM tasks WHERE id = ?1")?;
    let mut rows = stmt.query(params![task_id])?;
    match rows.next()? {
        Some(r) => Ok(r.get(0)?),
        None => Ok(None),
    }
}

/// Creates the team only while `team_json` is still NULL. Bumps `updated_at` on a write so the
/// Telegram detector signature and the dashboard `rev` change.
pub fn set_task_team_if_null(conn: &Connection, task_id: i64, json: &str) -> BrainResult<bool> {
    let now = now_ms();
    let changes = conn.execute(
        "UPDATE tasks SET team_json = ?1, updated_at = ?2 WHERE id = ?3 AND team_json IS NULL",
        params![json, now, task_id],
    )?;
    Ok(changes == 1)
}

/// Compare-and-swap on `team_json`: writes only when the stored value equals `old`
/// (`None` matches NULL, via the NULL-safe `IS`). Bumps `updated_at` on a write.
pub fn set_task_team_cas(
    conn: &Connection,
    task_id: i64,
    old: Option<&str>,
    new: &str,
) -> BrainResult<bool> {
    let now = now_ms();
    let changes = conn.execute(
        "UPDATE tasks SET team_json = ?1, updated_at = ?2 WHERE id = ?3 AND team_json IS ?4",
        params![new, now, task_id, old],
    )?;
    Ok(changes == 1)
}

pub fn count_tasks_since(conn: &Connection, origin: &str, since_ms: i64) -> BrainResult<i64> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE origin = ?1 AND created_at >= ?2",
        params![origin, since_ms],
        |r| r.get(0),
    )?;
    Ok(n)
}

pub fn set_task_approval(
    conn: &Connection,
    task_id: i64,
    at_ms: i64,
    commit: &str,
) -> BrainResult<()> {
    conn.execute(
        "UPDATE tasks SET approved_at = ?1, approved_commit = ?2 WHERE id = ?3",
        params![at_ms, commit, task_id],
    )?;
    Ok(())
}

pub fn clear_task_approval(conn: &Connection, task_id: i64) -> BrainResult<()> {
    conn.execute(
        "UPDATE tasks SET approved_at = NULL, approved_commit = NULL WHERE id = ?1",
        params![task_id],
    )?;
    Ok(())
}

pub fn set_auto_state(conn: &Connection, task_id: i64, state: Option<&str>) -> BrainResult<()> {
    let now = now_ms();
    conn.execute(
        "UPDATE tasks SET auto_state = ?1, updated_at = ?2 WHERE id = ?3",
        params![state, now, task_id],
    )?;
    Ok(())
}

pub fn set_auto_driver(
    conn: &Connection,
    task_id: i64,
    pid: Option<i64>,
    pid_start: Option<i64>,
) -> BrainResult<()> {
    conn.execute(
        "UPDATE tasks SET auto_pid = ?1, auto_pid_start = ?2 WHERE id = ?3",
        params![pid, pid_start, task_id],
    )?;
    Ok(())
}

pub fn set_auto_json(conn: &Connection, task_id: i64, json: Option<&str>) -> BrainResult<()> {
    conn.execute(
        "UPDATE tasks SET auto_json = ?1 WHERE id = ?2",
        params![json, task_id],
    )?;
    Ok(())
}

pub fn request_auto_stop(conn: &Connection, task_id: i64, kind: &str) -> BrainResult<()> {
    let now = now_ms();
    conn.execute(
        "UPDATE tasks SET auto_stop_at = ?1, auto_stop_kind = ?2 WHERE id = ?3",
        params![now, kind, task_id],
    )?;
    Ok(())
}

pub fn clear_auto_stop(conn: &Connection, task_id: i64) -> BrainResult<()> {
    conn.execute(
        "UPDATE tasks SET auto_stop_at = NULL, auto_stop_kind = NULL WHERE id = ?1",
        params![task_id],
    )?;
    Ok(())
}

/// Task qua QA bằng một run tay khi autopilot đang dừng (không có driver):
/// tắt autopilot (`ended`), xoá cờ dừng, giữ auto_json làm lịch sử.
/// Một câu lệnh có điều kiện; trả true khi đã ghi.
pub fn end_stopped_autopilot(conn: &Connection, task_id: i64) -> BrainResult<bool> {
    let changed = conn.execute(
        "UPDATE tasks SET auto_state = 'ended', auto_stop_at = NULL, auto_stop_kind = NULL, updated_at = ?1 \
         WHERE id = ?2 AND auto_state = 'stopped' AND auto_pid IS NULL",
        params![now_ms(), task_id],
    )?;
    Ok(changed == 1)
}

pub fn set_run_stage_round(
    conn: &Connection,
    run_id: i64,
    stage: Option<&str>,
    round: Option<i64>,
) -> BrainResult<()> {
    conn.execute(
        "UPDATE runs SET stage = ?1, round = ?2 WHERE id = ?3",
        params![stage, round, run_id],
    )?;
    Ok(())
}

pub fn merge_run_metrics(
    conn: &Connection,
    run_id: i64,
    patch: &serde_json::Value,
) -> BrainResult<()> {
    let patch_obj = match patch.as_object() {
        Some(obj) => obj,
        None => return Err(BrainError::error("metrics patch phải là JSON object")),
    };
    let raw: Option<String> = conn.query_row(
        "SELECT metrics_json FROM runs WHERE id = ?1",
        params![run_id],
        |r| r.get(0),
    )?;
    let mut base = match raw {
        Some(s) => match serde_json::from_str::<serde_json::Value>(&s) {
            Ok(serde_json::Value::Object(map)) => map,
            _ => serde_json::Map::new(),
        },
        None => serde_json::Map::new(),
    };
    for (k, v) in patch_obj {
        base.insert(k.clone(), v.clone());
    }
    let merged_str = serde_json::to_string(&serde_json::Value::Object(base))
        .map_err(|e| BrainError::error(format!("Lỗi serialize metrics JSON: {e}")))?;
    conn.execute(
        "UPDATE runs SET metrics_json = ?1 WHERE id = ?2",
        params![merged_str, run_id],
    )?;
    Ok(())
}

pub fn latest_run_for_task_role(
    conn: &Connection,
    task_id: i64,
    role: &str,
) -> BrainResult<Option<RunRow>> {
    let sql = format!(
        "SELECT {RUN_COLUMNS} FROM runs WHERE task_id = ?1 AND role = ?2 ORDER BY started_at DESC, id DESC LIMIT 1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![task_id, role])?;
    if let Some(r) = rows.next()? {
        Ok(Some(row_to_run(r)?))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DriverRow {
    pub task_id: i64,
    pub project_slug: String,
    pub profile: String,
    pub auto_pid: Option<i64>,
    pub auto_pid_start: Option<i64>,
}

pub fn auto_driver_rows(conn: &Connection) -> BrainResult<Vec<DriverRow>> {
    let mut stmt = conn.prepare(
        "SELECT tasks.id, projects.slug, projects.profile, tasks.auto_pid, tasks.auto_pid_start
         FROM tasks JOIN projects ON tasks.project_id = projects.id
         WHERE tasks.auto_state = 'running'
         ORDER BY tasks.id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(DriverRow {
                task_id: r.get(0)?,
                project_slug: r.get(1)?,
                profile: r.get(2)?,
                auto_pid: r.get(3)?,
                auto_pid_start: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IntakeRow {
    pub id: i64,
    pub source: String,
    pub project_id: Option<i64>,
    pub request: String,
    pub priority: String,
    pub status: String,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub split: bool,
}

const INTAKE_COLUMNS: &str =
    "id, source, project_id, request, priority, status, error, created_at, updated_at, split";

fn row_to_intake(r: &rusqlite::Row) -> rusqlite::Result<IntakeRow> {
    Ok(IntakeRow {
        id: r.get(0)?,
        source: r.get(1)?,
        project_id: r.get(2)?,
        request: r.get(3)?,
        priority: r.get(4)?,
        status: r.get(5)?,
        error: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
        split: r.get::<_, i64>(9)? != 0,
    })
}

pub fn insert_intake(
    conn: &Connection,
    source: &str,
    project_id: Option<i64>,
    request: &str,
    priority: &str,
    split: bool,
) -> BrainResult<i64> {
    let now = now_ms();
    let priority_val = if priority.is_empty() {
        "normal"
    } else {
        priority
    };
    conn.execute(
        "INSERT INTO intake (source, project_id, request, priority, status, error, created_at, updated_at, split)
         VALUES (?1, ?2, ?3, ?4, 'queued', NULL, ?5, ?5, ?6)",
        params![source, project_id, request, priority_val, now, if split { 1 } else { 0 }],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_plan_approval(
    conn: &Connection,
    task_id: i64,
    commit: &str,
    approved_at: i64,
    source: &str,
) -> BrainResult<()> {
    conn.execute(
        "INSERT INTO plan_approvals (task_id, \"commit\", approved_at, source)
         VALUES (?1, ?2, ?3, ?4)",
        params![task_id, commit, approved_at, source],
    )?;
    Ok(())
}

pub fn latest_different_plan_approval(
    conn: &Connection,
    task_id: i64,
    current_commit: &str,
) -> BrainResult<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT \"commit\" FROM plan_approvals
         WHERE task_id = ?1 AND \"commit\" != ?2
         ORDER BY approved_at DESC, id DESC
         LIMIT 1",
    )?;
    let mut rows = stmt.query(params![task_id, current_commit])?;
    if let Some(r) = rows.next()? {
        Ok(Some(r.get(0)?))
    } else {
        Ok(None)
    }
}

pub fn get_intake(conn: &Connection, id: i64) -> BrainResult<Option<IntakeRow>> {
    let sql = format!("SELECT {INTAKE_COLUMNS} FROM intake WHERE id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(r) = rows.next()? {
        Ok(Some(row_to_intake(r)?))
    } else {
        Ok(None)
    }
}

pub fn list_intake(conn: &Connection, status: Option<&str>) -> BrainResult<Vec<IntakeRow>> {
    match status {
        Some(s) => {
            let sql = format!(
                "SELECT {INTAKE_COLUMNS} FROM intake WHERE status = ?1 ORDER BY created_at ASC, id ASC"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![s], row_to_intake)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
        None => {
            let sql = format!(
                "SELECT {INTAKE_COLUMNS} FROM intake ORDER BY created_at ASC, id ASC"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map([], row_to_intake)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
    }
}

pub fn set_intake_status(conn: &Connection, id: i64, status: &str) -> BrainResult<bool> {
    set_intake_status_error(conn, id, status, None)
}

pub fn set_intake_status_error(
    conn: &Connection,
    id: i64,
    status: &str,
    error: Option<&str>,
) -> BrainResult<bool> {
    let now = now_ms();
    let changes = conn.execute(
        "UPDATE intake SET status = ?1, error = ?2, updated_at = ?3 WHERE id = ?4",
        params![status, error, now, id],
    )?;
    Ok(changes > 0)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProposalRow {
    pub id: i64,
    pub intake_id: i64,
    pub project_id: i64,
    pub title: String,
    pub request: String,
    pub priority: String,
    pub team: String,
    pub workflow: String,
    pub reason: Option<String>,
    pub status: String,
    pub task_id: Option<i64>,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub level: String,
    pub after_proposal_id: Option<i64>,
}

const PROPOSAL_COLUMNS: &str =
    "id, intake_id, project_id, title, request, priority, team, workflow, reason, status, task_id, error, created_at, updated_at, level, after_proposal_id";

fn row_to_proposal(r: &rusqlite::Row) -> rusqlite::Result<ProposalRow> {
    Ok(ProposalRow {
        id: r.get(0)?,
        intake_id: r.get(1)?,
        project_id: r.get(2)?,
        title: r.get(3)?,
        request: r.get(4)?,
        priority: r.get(5)?,
        team: r.get(6)?,
        workflow: r.get(7)?,
        reason: r.get(8)?,
        status: r.get(9)?,
        task_id: r.get(10)?,
        error: r.get(11)?,
        created_at: r.get(12)?,
        updated_at: r.get(13)?,
        level: r.get(14)?,
        after_proposal_id: r.get(15)?,
    })
}

#[derive(Debug, Clone)]
pub struct NewProposal<'a> {
    pub intake_id: i64,
    pub project_id: i64,
    pub title: &'a str,
    pub request: &'a str,
    pub priority: &'a str,
    pub team: &'a str,
    pub workflow: &'a str,
    pub reason: Option<&'a str>,
    pub level: &'a str,
}

pub fn insert_proposal(conn: &Connection, p: &NewProposal<'_>) -> BrainResult<i64> {
    let now = now_ms();
    let priority_val = if p.priority.is_empty() {
        "normal"
    } else {
        p.priority
    };
    let level_val = if p.level.is_empty() {
        "vua"
    } else {
        p.level
    };
    conn.execute(
        "INSERT INTO proposals (intake_id, project_id, title, request, priority, team, workflow, reason, status, task_id, error, created_at, updated_at, level, after_proposal_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', NULL, NULL, ?9, ?9, ?10, NULL)",
        params![
            p.intake_id,
            p.project_id,
            p.title,
            p.request,
            priority_val,
            p.team,
            p.workflow,
            p.reason,
            now,
            level_val,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn set_proposal_after_id(
    conn: &Connection,
    id: i64,
    after_proposal_id: Option<i64>,
) -> BrainResult<bool> {
    let now = now_ms();
    let changes = conn.execute(
        "UPDATE proposals SET after_proposal_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![after_proposal_id, now, id],
    )?;
    Ok(changes > 0)
}

pub fn get_proposal(conn: &Connection, id: i64) -> BrainResult<Option<ProposalRow>> {
    let sql = format!("SELECT {PROPOSAL_COLUMNS} FROM proposals WHERE id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(r) = rows.next()? {
        Ok(Some(row_to_proposal(r)?))
    } else {
        Ok(None)
    }
}

pub fn list_proposals(conn: &Connection, status: Option<&str>) -> BrainResult<Vec<ProposalRow>> {
    match status {
        Some(s) => {
            let sql = format!(
                "SELECT {PROPOSAL_COLUMNS} FROM proposals WHERE status = ?1 ORDER BY created_at ASC, id ASC"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![s], row_to_proposal)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
        None => {
            let sql = format!(
                "SELECT {PROPOSAL_COLUMNS} FROM proposals ORDER BY created_at ASC, id ASC"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map([], row_to_proposal)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
    }
}

pub fn set_proposal_status(conn: &Connection, id: i64, status: &str) -> BrainResult<bool> {
    let now = now_ms();
    let changes = conn.execute(
        "UPDATE proposals SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![status, now, id],
    )?;
    Ok(changes > 0)
}

pub fn set_proposal_status_reason(
    conn: &Connection,
    id: i64,
    status: &str,
    reason: Option<&str>,
) -> BrainResult<bool> {
    let now = now_ms();
    let changes = if let Some(r) = reason {
        conn.execute(
            "UPDATE proposals SET status = ?1, reason = ?2, updated_at = ?3 WHERE id = ?4",
            params![status, r, now, id],
        )?
    } else {
        conn.execute(
            "UPDATE proposals SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, now, id],
        )?
    };
    Ok(changes > 0)
}

pub fn set_proposal_priority(conn: &Connection, id: i64, priority: &str) -> BrainResult<bool> {
    let now = now_ms();
    let changes = conn.execute(
        "UPDATE proposals SET priority = ?1, updated_at = ?2 WHERE id = ?3",
        params![priority, now, id],
    )?;
    Ok(changes > 0)
}

pub fn list_approved_proposals(conn: &Connection) -> BrainResult<Vec<ProposalRow>> {
    let sql = format!(
        "SELECT {PROPOSAL_COLUMNS} FROM proposals
         WHERE status = 'approved'
         ORDER BY
             CASE priority
                 WHEN 'urgent' THEN 1
                 WHEN 'normal' THEN 2
                 WHEN 'low' THEN 3
                 ELSE 4
             END ASC,
             created_at ASC,
             id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |r| row_to_proposal(r))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn next_approved_proposal(conn: &Connection) -> BrainResult<Option<ProposalRow>> {
    Ok(list_approved_proposals(conn)?.into_iter().next())
}

pub fn count_active_tasks(conn: &Connection) -> BrainResult<u32> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM (
            SELECT task_id AS id FROM runs WHERE status = 'running' AND task_id IS NOT NULL
            UNION
            SELECT id FROM tasks WHERE auto_state = 'running'
         )",
        [],
        |r| r.get(0),
    )?;
    Ok(count as u32)
}

pub fn set_proposal_started(conn: &Connection, id: i64, task_id: Option<i64>) -> BrainResult<bool> {
    let now = now_ms();
    let changes = conn.execute(
        "UPDATE proposals SET status = 'started', task_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![task_id, now, id],
    )?;
    Ok(changes > 0)
}

pub fn set_proposal_failed(conn: &Connection, id: i64, error: &str) -> BrainResult<bool> {
    let now = now_ms();
    let changes = conn.execute(
        "UPDATE proposals SET status = 'failed', error = ?1, updated_at = ?2 WHERE id = ?3",
        params![error, now, id],
    )?;
    Ok(changes > 0)
}

pub fn set_proposal_error(conn: &Connection, id: i64, error: &str) -> BrainResult<bool> {
    let now = now_ms();
    let changes = conn.execute(
        "UPDATE proposals SET error = ?1, updated_at = ?2 WHERE id = ?3",
        params![error, now, id],
    )?;
    Ok(changes > 0)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QuotaStopRow {
    pub task_id: i64,
    pub stopped_at: i64,
    pub engine: String,
    pub notified: i64,
}

pub fn insert_quota_stop(
    conn: &Connection,
    task_id: i64,
    stopped_at: i64,
    engine: &str,
) -> BrainResult<bool> {
    let changes = conn.execute(
        "INSERT OR IGNORE INTO quota_stops (task_id, stopped_at, engine, notified) VALUES (?1, ?2, ?3, 0)",
        params![task_id, stopped_at, engine],
    )?;
    Ok(changes > 0)
}

pub fn get_quota_stop(conn: &Connection, task_id: i64) -> BrainResult<Option<QuotaStopRow>> {
    let mut stmt = conn.prepare(
        "SELECT task_id, stopped_at, engine, notified FROM quota_stops WHERE task_id = ?1",
    )?;
    let mut rows = stmt.query([task_id])?;
    if let Some(r) = rows.next()? {
        Ok(Some(QuotaStopRow {
            task_id: r.get(0)?,
            stopped_at: r.get(1)?,
            engine: r.get(2)?,
            notified: r.get(3)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn delete_quota_stop(conn: &Connection, task_id: i64) -> BrainResult<bool> {
    let changes = conn.execute("DELETE FROM quota_stops WHERE task_id = ?1", params![task_id])?;
    Ok(changes > 0)
}

/// C3: `metrics_json.quota.reset_at` (ms) of the task's latest run when that run stopped on
/// quota; `None` when it did not, or reported no reset time.
pub fn latest_quota_reset_at(conn: &Connection, task_id: i64) -> Option<i64> {
    let (outcome, metrics): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT outcome, metrics_json FROM runs WHERE task_id = ?1
             ORDER BY started_at DESC, id DESC LIMIT 1",
            params![task_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok()?;
    if outcome.as_deref() != Some(crate::quota::OUTCOME_QUOTA) {
        return None;
    }
    crate::quota::run_reset_at(metrics.as_deref())
}

pub fn list_quota_stops(conn: &Connection) -> BrainResult<Vec<QuotaStopRow>> {
    let mut stmt = conn.prepare(
        "SELECT task_id, stopped_at, engine, notified FROM quota_stops ORDER BY stopped_at ASC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(QuotaStopRow {
                task_id: r.get(0)?,
                stopped_at: r.get(1)?,
                engine: r.get(2)?,
                notified: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn cancel_intake_if_cancellable(conn: &Connection, id: i64) -> BrainResult<bool> {
    let now = now_ms();
    let changes = conn.execute(
        "UPDATE intake SET status = 'cancelled', updated_at = ?1 WHERE id = ?2 AND status IN ('queued', 'failed')",
        params![now, id],
    )?;
    Ok(changes > 0)
}

/// `failed` → `queued` with the error cleared (the next orchestrator tick triages it again).
/// False when the intake is missing or not `failed`.
pub fn retry_intake_if_failed(conn: &Connection, id: i64) -> BrainResult<bool> {
    let now = now_ms();
    let changes = conn.execute(
        "UPDATE intake SET status = 'queued', error = NULL, updated_at = ?1 WHERE id = ?2 AND status = 'failed'",
        params![now, id],
    )?;
    Ok(changes > 0)
}

pub fn approve_proposal_with(
    conn: &Connection,
    id: i64,
    team: &str,
    workflow: &str,
    priority: &str,
    title: &str,
) -> BrainResult<bool> {
    let now = now_ms();
    let changes = conn.execute(
        "UPDATE proposals SET team = ?1, workflow = ?2, priority = ?3, title = ?4, status = 'approved', updated_at = ?5 WHERE id = ?6 AND status = 'pending'",
        params![team, workflow, priority, title, now, id],
    )?;
    Ok(changes > 0)
}

pub fn list_project_display_map(
    conn: &Connection,
) -> BrainResult<std::collections::HashMap<i64, (String, String)>> {
    let mut stmt = conn.prepare("SELECT id, slug, name_vi FROM projects")?;
    let rows = stmt.query_map([], |r| {
        let id: i64 = r.get(0)?;
        let slug: String = r.get(1)?;
        let name_vi: String = r.get(2)?;
        let name = if name_vi.trim().is_empty() {
            slug.clone()
        } else {
            name_vi
        };
        Ok((id, (slug, name)))
    })?;
    let mut map = std::collections::HashMap::new();
    for row in rows {
        let (id, pair) = row?;
        map.insert(id, pair);
    }
    Ok(map)
}

pub fn list_orchestrator_board_intake(conn: &Connection) -> BrainResult<Vec<IntakeRow>> {
    let now = now_ms();
    let cutoff_ms = now - 24 * 3600 * 1000;
    let cutoff_sec = (now / 1000) - 24 * 3600;

    let sql = format!(
        "SELECT {INTAKE_COLUMNS} FROM intake
         WHERE status IN ('queued', 'triaging')
            OR (status = 'failed' AND (updated_at >= ?1 OR created_at >= ?1 OR (updated_at >= ?2 AND updated_at < 10000000000)))
         ORDER BY
             CASE WHEN status = 'failed' THEN 2 ELSE 1 END ASC,
             CASE priority
                 WHEN 'urgent' THEN 1
                 WHEN 'normal' THEN 2
                 WHEN 'low' THEN 3
                 ELSE 4
             END ASC,
             created_at ASC,
             id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![cutoff_ms, cutoff_sec], row_to_intake)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn list_orchestrator_board_proposals(conn: &Connection) -> BrainResult<Vec<ProposalRow>> {
    let sql = format!(
        "SELECT {PROPOSAL_COLUMNS} FROM proposals
         WHERE status IN ('pending', 'approved')
         ORDER BY
             CASE priority
                 WHEN 'urgent' THEN 1
                 WHEN 'normal' THEN 2
                 WHEN 'low' THEN 3
                 ELSE 4
             END ASC,
             created_at ASC,
             id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], row_to_proposal)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn list_orchestrator_board_quota_stops(conn: &Connection) -> BrainResult<Vec<QuotaStopRow>> {
    let sql = "SELECT qs.task_id, qs.stopped_at, qs.engine, qs.notified
         FROM quota_stops qs
         WHERE EXISTS (
             SELECT 1 FROM proposals pr
             WHERE pr.task_id = qs.task_id AND pr.status = 'started'
         )
         ORDER BY qs.stopped_at ASC";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(QuotaStopRow {
                task_id: r.get(0)?,
                stopped_at: r.get(1)?,
                engine: r.get(2)?,
                notified: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn list_orchestrator_board_recent(
    conn: &Connection,
    proj_map: &std::collections::HashMap<i64, (String, String)>,
) -> BrainResult<Vec<serde_json::Value>> {
    let now = now_ms();
    let cutoff_ms = now - 24 * 3600 * 1000;
    let cutoff_sec = (now / 1000) - 24 * 3600;

    let mut recent = Vec::new();

    let prop_sql = format!(
        "SELECT {PROPOSAL_COLUMNS} FROM proposals
         WHERE status IN ('rejected', 'failed')
           AND (updated_at >= ?1 OR created_at >= ?1 OR (updated_at >= ?2 AND updated_at < 10000000000))
         ORDER BY updated_at DESC, id DESC
         LIMIT 20"
    );
    let mut stmt = conn.prepare(&prop_sql)?;
    let prop_rows = stmt.query_map(params![cutoff_ms, cutoff_sec], row_to_proposal)?;
    for r in prop_rows {
        let p = r?;
        let (slug, name) = proj_map
            .get(&p.project_id)
            .cloned()
            .unwrap_or_else(|| (p.project_id.to_string(), p.project_id.to_string()));
        recent.push((
            p.updated_at,
            serde_json::json!({
                "kind": "proposal",
                "id": p.id,
                "intake_id": p.intake_id,
                "project_id": p.project_id,
                "project": slug,
                "project_name": name,
                "title": p.title,
                "request": p.request,
                "priority": p.priority,
                "team": p.team,
                "workflow": p.workflow,
                "reason": p.reason,
                "status": p.status,
                "task_id": p.task_id,
                "error": p.error,
                "created_at": p.created_at,
                "updated_at": p.updated_at,
            }),
        ));
    }

    let intake_sql = format!(
        "SELECT {INTAKE_COLUMNS} FROM intake
         WHERE status IN ('failed', 'cancelled')
           AND (updated_at >= ?1 OR created_at >= ?1 OR (updated_at >= ?2 AND updated_at < 10000000000))
         ORDER BY updated_at DESC, id DESC
         LIMIT 20"
    );
    let mut stmt = conn.prepare(&intake_sql)?;
    let intake_rows = stmt.query_map(params![cutoff_ms, cutoff_sec], row_to_intake)?;
    for r in intake_rows {
        let it = r?;
        let (slug, name) = it
            .project_id
            .and_then(|pid| proj_map.get(&pid).cloned())
            .map(|(s, n)| (Some(s), Some(n)))
            .unwrap_or((None, None));
        recent.push((
            it.updated_at,
            serde_json::json!({
                "kind": "intake",
                "id": it.id,
                "source": it.source,
                "project_id": it.project_id,
                "project": slug,
                "project_name": name,
                "title": it.request,
                "request": it.request,
                "priority": it.priority,
                "status": it.status,
                "error": it.error,
                "created_at": it.created_at,
                "updated_at": it.updated_at,
            }),
        ));
    }

    recent.sort_by(|a, b| b.0.cmp(&a.0));
    recent.truncate(20);

    Ok(recent.into_iter().map(|(_, v)| v).collect())
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FixbugRefRow {
    pub id: i64,
    pub project_id: i64,
    pub kind: String,
    pub ext_id: String,
    pub key: String,
    pub title: String,
    pub url: Option<String>,
    pub intake_id: Option<i64>,
    pub state: String,
    pub note: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

const FIXBUG_REF_COLUMNS: &str =
    "id, project_id, kind, ext_id, key, title, url, intake_id, state, note, created_at, updated_at";

fn row_to_fixbug_ref(r: &rusqlite::Row) -> rusqlite::Result<FixbugRefRow> {
    Ok(FixbugRefRow {
        id: r.get(0)?,
        project_id: r.get(1)?,
        kind: r.get(2)?,
        ext_id: r.get(3)?,
        key: r.get(4)?,
        title: r.get(5)?,
        url: r.get(6)?,
        intake_id: r.get(7)?,
        state: r.get(8)?,
        note: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn insert_fixbug_ref(
    conn: &Connection,
    project_id: i64,
    kind: &str,
    ext_id: &str,
    key: &str,
    title: &str,
    url: Option<&str>,
    intake_id: Option<i64>,
) -> BrainResult<i64> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO fixbug_refs (project_id, kind, ext_id, key, title, url, intake_id, state, note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pulled', NULL, ?8, ?8)",
        params![project_id, kind, ext_id, key, title, url, intake_id, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn fixbug_ref_by_key(
    conn: &Connection,
    project_id: i64,
    key: &str,
) -> BrainResult<Option<FixbugRefRow>> {
    let sql = format!("SELECT {FIXBUG_REF_COLUMNS} FROM fixbug_refs WHERE project_id = ?1 AND key = ?2");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![project_id, key])?;
    if let Some(r) = rows.next()? {
        Ok(Some(row_to_fixbug_ref(r)?))
    } else {
        Ok(None)
    }
}

pub fn fixbug_ref_exists(
    conn: &Connection,
    project_id: i64,
    kind: &str,
    ext_id: &str,
) -> BrainResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM fixbug_refs WHERE project_id = ?1 AND kind = ?2 AND ext_id = ?3",
        params![project_id, kind, ext_id],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}

pub fn set_fixbug_ref_state(
    conn: &Connection,
    id: i64,
    state: &str,
    note: Option<&str>,
) -> BrainResult<()> {
    let now = now_ms();
    conn.execute(
        "UPDATE fixbug_refs SET state = ?1, note = ?2, updated_at = ?3 WHERE id = ?4",
        params![state, note, now, id],
    )?;
    Ok(())
}

pub fn list_fixbug_refs(
    conn: &Connection,
    project_id: i64,
) -> BrainResult<Vec<FixbugRefRow>> {
    let sql = format!(
        "SELECT {FIXBUG_REF_COLUMNS} FROM fixbug_refs WHERE project_id = ?1 ORDER BY id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![project_id], row_to_fixbug_ref)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn set_task_level(conn: &Connection, task_id: i64, level: &str) -> BrainResult<()> {
    let now = now_ms();
    let changes = conn.execute(
        "UPDATE tasks SET level = ?1, updated_at = ?2 WHERE id = ?3",
        params![level, now, task_id],
    )?;
    if changes == 0 {
        return Err(BrainError::error(format!("Không tìm thấy task #{task_id}")));
    }
    Ok(())
}

pub fn task_level(conn: &Connection, task_id: i64) -> BrainResult<String> {
    let lvl: Option<String> = conn
        .query_row(
            "SELECT level FROM tasks WHERE id = ?1",
            params![task_id],
            |r| r.get(0),
        )
        .ok();
    Ok(lvl.unwrap_or_else(|| "vua".to_string()))
}

pub fn set_proposal_level(conn: &Connection, id: i64, level: &str) -> BrainResult<bool> {
    let now = now_ms();
    let changes = conn.execute(
        "UPDATE proposals SET level = ?1, updated_at = ?2 WHERE id = ?3",
        params![level, now, id],
    )?;
    Ok(changes > 0)
}

pub fn set_run_member_id(conn: &Connection, run_id: i64, member_id: &str) -> BrainResult<()> {
    conn.execute(
        "UPDATE runs SET member_id = ?1 WHERE id = ?2",
        params![member_id, run_id],
    )?;
    Ok(())
}

pub fn member_score_avg(conn: &Connection, member_id: &str) -> BrainResult<(f64, i64)> {
    let row: (Option<f64>, i64) = conn.query_row(
        "SELECT AVG(score), COUNT(*) FROM member_scores WHERE member_id = ?1",
        params![member_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok((row.0.unwrap_or(0.0), row.1))
}

pub fn running_count_by_member(conn: &Connection, member_id: &str) -> BrainResult<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM runs WHERE member_id = ?1 AND status = 'running'",
        params![member_id],
        |r| r.get(0),
    )?;
    Ok(count)
}

pub fn last_member_for(conn: &Connection, task_id: i64, role: &str) -> BrainResult<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT member_id FROM runs WHERE task_id = ?1 AND role = ?2 AND member_id IS NOT NULL ORDER BY id DESC LIMIT 1",
    )?;
    let mut rows = stmt.query(params![task_id, role])?;
    if let Some(r) = rows.next()? {
        Ok(r.get(0)?)
    } else {
        Ok(None)
    }
}

pub fn record_member_score(
    conn: &Connection,
    run_id: i64,
    member_id: &str,
    task_id: Option<i64>,
    score: i32,
    note: Option<&str>,
) -> BrainResult<i64> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO member_scores (run_id, member_id, task_id, score, note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT(run_id) DO UPDATE SET score = ?4, note = ?5, updated_at = ?6",
        params![run_id, member_id, task_id, score, note, now],
    )?;
    Ok(conn.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_db() -> (tempfile::TempDir, Connection, i64) {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = open(&dir.path().join("brain.db")).expect("open db");
        let project = Project {
            id: "demo".to_string(),
            path: dir.path().to_string_lossy().to_string(),
            name_vi: "Demo".to_string(),
            aliases: vec![],
            kind: "code".to_string(),
            profile: "default".to_string(),
            repo: None,
            missing: false,
            stack: vec![],
            read_sources: vec![],
            protocol_dir: String::new(),
        };
        sync_projects(&conn, &[project]).expect("sync projects");
        let project_id = project_row_id(&conn, "demo").expect("project id");
        (dir, conn, project_id)
    }

    fn new_run(conn: &Connection, project_id: i64) -> i64 {
        insert_run(
            conn,
            &NewRun {
                task_id: None,
                project_id,
                role: "dev".to_string(),
                profile: "default".to_string(),
                model: None,
                pid: 1,
                pgid: 1,
                pid_start: 1,
                base_commit: None,
                run_dir: None,
            },
        )
        .expect("insert run")
    }

    #[test]
    fn mark_lost_if_running_marks_running_run() {
        let (_dir, conn, project_id) = fresh_db();
        let run_id = new_run(&conn, project_id);
        assert!(mark_lost_if_running(&conn, run_id).expect("mark lost"));
        let run = get_run(&conn, run_id).expect("get run").expect("run row");
        assert_eq!(run.status, "lost");
        assert!(run.ended_at.is_some());
    }

    #[test]
    fn mark_lost_if_running_keeps_killed_run() {
        let (_dir, conn, project_id) = fresh_db();
        let run_id = new_run(&conn, project_id);
        mark_status(&conn, run_id, "killed").expect("mark killed");
        assert!(!mark_lost_if_running(&conn, run_id).expect("mark lost"));
        let run = get_run(&conn, run_id).expect("get run").expect("run row");
        assert_eq!(run.status, "killed");
    }

    #[test]
    fn test_end_stopped_autopilot() {
        let (_dir, conn, project_id) = fresh_db();
        let task_id = upsert_task(&conn, project_id, "t", "T", "t", Some("main"), None)
            .expect("upsert task");

        // 1. auto_state = 'stopped', auto_pid = NULL, có auto_stop_at, auto_json
        let initial_json = "{\"stop\":{\"reason\":\"vuong_mac\"}}";
        conn.execute(
            "UPDATE tasks SET auto_state = 'stopped', auto_pid = NULL, auto_stop_at = 12345, auto_stop_kind = 'vuong_mac', auto_json = ?1 WHERE id = ?2",
            params![initial_json, task_id],
        ).expect("setup stopped task");

        assert!(end_stopped_autopilot(&conn, task_id).expect("end_stopped_autopilot"));
        let auto = task_auto(&conn, task_id).expect("task_auto").expect("row");
        assert_eq!(auto.auto_state.as_deref(), Some("ended"));
        assert_eq!(auto.auto_stop_at, None);
        assert_eq!(auto.auto_stop_kind, None);
        assert_eq!(auto.auto_json.as_deref(), Some(initial_json));

        // 2. auto_state = 'running', auto_pid = Some(1): trả false, không đổi gì
        conn.execute(
            "UPDATE tasks SET auto_state = 'running', auto_pid = 1, auto_stop_at = 9999, auto_stop_kind = 'soft' WHERE id = ?1",
            params![task_id],
        ).expect("setup running task");
        assert!(!end_stopped_autopilot(&conn, task_id).expect("end_stopped_autopilot running"));
        let auto = task_auto(&conn, task_id).expect("task_auto").expect("row");
        assert_eq!(auto.auto_state.as_deref(), Some("running"));
        assert_eq!(auto.auto_pid, Some(1));
        assert_eq!(auto.auto_stop_at, Some(9999));

        // 3. auto_state = 'stopped', auto_pid = Some(1): trả false
        conn.execute(
            "UPDATE tasks SET auto_state = 'stopped', auto_pid = 1 WHERE id = ?1",
            params![task_id],
        ).expect("setup stopped with pid");
        assert!(!end_stopped_autopilot(&conn, task_id).expect("end_stopped_autopilot stopped with pid"));
        let auto = task_auto(&conn, task_id).expect("task_auto").expect("row");
        assert_eq!(auto.auto_state.as_deref(), Some("stopped"));
        assert_eq!(auto.auto_pid, Some(1));

        // 4. auto_state = 'gate' hoặc NULL: trả false
        conn.execute(
            "UPDATE tasks SET auto_state = 'gate', auto_pid = NULL WHERE id = ?1",
            params![task_id],
        ).expect("setup gate task");
        assert!(!end_stopped_autopilot(&conn, task_id).expect("end_stopped_autopilot gate"));
        let auto = task_auto(&conn, task_id).expect("task_auto").expect("row");
        assert_eq!(auto.auto_state.as_deref(), Some("gate"));

        conn.execute(
            "UPDATE tasks SET auto_state = NULL, auto_pid = NULL WHERE id = ?1",
            params![task_id],
        ).expect("setup null auto_state");
        assert!(!end_stopped_autopilot(&conn, task_id).expect("end_stopped_autopilot null"));
    }
}
