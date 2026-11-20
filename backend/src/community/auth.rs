use super::User;
use crate::{
    AppState,
    api::{ApiError, Result, bad},
    platforms::discord::{Discord, manages, snowflake},
};
use axum::{
    Json,
    extract::{Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{AppendHeaders, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
pub fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|s| {
            s.trim()
                .split_once('=')
                .filter(|(key, _)| *key == name)
                .map(|(_, value)| value)
        })
}
fn set_cookie(s: &AppState, name: &str, value: &str, seconds: i32, same_site: &str) -> String {
    format!(
        "{name}={value}; Path=/; HttpOnly; SameSite={same_site}; Max-Age={seconds}{}",
        if s.secure_cookie { "; Secure" } else { "" }
    )
}
fn unauthorized() -> ApiError {
    ApiError(
        StatusCode::UNAUTHORIZED,
        "Sign in with Discord to access your server library".into(),
    )
}
pub async fn current(s: &AppState, headers: &HeaderMap) -> Result<User> {
    let token = cookie(headers, "la_user").ok_or_else(unauthorized)?;
    sqlx::query_as("SELECT u.* FROM community_users u JOIN community_sessions s ON s.user_id=u.id WHERE s.token_hash=$1 AND s.expires_at>now()")
        .bind(hash(token)).fetch_optional(&s.pool).await?.ok_or_else(unauthorized)
}
pub async fn middleware(State(s): State<AppState>, mut req: Request, next: Next) -> Response {
    match current(&s, req.headers()).await {
        Ok(user) => {
            req.extensions_mut().insert(user);
            next.run(req).await
        }
        Err(e) => e.into_response(),
    }
}
pub async fn config(State(s): State<AppState>) -> Json<Value> {
    Json(
        json!({"discord_enabled":s.discord.is_some(),"install_url":s.discord.as_ref().map(|d|format!("https://discord.com/oauth2/authorize?client_id={}&scope=bot%20applications.commands&permissions=84992&integration_type=0",d.application_id))}),
    )
}
pub async fn login(State(s): State<AppState>) -> Result<Response> {
    let d = s
        .discord
        .as_ref()
        .ok_or_else(|| bad("The Discord bot is not configured yet"))?;
    let state = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO oauth_states(token_hash,expires_at) VALUES($1,now()+interval '10 minutes')",
    )
    .bind(hash(&state))
    .execute(&s.pool)
    .await?;
    let mut url = url::Url::parse("https://discord.com/oauth2/authorize").unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", &d.application_id)
        .append_pair(
            "redirect_uri",
            &format!("{}/api/community/auth/callback", d.base_url),
        )
        .append_pair("response_type", "code")
        .append_pair("scope", "identify guilds")
        .append_pair("state", &state);
    Ok((
        [(
            header::SET_COOKIE,
            set_cookie(&s, "la_oauth", &state, 600, "Lax"),
        )],
        Redirect::to(url.as_str()),
    )
        .into_response())
}
#[derive(Deserialize)]
pub struct Callback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}
pub async fn callback(
    State(s): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Callback>,
) -> Response {
    match complete(&s, &headers, q).await {
        Ok(token) => (
            AppendHeaders([
                (
                    header::SET_COOKIE,
                    set_cookie(&s, "la_user", &token, 604800, "Strict"),
                ),
                (header::SET_COOKIE, set_cookie(&s, "la_oauth", "", 0, "Lax")),
            ]),
            Redirect::to("/community"),
        )
            .into_response(),
        Err(_) => (
            [(header::SET_COOKIE, set_cookie(&s, "la_oauth", "", 0, "Lax"))],
            Redirect::to("/community?login_error=1"),
        )
            .into_response(),
    }
}
async fn complete(s: &AppState, headers: &HeaderMap, q: Callback) -> Result<String> {
    if q.error.is_some() {
        return Err(bad("Login cancelled"));
    }
    let state = q.state.ok_or_else(|| bad("Missing OAuth state"))?;
    let browser = cookie(headers, "la_oauth").ok_or_else(|| bad("Missing OAuth cookie"))?;
    use subtle::ConstantTimeEq;
    if !bool::from(state.as_bytes().ct_eq(browser.as_bytes())) {
        return Err(bad("Invalid OAuth state"));
    }
    let consumed = sqlx::query("DELETE FROM oauth_states WHERE token_hash=$1 AND expires_at>now()")
        .bind(hash(&state))
        .execute(&s.pool)
        .await?
        .rows_affected();
    if consumed != 1 {
        return Err(bad("OAuth state expired or already used"));
    }
    let d = s
        .discord
        .as_ref()
        .ok_or_else(|| bad("Discord is unavailable"))?;
    let response = d
        .client()
        .post("https://discord.com/api/oauth2/token")
        .form(&[
            ("client_id", d.application_id.as_str()),
            ("client_secret", d.client_secret.as_str()),
            ("grant_type", "authorization_code"),
            (
                "code",
                q.code.as_deref().ok_or_else(|| bad("Missing code"))?,
            ),
            (
                "redirect_uri",
                &format!("{}/api/community/auth/callback", d.base_url),
            ),
        ])
        .send()
        .await
        .map_err(|_| bad("Discord login failed"))?;
    let token = Discord::response(response)
        .await
        .map_err(|_| bad("Discord login failed"))?;
    let access = token["access_token"]
        .as_str()
        .ok_or_else(|| bad("Missing Discord access token"))?;
    let profile = Discord::response(
        d.client()
            .get("https://discord.com/api/v10/users/@me")
            .bearer_auth(access)
            .send()
            .await
            .map_err(|_| bad("Discord unavailable"))?,
    )
    .await
    .map_err(|_| bad("Discord profile unavailable"))?;
    let external = profile["id"]
        .as_str()
        .filter(|id| snowflake(id))
        .ok_or_else(|| bad("Invalid Discord profile"))?;
    let mut guilds = Vec::new();
    let mut after = String::from("0");
    for _ in 0..10 {
        let page = Discord::response(
            d.client()
                .get("https://discord.com/api/v10/users/@me/guilds")
                .query(&[("limit", "200"), ("after", after.as_str())])
                .bearer_auth(access)
                .send()
                .await
                .map_err(|_| bad("Discord unavailable"))?,
        )
        .await
        .map_err(|_| bad("Discord servers unavailable"))?;
        let page = page.as_array().ok_or_else(|| bad("Invalid server list"))?;
        if page.is_empty() {
            break;
        }
        after = page
            .last()
            .and_then(|g| g["id"].as_str())
            .unwrap_or("0")
            .into();
        guilds.extend(page.clone());
        if page.len() < 200 {
            break;
        }
    }
    let mut tx = s.pool.begin().await?;
    let user = super::service::upsert_user(
        &mut tx,
        "discord",
        external,
        profile["global_name"]
            .as_str()
            .or_else(|| profile["username"].as_str())
            .unwrap_or("Discord user"),
        &avatar(&profile),
    )
    .await?;
    sqlx::query("DELETE FROM login_guilds WHERE user_id=$1")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    for guild in guilds {
        let Some(id) = guild["id"].as_str().filter(|id| snowflake(id)) else {
            continue;
        };
        let permissions = guild["permissions"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        sqlx::query(
            "INSERT INTO login_guilds(user_id,external_id,name,can_manage) VALUES($1,$2,$3,$4)",
        )
        .bind(user.id)
        .bind(id)
        .bind(guild["name"].as_str().unwrap_or("Server"))
        .bind(manages(permissions) || guild["owner"].as_bool() == Some(true))
        .execute(&mut *tx)
        .await?;
    }
    let session = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    sqlx::query("INSERT INTO community_sessions(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '7 days')").bind(hash(&session)).bind(user.id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(session)
}
pub fn avatar(profile: &Value) -> String {
    match (profile["id"].as_str(), profile["avatar"].as_str()) {
        (Some(id), Some(hash))
            if snowflake(id) && hash.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') =>
        {
            format!("https://cdn.discordapp.com/avatars/{id}/{hash}.png")
        }
        _ => String::new(),
    }
}
pub async fn logout(State(s): State<AppState>, headers: HeaderMap) -> Result<Response> {
    if let Some(token) = cookie(&headers, "la_user") {
        sqlx::query("DELETE FROM community_sessions WHERE token_hash=$1")
            .bind(hash(token))
            .execute(&s.pool)
            .await?;
    }
    Ok((
        [(
            header::SET_COOKIE,
            set_cookie(&s, "la_user", "", 0, "Strict"),
        )],
        Json(json!({"ok":true})),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_redirect_preserves_both_cookies() {
        let response = (
            AppendHeaders([
                (
                    header::SET_COOKIE,
                    "la_user=session; Path=/; HttpOnly; SameSite=Strict",
                ),
                (header::SET_COOKIE, "la_oauth=; Path=/; HttpOnly; Max-Age=0"),
            ]),
            Redirect::to("/community"),
        )
            .into_response();
        let cookies: Vec<_> = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .collect();
        assert_eq!(cookies.len(), 2);
        assert!(cookies[0].to_str().unwrap().starts_with("la_user=session;"));
        assert!(cookies[1].to_str().unwrap().starts_with("la_oauth=;"));
    }
}
