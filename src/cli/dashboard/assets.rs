use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use rust_embed::Embed;
use std::path::Path;

#[derive(Embed)]
#[folder = "static/dashboard/"]
struct DashboardAssets;

pub async fn serve(axum::extract::Path(path): axum::extract::Path<String>) -> impl IntoResponse {
    let mime = match Path::new(&path).extension().and_then(|ext| ext.to_str()) {
        Some("js") => "application/javascript",
        Some("css") => "text/css",
        _ => "application/octet-stream",
    };

    match DashboardAssets::get(&path) {
        Some(file) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime)],
            file.data.to_vec(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
