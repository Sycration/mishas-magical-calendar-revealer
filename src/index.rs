use axum::{extract::State, response::IntoResponse};
use axum_template::{RenderHtml, engine};
use serde_json::json;

use crate::AppState;

pub(crate) async fn index(State(AppState { engine }): State<AppState>) -> impl IntoResponse {
    let data = json!({});
    RenderHtml("index.hbs", engine, data)
}
