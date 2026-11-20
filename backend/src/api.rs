use crate::AppState;
use axum::{
    Json,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use subtle::ConstantTimeEq;

pub struct ApiError(pub StatusCode, pub String);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &e
            && db.is_unique_violation()
        {
            return Self(
                StatusCode::CONFLICT,
                "This URL has already been added".into(),
            );
        }
        tracing::error!(error = %e, "database request failed");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to complete the operation".into(),
        )
    }
}
pub(crate) type Result<T> = std::result::Result<T, ApiError>;
pub async fn feed_options(State(state): State<AppState>) -> Json<Value> {
    Json(json!({"rsshub_base_url": state.rsshub_base_url.as_str()}))
}
pub(crate) fn bad(s: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, s.into())
}
pub(crate) fn not_found() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "Record not found".into())
}
fn matches_token(a: &str, b: &str) -> bool {
    a.as_bytes().ct_eq(b.as_bytes()).into()
}
pub async fn auth(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let bearer = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    let cookie = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            s.split(';')
                .find_map(|s| s.trim().strip_prefix("la_session="))
        });
    if !bearer
        .or(cookie)
        .is_some_and(|v| matches_token(v, &state.token))
    {
        return ApiError(
            StatusCode::UNAUTHORIZED,
            "Administrator authentication required".into(),
        )
        .into_response();
    }
    next.run(req).await
}
#[derive(Deserialize)]
pub struct Login {
    token: String,
}
pub async fn login(
    State(state): State<AppState>,
    Json(input): Json<Login>,
) -> Result<impl IntoResponse> {
    if !matches_token(&input.token, &state.token) {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "Incorrect access key".into(),
        ));
    }
    let secure = if state.secure_cookie { "; Secure" } else { "" };
    let cookie = format!(
        "la_session={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=604800{secure}",
        state.token
    );
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({"ok":true}))))
}
pub async fn logout() -> impl IntoResponse {
    (
        [(
            header::SET_COOKIE,
            "la_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0",
        )],
        Json(json!({"ok":true})),
    )
}
pub async fn health(State(s): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(&s.pool).await {
        Ok(_) => (StatusCode::OK, Json(json!({"status":"ok"}))),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"status":"unavailable"})),
        ),
    }
}
