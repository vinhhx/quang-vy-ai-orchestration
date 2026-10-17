use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

pub const TICK_MS: u64 = 250;
pub const RECONCILE_EVERY: u32 = 20;

pub fn spawn(
    db_path: PathBuf,
    hints: broadcast::Sender<i64>,
    stop: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("ab-pump".to_string())
        .spawn(move || {
            let mut tick_count: u32 = 0;
            while !stop.load(Ordering::SeqCst) {
                let conn = match crate::db::open(&db_path) {
                    Ok(c) => c,
                    Err(e) => {
                        crate::term::emit(&format!("Lỗi mở CSDL trong pump: {e}"));
                        std::thread::sleep(Duration::from_millis(TICK_MS));
                        continue;
                    }
                };

                let mut max_event_id: i64 =
                    match conn
                        .query_row("SELECT COALESCE(MAX(id), 0) FROM events", [], |r| r.get(0))
                    {
                        Ok(id) => id,
                        Err(e) => {
                            crate::term::emit(&format!("Lỗi đọc events trong pump: {e}"));
                            std::thread::sleep(Duration::from_millis(TICK_MS));
                            continue;
                        }
                    };

                let mut last_data_version: i64 =
                    match conn.pragma_query_value(None, "data_version", |r| r.get(0)) {
                        Ok(v) => v,
                        Err(e) => {
                            crate::term::emit(&format!("Lỗi đọc data_version trong pump: {e}"));
                            std::thread::sleep(Duration::from_millis(TICK_MS));
                            continue;
                        }
                    };

                while !stop.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(TICK_MS));
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }

                    tick_count = tick_count.wrapping_add(1);
                    if tick_count % RECONCILE_EVERY == 0 {
                        match crate::runner::reconcile_stale_runs(&conn) {
                            Ok(marked) => {
                                if marked > 0 {
                                    let _ = hints.send(0);
                                }
                            }
                            Err(e) => {
                                crate::term::emit(&format!("Lỗi reconcile trong pump: {e}"));
                                break;
                            }
                        }
                    }

                    let current_version: i64 =
                        match conn.pragma_query_value(None, "data_version", |r| r.get(0)) {
                            Ok(v) => v,
                            Err(e) => {
                                crate::term::emit(&format!("Lỗi đọc data_version trong pump: {e}"));
                                break;
                            }
                        };

                    if current_version != last_data_version {
                        last_data_version = current_version;
                        let query_res: Result<Vec<(i64, i64)>, rusqlite::Error> = (|| {
                            let mut stmt = conn.prepare(
                                "SELECT run_id, MAX(id) FROM events WHERE id > ?1 GROUP BY run_id",
                            )?;
                            let rows = stmt.query_map([max_event_id], |r| {
                                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
                            })?;
                            let mut list = Vec::new();
                            for row in rows {
                                list.push(row?);
                            }
                            Ok(list)
                        })(
                        );

                        match query_res {
                            Ok(runs) => {
                                for (run_id, run_max_id) in runs {
                                    if run_max_id > max_event_id {
                                        max_event_id = run_max_id;
                                    }
                                    let _ = hints.send(run_id);
                                }
                                let _ = hints.send(0);
                            }
                            Err(e) => {
                                crate::term::emit(&format!("Lỗi đọc events mới trong pump: {e}"));
                                break;
                            }
                        }
                    }
                }
            }
        })
        .expect("spawn ab-pump thread")
}
