use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "assets/static/"]
struct StaticAssets;

#[derive(Embed)]
#[folder = "assets/vendor/"]
struct VendorAssets;

pub async fn static_handler(Path(path): Path<String>) -> Response {
    serve_embedded::<StaticAssets>(&path)
}

pub async fn vendor_handler(Path(path): Path<String>) -> Response {
    serve_embedded::<VendorAssets>(&path)
}

fn serve_embedded<E: Embed>(path: &str) -> Response {
    match E::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime.as_ref().to_string()),
                    (
                        header::CACHE_CONTROL,
                        "public, max-age=31536000, immutable".to_string(),
                    ),
                ],
                file.data.to_vec(),
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
