pub mod assets;
pub mod auth;
pub mod pump;
pub mod sse;

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

    // NOTE: Plan 05-03 single-instance lock acquisition goes here.

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

    let state = Arc::new(AppState {
        cfg,
        port: real_port,
        token: token.clone(),
        map_path: map_p,
        db_path: db_p,
        exe,
        hints: hints_tx,
        shutdown: shutdown_rx,
    });

    let app_router = router(state);

    crate::term::emit_out(&format!(
        "agent-brain dashboard: http://127.0.0.1:{real_port}/?token={token}"
    ));
    crate::term::emit_out(
        "Mở URL trên trong trình duyệt. Nhấn Ctrl+C để dừng dashboard (các run do dashboard khởi chạy cũng dừng theo).",
    );

    let signal_fut = async {
        let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())?;
        tokio::select! {
            _ = sigint.recv() => {},
            _ = sigterm.recv() => {},
            _ = sighup.recv() => {},
        }
        Ok::<(), std::io::Error>(())
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
        sig_res = signal_fut => {
            if let Err(e) = sig_res {
                crate::term::emit(&format!("Lỗi lắng nghe tín hiệu: {e}"));
            }
            let _ = shutdown_tx.send(true);
            pump_stop.store(true, Ordering::SeqCst);
            // Spot for plan 05-04: job manager shutdown goes here.

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

    crate::term::emit("Đã dừng dashboard.");
    0
}
