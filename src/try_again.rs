use axum::{extract::State, response::IntoResponse, Form};
use axum_template::{RenderHtml, engine};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;


#[derive(Deserialize)]
pub(crate) struct ErrorReason {
    reason: String,
}

pub(crate) async fn try_again(State(AppState { engine }): State<AppState>, Form(form): Form<ErrorReason>) -> impl IntoResponse {
    let data = json!({
        "error_reason": form.reason
    });
    RenderHtml("try_again.hbs", engine, data)
}
