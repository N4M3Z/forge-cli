use axum::Router;
use axum::routing::{get, post};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::assets;
use commands::view::DashboardView;

mod artifact;
mod browse;
mod files;
mod integrity;
mod shared;

pub struct DashboardState {
    pub view: DashboardView,
    pub provider_targets: Vec<(String, String)>,
    pub settings_filenames: Vec<String>,
    pub local_repos: std::collections::HashMap<String, PathBuf>,
    pub version: String,
    pub binary_hash: String,
    pub scanned_at: String,
}

type SharedState = Arc<RwLock<DashboardState>>;

#[derive(Clone)]
pub struct AppState {
    pub shared: SharedState,
    pub root: PathBuf,
}

pub fn router(shared: SharedState, root: PathBuf) -> Router {
    let app_state = AppState { shared, root };
    Router::new()
        .route("/", get(browse::overview))
        .route("/chrome", get(browse::chrome))
        .route("/repositories", get(browse::modules_page))
        .route("/repositories/{name}", get(browse::module_detail))
        .route(
            "/artifact/{module}/{kind}/{name}",
            get(artifact::artifact_detail_in_module),
        )
        .route(
            "/companion/{module}/{parent}/{name}",
            get(artifact::companion_detail),
        )
        .route("/provenance", get(integrity::provenance_page))
        .route("/adrs", get(integrity::adrs_page))
        .route("/variants", get(integrity::variants_page))
        .route("/adr/{repo}/{id}", get(integrity::adr_detail))
        .route("/search", get(browse::search))
        .route("/refresh", post(artifact::refresh))
        .route(
            "/deployed/{module}/{harness}/{*path}",
            get(artifact::deployed),
        )
        .route(
            "/version/{module}/{kind}/{name}/{sha}",
            get(artifact::version_page),
        )
        .route(
            "/effective/{module}/{kind}/{name}",
            get(artifact::effective_page),
        )
        .route("/settings", get(files::settings_page))
        .route("/settings/{harness}/{index}", get(files::settings_detail))
        .route("/hooks", get(files::hooks_page))
        .route("/hook/{harness}/{index}", get(files::hook_detail))
        .route("/config", get(files::config_page))
        .route("/config/{index}", get(files::config_detail))
        .route("/schemas", get(files::schemas_page))
        .route("/schemas/{group}/{index}", get(files::schema_file_detail))
        .route("/schema/{repo}/{kind}", get(files::schema_page))
        .route("/static/{*path}", get(assets::serve))
        .with_state(app_state)
        .layer(axum::middleware::from_fn(host_guard))
}

/// Rejects requests whose `Host` header is not a loopback name. Without this,
/// a remote page can use DNS rebinding to reach the dashboard on 127.0.0.1 and
/// read local artifact/config content. Browsers always send `Host`; a missing
/// or non-loopback host is refused.
async fn host_guard(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let host_allowed = request
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(host_name)
        .is_some_and(|name| matches!(name, "127.0.0.1" | "localhost" | "forge.localhost" | "::1"));
    if !host_allowed {
        return (
            axum::http::StatusCode::FORBIDDEN,
            "forbidden: dashboard only serves loopback hosts",
        )
            .into_response();
    }
    // The Host guard stops DNS-rebinding reads but not a cross-origin form POST.
    // State-changing methods additionally require a same-origin fetch, which a
    // forged cross-site request cannot supply (browsers set Sec-Fetch-Site).
    if request.method() != axum::http::Method::GET {
        let same_origin = request
            .headers()
            .get("sec-fetch-site")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|site| matches!(site, "same-origin" | "none"));
        if !same_origin {
            return (
                axum::http::StatusCode::FORBIDDEN,
                "forbidden: cross-origin state change rejected",
            )
                .into_response();
        }
    }
    next.run(request).await
}

/// Extracts the hostname from a `Host` header value, dropping any port. Handles
/// bracketed IPv6 literals (`[::1]:40000` -> `::1`).
fn host_name(host: &str) -> &str {
    if let Some(rest) = host.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest);
    }
    host.split(':').next().unwrap_or(host)
}

const PAGE_SIZE: usize = 48;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_name_strips_port_and_brackets() {
        assert_eq!(host_name("127.0.0.1:40000"), "127.0.0.1");
        assert_eq!(host_name("forge.localhost"), "forge.localhost");
        assert_eq!(host_name("[::1]:40000"), "::1");
    }
}
