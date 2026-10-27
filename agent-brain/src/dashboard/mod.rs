pub mod api_board;
pub mod api_projects;
pub mod api_runs;
pub mod api_settings;
pub mod api_stats;
pub mod api_tasks;
pub mod api_workflows;
pub mod assets;
pub mod auth;
pub mod board;
pub mod board_scan;
pub mod jobs;
pub mod lock;
pub mod pipeline;
pub mod pump;
pub mod settings_store;
pub mod sse;
pub mod taskstate;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::future::IntoFuture;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, watch};

pub struct AppState {
    pub cfg: crate::config::Config,
    pub port: u16,
    pub token: String,
    pub map_path: PathBuf,
    pub db_path: PathBuf,
    pub exe: PathBuf,
    pub hints: broadcast::Sender<i64>,
    pub shutdown: watch::Receiver<bool>,
    pub jobs: std::sync::Arc<jobs::JobManager>,
}

pub fn json_error(status: StatusCode, message: &str) -> Response {
    let body = serde_json::json!({ "error": message });
    (
        status,
        [(
            axum::http::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )],
        axum::Json(body),
    )
        .into_response()
}

pub async fn blocking<T, F>(f: F) -> crate::error::BrainResult<T>
where
    F: FnOnce() -> crate::error::BrainResult<T> + Send + 'static,
    T: Send + 'static,
{
    match tokio::task::spawn_blocking(f).await {
        Ok(res) => res,
        Err(e) => Err(crate::error::BrainError::error(format!(
            "Lỗi tác vụ blocking: {e}"
        ))),
    }
}

pub fn router(state: Arc<AppState>) -> axum::Router {
    axum::Router::new()
        .merge(api_tasks::routes())
        .merge(api_runs::routes())
        .merge(api_projects::routes())
        .merge(api_stats::routes())
        .merge(api_workflows::routes())
        .merge(api_settings::routes())
        .merge(api_board::routes())
        .route("/api/runs/{id}/stream", axum::routing::get(sse::stream))
        .fallback(assets::serve)
        .layer(axum::extract::DefaultBodyLimit::max(65536))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::gate,
        ))
        .with_state(state)
}

pub fn cmd_serve(port_flag: Option<u16>) -> i32 {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            crate::term::emit(&format!("Không khởi tạo được tokio runtime: {e}"));
            return 1;
        }
    };
    rt.block_on(async_serve(port_flag))
}

