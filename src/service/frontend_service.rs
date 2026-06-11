use axum::http::header;
use axum::response::IntoResponse;

// The frontend is built by Vite (`cd frontend && npm run build`) into
// static/dist/ with fixed filenames, then embedded into the binary here.
pub(crate) async fn serve_html() -> impl IntoResponse {
    axum::response::Html(include_str!("../../static/dist/index.html"))
}

pub(crate) async fn serve_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript")],
        include_str!("../../static/dist/app.js"),
    )
}

pub(crate) async fn serve_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css")],
        include_str!("../../static/dist/app.css"),
    )
}
