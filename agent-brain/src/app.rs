use crate::config::Config;
use crate::error::BrainResult;
use crate::projects::Project;
use rusqlite::Connection;
use std::path::Path;

pub struct Ctx {
    pub cfg: Config,
    pub conn: Connection,
    pub map: serde_json::Value,
    pub projects: Vec<Project>,
}

pub fn open_ctx() -> BrainResult<Ctx> {
    let cfg = crate::config::load_config()?;
    let map_p = crate::config::map_path();
    let db_p = crate::config::db_path();
    open_ctx_with(cfg, &map_p, &db_p)
}

pub fn open_ctx_with(cfg: Config, map_path: &Path, db_path: &Path) -> BrainResult<Ctx> {
    let map = crate::projects::load_map(map_path)?;
    let conn = crate::db::open(db_path)?;
    crate::runner::reconcile_stale_runs(&conn)?;
    let projects = crate::projects::projects_of(&map);
    crate::db::sync_projects(&conn, &projects)?;
    Ok(Ctx {
        cfg,
        conn,
        map,
        projects,
    })
}