async fn async_serve(port_flag: Option<u16>) -> i32 {
    let cfg = match crate::config::load_config() {
        Ok(c) => c,
        Err(e) => {
            crate::term::emit(&e.message);
            return e.code;
        }
    };

    let port = port_flag.unwrap_or(cfg.dashboard_port);

    let data_dir = crate::config::data_dir();
    let mut serve_lock = match lock::acquire(&data_dir) {
        Ok(l) => l,
        Err(lock::LockError::Held { pid, port }) => {
            let pid_str = pid
                .map(|p| p.to_string())
                .unwrap_or_else(|| "?".to_string());
            let port_str = port
                .map(|p| p.to_string())
                .unwrap_or_else(|| "?".to_string());
            crate::term::emit(&format!(
                "agent-brain serve đã chạy (pid {pid_str}) tại http://127.0.0.1:{port_str}/ — mở URL có token in ở terminal đang chạy serve, hoặc dừng nó trước (Ctrl+C)."
            ));
            return 1;
        }
        Err(lock::LockError::Io(e)) => {
            let path = data_dir.join(lock::LOCK_FILE);
            crate::term::emit(&format!("Không tạo được file khóa {}: {e}", path.display()));
            return 1;
        }
    };

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            crate::term::emit(&format!("Không xác định được đường dẫn thực thi: {e}"));
            return 1;
        }
    };

    let listener = match tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await {
        Ok(l) => l,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                crate::term::emit(&format!(
                    "Cổng {port} đang được dùng — chọn cổng khác: agent-brain serve --port <cổng> (hoặc khóa dashboard_port trong config.json)"
                ));
            } else {
                crate::term::emit(&format!("Không mở được cổng 127.0.0.1:{port}: {e}"));
            }
            return 1;
        }
    };

    let local_addr = match listener.local_addr() {
        Ok(a) => a,
        Err(e) => {
            crate::term::emit(&format!("Không đọc được địa chỉ lắng nghe: {e}"));
            return 1;
        }
    };
    let real_port = local_addr.port();

    if let Err(e) = serve_lock.write_info(std::process::id(), real_port, crate::db::now_ms()) {
        crate::term::emit(&format!("Không ghi được thông tin khóa: {e}"));
        return 1;
    }

    let map_p = crate::config::map_path();
    let db_p = crate::config::db_path();
    if let Err(e) = crate::app::open_ctx_with(cfg.clone(), &map_p, &db_p) {
        crate::term::emit(&e.message);
        return e.code;
    }

    let token = match auth::new_token() {
        Ok(t) => t,
        Err(e) => {
            crate::term::emit(&format!("Không tạo được token ngẫu nhiên: {e}"));
            return 1;
        }
    };

    let (hints_tx, _) = broadcast::channel::<i64>(1024);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let pump_stop = Arc::new(AtomicBool::new(false));
    let _pump_handle = pump::spawn(db_p.clone(), hints_tx.clone(), pump_stop.clone());

    let jobs = jobs::JobManager::new(exe.clone());

    let state = Arc::new(AppState {
        cfg: cfg.clone(),
        port: real_port,
        token: token.clone(),
        map_path: map_p,
        db_path: db_p,
        exe,
        hints: hints_tx,
        shutdown: shutdown_rx,
        jobs: jobs.clone(),
    });

    let app_router = router(state.clone());

    let mut sigint = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
    {
        Ok(s) => s,
        Err(e) => {
            crate::term::emit(&format!("Không đăng ký được tín hiệu ngắt (SIGINT): {e}"));
            return 1;
        }
    };
    let mut sigterm = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    {
        Ok(s) => s,
        Err(e) => {
            crate::term::emit(&format!(
                "Không đăng ký được tín hiệu kết thúc (SIGTERM): {e}"
            ));
            return 1;
        }
    };
    let mut sighup = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()) {
        Ok(s) => s,
        Err(e) => {
            crate::term::emit(&format!(
                "Không đăng ký được tín hiệu ngắt kết nối (SIGHUP): {e}"
            ));
            return 1;
        }
    };

    crate::term::emit_out(&format!(
        "agent-brain dashboard: http://127.0.0.1:{real_port}/?token={token}"
    ));
    crate::term::emit_out(
        "Mở URL trên trong trình duyệt. Nhấn Ctrl+C để dừng dashboard (các run do dashboard khởi chạy cũng dừng theo).",
    );
    let bot = crate::telegram::start(state.clone());

    let signal_fut = async {
        tokio::select! {
            _ = sigint.recv() => {},
            _ = sigterm.recv() => {},
            _ = sighup.recv() => {},
        }
    };

    let (graceful_tx, graceful_rx) = tokio::sync::oneshot::channel::<()>();
    let serve_fut = axum::serve(listener, app_router)
        .with_graceful_shutdown(async move {
            let _ = graceful_rx.await;
        })
        .into_future();
    tokio::pin!(serve_fut);

    tokio::select! {
        res = &mut serve_fut => {
            if let Err(e) = res {
                crate::term::emit(&format!("Lỗi serve: {e}"));
            }
        }
        _ = signal_fut => {
            let _ = shutdown_tx.send(true);
            pump_stop.store(true, Ordering::SeqCst);
            if let Some(b) = &bot {
                b.stop();
            }

            let jobs_clone = jobs.clone();
            let grace = Duration::from_secs(cfg.kill_grace_secs + 5);
            let shutdown_task = tokio::task::spawn_blocking(move || {
                jobs_clone.shutdown(grace);
            });

            tokio::select! {
                _ = shutdown_task => {},
                _ = sigint.recv() => {
                    jobs.kill_all_now();
                    crate::term::emit("Dừng ngay (SIGKILL các run còn lại)");
                    std::process::exit(130);
                }
                _ = sigterm.recv() => {
                    jobs.kill_all_now();
                    crate::term::emit("Dừng ngay (SIGKILL các run còn lại)");
                    std::process::exit(130);
                }
                _ = sighup.recv() => {
                    jobs.kill_all_now();
                    crate::term::emit("Dừng ngay (SIGKILL các run còn lại)");
                    std::process::exit(130);
                }
            }

            let _ = graceful_tx.send(());

            tokio::select! {
                res = &mut serve_fut => {
                    if let Err(e) = res {
                        crate::term::emit(&format!("Lỗi serve khi dừng: {e}"));
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(3)) => {}
            }
        }
    }

    drop(serve_lock);
    crate::term::emit("Đã dừng dashboard.");
    0
}
