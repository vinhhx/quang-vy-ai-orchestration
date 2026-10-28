use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};

pub struct Asset {
    pub path: &'static str,
    pub mime: &'static str,
    pub body: &'static [u8],
}

pub static ASSETS: &[Asset] = &[
    Asset {
        path: "/",
        mime: "text/html; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/index.html"),
    },
    Asset {
        path: "/app.css",
        mime: "text/css; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/app.css"),
    },
    Asset {
        path: "/vendor/alpine.csp.min.js",
        mime: "text/javascript; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/vendor/alpine.csp.min.js"),
    },
    Asset {
        path: "/js/theme.js",
        mime: "text/javascript; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/js/theme.js"),
    },
    Asset {
        path: "/js/app.js",
        mime: "text/javascript; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/js/app.js"),
    },
    Asset {
        path: "/js/overview.js",
        mime: "text/javascript; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/js/overview.js"),
    },
    Asset {
        path: "/screens/overview.html",
        mime: "text/html; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/screens/overview.html"),
    },
    Asset {
        path: "/js/assign.js",
        mime: "text/javascript; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/js/assign.js"),
    },
    Asset {
        path: "/screens/assign.html",
        mime: "text/html; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/screens/assign.html"),
    },
    Asset {
        path: "/js/task.js",
        mime: "text/javascript; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/js/task.js"),
    },
    Asset {
        path: "/screens/task.html",
        mime: "text/html; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/screens/task.html"),
    },
    Asset {
        path: "/js/stats.js",
        mime: "text/javascript; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/js/stats.js"),
    },
    Asset {
        path: "/screens/stats.html",
        mime: "text/html; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/screens/stats.html"),
    },
    Asset {
        path: "/js/projects.js",
        mime: "text/javascript; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/js/projects.js"),
    },
    Asset {
        path: "/screens/projects.html",
        mime: "text/html; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/screens/projects.html"),
    },
    Asset {
        path: "/js/board.js",
        mime: "text/javascript; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/js/board.js"),
    },
    Asset {
        path: "/screens/board.html",
        mime: "text/html; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/screens/board.html"),
    },
    Asset {
        path: "/js/settings.js",
        mime: "text/javascript; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/js/settings.js"),
    },
    Asset {
        path: "/screens/settings.html",
        mime: "text/html; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/screens/settings.html"),
    },
    Asset {
        path: "/js/telegram.js",
        mime: "text/javascript; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/js/telegram.js"),
    },
    Asset {
        path: "/screens/telegram.html",
        mime: "text/html; charset=utf-8",
        body: include_bytes!("../../assets/dashboard/screens/telegram.html"),
    },
];

pub fn lookup(path: &str) -> Option<&'static Asset> {
    ASSETS.iter().find(|a| a.path == path)
}

pub async fn serve(method: Method, uri: Uri) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            [(CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Phương thức không được hỗ trợ",
        )
            .into_response();
    }

    let path = uri.path();
    if path.starts_with("/api/") || path == "/api" {
        let body = serde_json::json!({ "error": "Không có API này" });
        return (
            StatusCode::NOT_FOUND,
            [(CONTENT_TYPE, "application/json; charset=utf-8")],
            axum::Json(body),
        )
            .into_response();
    }

    if let Some(asset) = lookup(path) {
        if method == Method::HEAD {
            Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, asset.mime)
                .body(Body::empty())
                .unwrap()
        } else {
            Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, asset.mime)
                .body(Body::from(asset.body))
                .unwrap()
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            [(CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Không tìm thấy",
        )
            .into_response()
    }
}
