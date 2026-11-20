use super::*;
use crate::{
    AppState,
    platforms::{Platform, PlatformFuture},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;

#[derive(Default)]
struct MockPlatform {
    sent: Mutex<Vec<Value>>,
}
impl Platform for MockPlatform {
    fn notification(
        &self,
        entry: &crate::models::Entry,
        artist: Uuid,
        recipients: &[String],
        nonce: &str,
    ) -> anyhow::Result<Value> {
        crate::platforms::discord::bot_payload(entry, artist, recipients, nonce)
    }
    fn membership<'a>(&'a self, server: &'a str, user: &'a str) -> PlatformFuture<'a, bool> {
        Box::pin(async move {
            anyhow::ensure!(
                user != "revoked" && !(server == "200" && user == "member"),
                "Not a member"
            );
            Ok(user == "manager")
        })
    }
    fn send<'a>(&'a self, _channel: &'a str, payload: Value) -> PlatformFuture<'a, String> {
        Box::pin(async move {
            self.sent.lock().unwrap().push(payload);
            Ok("message-id".into())
        })
    }
}
async fn request(app: &Router, path: &str, token: &str, method: &str, body: Value) -> (u16, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .method(method)
                .header("cookie", format!("la_user={token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), 1_000_000).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
#[tokio::test]
#[ignore = "requires COMMUNITY_TEST_DATABASE_URL pointing to an isolated *_test database"]
async fn server_and_user_isolation() {
    let url = std::env::var("COMMUNITY_TEST_DATABASE_URL").expect("Test database required");
    assert!(url::Url::parse(&url).unwrap().path().ends_with("_test"));
    let admin = sqlx::PgPool::connect(&url).await.unwrap();
    let schema = format!("community_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let path = format!("SET search_path TO {schema}");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |connection, _| {
            let path = path.clone();
            Box::pin(async move {
                sqlx::query(&path).execute(connection).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let adapter = Arc::new(MockPlatform::default());
    let mut platforms: HashMap<String, Arc<dyn Platform>> = HashMap::new();
    platforms.insert("discord".into(), adapter.clone());
    let s = AppState {
        tagger: None,
        pool: pool.clone(),
        token: Arc::new("unused-owner-token".into()),
        secure_cookie: false,
        download_dir: Arc::new(std::path::PathBuf::from(".test-runtime/downloads")),
        max_download_bytes: 1024,
        wake: Arc::new(tokio::sync::Notify::new()),
        rsshub_base_url: Arc::new("https://rsshub.app".into()),
        discord: None,
        platforms: Arc::new(platforms),
    };
    let mut users = Vec::new();
    for external in ["manager", "member", "revoked"] {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO community_users(id,platform,external_id,display_name) VALUES($1,'discord',$2,$2)").bind(id).bind(external).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO community_sessions(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '1 hour')").bind(auth::hash(external)).bind(id).execute(&pool).await.unwrap();
        users.push(id);
    }
    let a = service::connect(&s, "discord", "100", "First server")
        .await
        .ok()
        .unwrap();
    let b = service::connect(&s, "discord", "200", "Second server")
        .await
        .ok()
        .unwrap();
    let artist_a = service::track(
        &s,
        a.id,
        &ArtistInput {
            name: "Shared artist".into(),
            url: "https://example.com/shared".into(),
            source: "rss".into(),
            category: String::new(),
        },
    )
    .await
    .ok()
    .unwrap();
    let artist_b = service::track(
        &s,
        b.id,
        &ArtistInput {
            name: "Same feed, different server".into(),
            url: "https://example.com/shared".into(),
            source: "rss".into(),
            category: String::new(),
        },
    )
    .await
    .ok()
    .unwrap();
    assert_eq!(artist_a["feed_id"], artist_b["feed_id"]);
    let private_artist = service::track(
        &s,
        b.id,
        &ArtistInput {
            name: "Private artist".into(),
            url: "https://example.com/private".into(),
            source: "rss".into(),
            category: String::new(),
        },
    )
    .await
    .ok()
    .unwrap();
    let shared = Uuid::new_v4();
    let private = Uuid::new_v4();
    for (entry, feed) in [(shared, &artist_a), (private, &private_artist)] {
        sqlx::query("INSERT INTO entries(id,feed_id,guid,title,author,url,published_at) VALUES($1,$2,$3,'Artwork','Artist','https://example.com/post',now())")
            .bind(entry).bind(feed["feed_id"].as_str().unwrap().parse::<Uuid>().unwrap()).bind(entry.to_string()).execute(&pool).await.unwrap();
    }
    let app = routes(s.clone()).with_state(s.clone());
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/posts/{shared}", a.id),
            "member",
            "GET",
            json!({})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/posts/{shared}", a.id),
            "revoked",
            "GET",
            json!({})
        )
        .await
        .0,
        403
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/posts/{private}", a.id),
            "manager",
            "GET",
            json!({})
        )
        .await
        .0,
        404
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/posts/{shared}", a.id),
            "",
            "GET",
            json!({})
        )
        .await
        .0,
        401
    );
    let posts = format!("/servers/{}/posts", a.id);
    assert_eq!(request(&app, &posts, "", "GET", json!({})).await.0, 401);
    assert_eq!(
        request(&app, &posts, "revoked", "GET", json!({})).await.0,
        403
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/posts", b.id),
            "member",
            "GET",
            json!({})
        )
        .await
        .0,
        403
    );
    let response = request(
        &app,
        &format!("{posts}/{shared}"),
        "member",
        "PATCH",
        json!({"favorite":true,"liked":true,"user_id":users[0]}),
    )
    .await;
    assert_eq!(response.0, 200);
    assert_eq!(response.1["likes"], 1);
    let own = request(&app, &posts, "member", "GET", json!({})).await.1;
    let other = request(&app, &posts, "manager", "GET", json!({})).await.1;
    assert_eq!(own["items"][0]["favorite"], true);
    assert_eq!(other["items"][0]["favorite"], false);
    assert_eq!(other["items"][0]["liked"], false);
    assert_eq!(other["items"][0]["likes"], 1);
    assert_eq!(
        request(
            &app,
            &format!("{posts}?favorites=true"),
            "manager",
            "GET",
            json!({})
        )
        .await
        .1["total"],
        0
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/posts", b.id),
            "manager",
            "GET",
            json!({})
        )
        .await
        .1["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == shared.to_string())
            .unwrap()["likes"],
        0
    );
    assert_eq!(
        request(
            &app,
            &format!("{posts}/{private}"),
            "member",
            "PATCH",
            json!({"liked":true})
        )
        .await
        .0,
        404
    );
    let artist = artist_a["id"].as_str().unwrap();
    let artist_path = format!("/servers/{}/artists/{artist}", a.id);
    assert_eq!(
        request(
            &app,
            &artist_path,
            "member",
            "PATCH",
            json!({"favorite":true})
        )
        .await
        .0,
        200
    );
    let artist_list = request(
        &app,
        &format!("/servers/{}/artists", a.id),
        "member",
        "GET",
        json!({}),
    )
    .await
    .1;
    assert_eq!(artist_list[0]["favorite"], true);
    assert_eq!(artist_list[0]["notify"], false);
    assert_eq!(
        request(&app, &artist_path, "member", "DELETE", json!({}))
            .await
            .0,
        403
    );
    assert_eq!(
        request(
            &app,
            &format!(
                "/servers/{}/artists/{}",
                a.id,
                artist_b["id"].as_str().unwrap()
            ),
            "member",
            "PATCH",
            json!({"notify":true})
        )
        .await
        .0,
        404
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/artists", a.id),
            "member",
            "POST",
            json!({"name":"Denied","url":"https://example.com/denied"})
        )
        .await
        .0,
        403
    );
    assert_eq!(
        request(
            &app,
            &artist_path,
            "member",
            "PATCH",
            json!({"notify":true})
        )
        .await
        .0,
        200
    );
    // Rankings expose aggregates, never the identity or private state of other savers.
    let rankings = format!("/servers/{}/trending", a.id);
    let (status, ranked) = request(&app, &rankings, "manager", "GET", json!({})).await;
    assert_eq!(status, 200);
    assert_eq!(ranked["total"], 1);
    assert_eq!(ranked["items"][0]["id"], shared.to_string());
    assert_eq!(ranked["items"][0]["likes"], 1);
    assert_eq!(ranked["items"][0]["saves"], 1);
    assert_eq!(ranked["items"][0]["favorite"], false);
    assert!(ranked["items"][0].get("user_id").is_none());
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/trending", b.id),
            "manager",
            "GET",
            json!({})
        )
        .await
        .1["total"],
        0
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/trending", b.id),
            "member",
            "GET",
            json!({})
        )
        .await
        .0,
        403
    );
    assert_eq!(
        request(
            &app,
            &format!("{rankings}?period=invalid"),
            "manager",
            "GET",
            json!({})
        )
        .await
        .0,
        400
    );
    let feed: Uuid = artist_a["feed_id"].as_str().unwrap().parse().unwrap();
    let mut historic = Vec::new();
    for days in [15, 200, 400, -1] {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO entries(id,feed_id,guid,title,author,url,published_at) VALUES($1,$2,$3,'Historical','Artist','https://example.com/history',now()-make_interval(days=>$4))")
            .bind(id).bind(feed).bind(id.to_string()).bind(days).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO post_preferences(user_id,community_id,entry_id,liked,favorite) VALUES($1,$2,$3,true,true)")
            .bind(users[0]).bind(a.id).bind(id).execute(&pool).await.unwrap();
        historic.push(id);
    }
    assert_eq!(
        request(&app, &rankings, "manager", "GET", json!({}))
            .await
            .1["total"],
        1
    );
    assert_eq!(
        request(
            &app,
            &format!("{rankings}?period=month"),
            "manager",
            "GET",
            json!({})
        )
        .await
        .1["total"],
        2
    );
    assert_eq!(
        request(
            &app,
            &format!("{rankings}?period=year"),
            "manager",
            "GET",
            json!({})
        )
        .await
        .1["total"],
        3
    );
    // Saves and likes can have different winners; totals are scoped to publication time.
    sqlx::query("INSERT INTO post_preferences(user_id,community_id,entry_id,favorite) VALUES($1,$2,$3,true)")
        .bind(users[1]).bind(a.id).bind(historic[0]).execute(&pool).await.unwrap();
    let saved = request(
        &app,
        &format!("{rankings}?period=month&metric=saves"),
        "manager",
        "GET",
        json!({}),
    )
    .await
    .1;
    assert_eq!(saved["items"][0]["id"], historic[0].to_string());
    assert_eq!(saved["items"][0]["saves"], 2);
    assert_eq!(saved["summary"]["saves"], 3);
    let combined = request(
        &app,
        &format!("{rankings}?period=month&metric=engagement"),
        "manager",
        "GET",
        json!({}),
    )
    .await
    .1;
    assert_eq!(combined["items"][0]["score"], 3);
    let page = trending::posts(
        &s,
        a.id,
        users[0],
        &trending::Filter {
            period: trending::Period::Month,
            metric: trending::Metric::Saves,
            page: 2,
        },
        1,
    )
    .await
    .ok()
    .unwrap();
    assert_eq!(page["items"][0]["rank"], 2);
    assert_eq!(page["pages"], 2);
    assert_eq!(
        request(
            &app,
            &format!("{rankings}?page=999"),
            "manager",
            "GET",
            json!({})
        )
        .await
        .1["items"],
        json!([])
    );
    let profile_path = format!("{artist_path}/profile");
    let profile = json!({"name":"Aoi official","avatar_url":"https://example.com/portrait.png","official_links":[{"label":"Pixiv","url":"https://www.pixiv.net/users/123"}]});
    assert_eq!(
        request(&app, &profile_path, "member", "PUT", profile.clone())
            .await
            .0,
        403
    );
    assert_eq!(
        request(&app, &profile_path, "manager", "PUT", profile.clone())
            .await
            .0,
        200
    );
    let visible = request(
        &app,
        &format!("/servers/{}/artists", a.id),
        "member",
        "GET",
        json!({}),
    )
    .await
    .1;
    assert_eq!(visible[0]["avatar_url"], profile["avatar_url"]);
    assert_eq!(visible[0]["official_links"], profile["official_links"]);
    let unrelated = request(
        &app,
        &format!("/servers/{}/artists", b.id),
        "manager",
        "GET",
        json!({}),
    )
    .await
    .1;
    assert!(
        unrelated
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["avatar_url"] == "")
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/artists/{artist}/profile", b.id),
            "manager",
            "PUT",
            profile
        )
        .await
        .0,
        404
    );
    assert_eq!(
        request(
            &app,
            &profile_path,
            "manager",
            "PUT",
            json!({"name":"Bad","avatar_url":"javascript:alert(1)"})
        )
        .await
        .0,
        400
    );
    for id in historic {
        sqlx::query("DELETE FROM entries WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Valid numeric account IDs are needed to render mentions.
    sqlx::query("UPDATE community_users SET external_id='123' WHERE id=$1")
        .bind(users[1])
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE communities SET channel_id='999'")
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    worker::enqueue(
        &mut tx,
        artist_a["feed_id"].as_str().unwrap().parse().unwrap(),
        shared,
    )
    .await
    .unwrap();
    worker::enqueue(
        &mut tx,
        artist_a["feed_id"].as_str().unwrap().parse().unwrap(),
        shared,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let queued: Vec<(Uuid, Vec<String>)> =
        sqlx::query_as("SELECT community_id,recipient_ids FROM community_deliveries")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(queued.len(), 2);
    assert_eq!(queued.iter().find(|q| q.0 == a.id).unwrap().1, vec!["123"]);
    assert!(queued.iter().find(|q| q.0 == b.id).unwrap().1.is_empty());
    // An opt-out after enqueue must remove that recipient at delivery time.
    sqlx::query("UPDATE artist_preferences SET notify=false WHERE user_id=$1")
        .bind(users[1])
        .execute(&pool)
        .await
        .unwrap();
    assert!(worker::delivery(&s).await.unwrap());
    assert!(worker::delivery(&s).await.unwrap());
    assert!(!worker::delivery(&s).await.unwrap());
    let sent = adapter.sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 2);
    for message in sent {
        assert_eq!(message["allowed_mentions"]["users"], json!([]));
    }
    // A server may use webhooks without configuring a bot channel.
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}", a.id),
            "manager",
            "PUT",
            json!({"channel_id":"","enabled":true,"mentions_enabled":true})
        )
        .await
        .0,
        200
    );
    // Installation endpoints never accept a community login, including a server administrator.
    let owner_app = Router::new()
        .route("/settings", axum::routing::get(crate::admin::settings))
        .route_layer(axum::middleware::from_fn_with_state(
            s.clone(),
            crate::api::auth,
        ))
        .with_state(s.clone());
    assert_eq!(
        request(&owner_app, "/settings", "manager", "GET", json!({}))
            .await
            .0,
        401
    );
    let thread = format!("/servers/{}/posts/{shared}/comments", a.id);
    assert_eq!(
        request(&app, &thread, "revoked", "GET", json!({})).await.0,
        403
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/posts/{private}/comments", a.id),
            "manager",
            "POST",
            json!({"body":"no"})
        )
        .await
        .0,
        404
    );
    assert_eq!(
        request(&app, &thread, "member", "POST", json!({"body":"   "}))
            .await
            .0,
        400
    );
    let (status, comment) = request(
        &app,
        &thread,
        "member",
        "POST",
        json!({"body":"<script>plain text</script>","user_id":users[0]}),
    )
    .await;
    assert_eq!(status, 201);
    let own = request(&app, &thread, "member", "GET", json!({})).await.1;
    assert_eq!(own["items"][0]["author"]["id"], users[1].to_string());
    assert_eq!(own["items"][0]["can_delete"], true);
    let other_thread = format!("/servers/{}/posts/{shared}/comments", b.id);
    assert_eq!(
        request(&app, &other_thread, "manager", "GET", json!({}))
            .await
            .1["total"],
        0
    );
    assert_eq!(
        request(
            &app,
            &format!("{other_thread}/{}", comment["id"].as_str().unwrap()),
            "manager",
            "DELETE",
            json!({})
        )
        .await
        .0,
        404
    );
    assert_eq!(
        request(
            &app,
            &format!("{thread}/{}", comment["id"].as_str().unwrap()),
            "manager",
            "DELETE",
            json!({})
        )
        .await
        .0,
        200
    );
    let (_, admin_comment) = request(
        &app,
        &thread,
        "manager",
        "POST",
        json!({"body":"Moderator"}),
    )
    .await;
    assert_eq!(
        request(
            &app,
            &format!("{thread}/{}", admin_comment["id"].as_str().unwrap()),
            "member",
            "DELETE",
            json!({})
        )
        .await
        .0,
        404
    );
    for _ in 0..5 {
        assert_eq!(
            request(&app, &thread, "member", "POST", json!({"body":"A thought"}))
                .await
                .0,
            201
        );
    }
    assert_eq!(
        request(&app, &thread, "member", "POST", json!({"body":"Too fast"}))
            .await
            .0,
        429
    );
    let hooks = format!("/servers/{}/webhooks", a.id);
    let hook = json!({"name":"Server updates","url":"https://example.com/hook","username":"Atelier","avatar_url":"","embed_color":123,"enabled":true});
    assert_eq!(
        request(&app, &hooks, "member", "GET", json!({})).await.0,
        403
    );
    assert_eq!(
        request(&app, &hooks, "member", "POST", hook.clone())
            .await
            .0,
        403
    );
    let (status, created) = request(&app, &hooks, "manager", "POST", hook.clone()).await;
    assert_eq!(status, 201);
    assert!(created.get("url").is_none());
    let hook_id = created["id"].as_str().unwrap();
    let (status, other_hook) = request(
        &app,
        &format!("/servers/{}/webhooks", b.id),
        "manager",
        "POST",
        hook.clone(),
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(
        request(
            &app,
            &format!(
                "/servers/{}/webhooks/{}",
                b.id,
                other_hook["id"].as_str().unwrap()
            ),
            "manager",
            "DELETE",
            json!({})
        )
        .await
        .0,
        200
    );
    let delivery_path = format!(
        "/servers/{}/artists/{}/delivery",
        a.id,
        artist_a["id"].as_str().unwrap()
    );
    assert_eq!(
        request(
            &app,
            &delivery_path,
            "member",
            "PUT",
            json!({"enabled":false})
        )
        .await
        .0,
        403
    );
    assert_eq!(
        request(
            &app,
            &delivery_path,
            "manager",
            "PUT",
            json!({"enabled":false})
        )
        .await
        .0,
        200
    );
    let enabled: bool = sqlx::query_scalar("SELECT enabled FROM community_artists WHERE id=$1")
        .bind(artist_b["id"].as_str().unwrap().parse::<Uuid>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(enabled);
    assert_eq!(
        request(
            &app,
            &delivery_path,
            "manager",
            "PUT",
            json!({"enabled":true})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/webhooks/{hook_id}", b.id),
            "manager",
            "PUT",
            hook.clone()
        )
        .await
        .0,
        404
    );
    let global_hook = Uuid::new_v4();
    sqlx::query("INSERT INTO webhooks(id,name,url) VALUES($1,'Owner','https://example.com/owner')")
        .bind(global_hook)
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    worker::enqueue(
        &mut tx,
        artist_a["feed_id"].as_str().unwrap().parse().unwrap(),
        shared,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let queued_hooks: Vec<Uuid> = sqlx::query_scalar(
        "SELECT webhook_id FROM jobs WHERE entry_id=$1 AND webhook_id IS NOT NULL",
    )
    .bind(shared)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(queued_hooks, vec![hook_id.parse::<Uuid>().unwrap()]);
    let scoped_hooks = request(&app, &hooks, "manager", "GET", json!({})).await.1;
    assert_eq!(scoped_hooks.as_array().unwrap().len(), 1);
    assert_eq!(scoped_hooks[0]["id"], hook_id);
    assert_eq!(
        request(
            &app,
            &format!("{hooks}/{hook_id}"),
            "manager",
            "DELETE",
            json!({})
        )
        .await
        .0,
        200
    );
    // Transfer and media endpoints are scoped to the community, not the administrator archive.
    let transfer = format!("/servers/{}/feeds/import", a.id);
    let opml = json!({"opml":"<opml><body><outline text=\"Illustration\"><outline text=\"Imported artist\" xmlUrl=\"https://example.com/imported\"/></outline></body></opml>","auto_download":true});
    assert_eq!(
        request(&app, &transfer, "member", "POST", opml.clone())
            .await
            .0,
        403
    );
    assert_eq!(
        request(&app, &transfer, "manager", "POST", opml.clone())
            .await
            .1["imported"],
        1
    );
    assert_eq!(
        request(
            &app,
            &format!("{transfer}/preview"),
            "manager",
            "POST",
            opml.clone()
        )
        .await
        .1["duplicates"],
        1
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/feeds/import/preview", b.id),
            "manager",
            "POST",
            opml.clone()
        )
        .await
        .1["new"],
        1
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/feeds/export", a.id),
            "manager",
            "GET",
            json!({})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/posts/{private}/downloads", a.id),
            "manager",
            "GET",
            json!({})
        )
        .await
        .0,
        404
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/posts/{shared}/downloads", a.id),
            "member",
            "POST",
            json!({})
        )
        .await
        .0,
        403
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/posts/{shared}/downloads", a.id),
            "member",
            "GET",
            json!({})
        )
        .await
        .0,
        200
    );
    // Categories and tags belong to the destination server even for shared feeds.
    let tags_path = format!("/servers/{}/posts/{shared}/tags", a.id);
    let labels = json!({"tags":["Digital Art", "digital_art", "Blue Sky"]});
    assert_eq!(
        request(&app, &tags_path, "member", "PUT", labels.clone())
            .await
            .0,
        403
    );
    let result = request(&app, &tags_path, "manager", "PUT", labels).await;
    assert_eq!(result.0, 200);
    assert_eq!(result.1["tags"], json!(["digital_art", "blue_sky"]));
    assert_eq!(
        request(
            &app,
            &tags_path,
            "manager",
            "PUT",
            json!({"tags":["@everyone"]})
        )
        .await
        .0,
        400
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/posts/{private}/tags", a.id),
            "manager",
            "PUT",
            json!({"tags":[]})
        )
        .await
        .0,
        404
    );
    assert_eq!(
        request(
            &app,
            &format!("{tags_path}/suggest"),
            "member",
            "POST",
            json!({})
        )
        .await
        .0,
        403
    );
    assert_eq!(
        request(
            &app,
            &format!("{tags_path}/suggest"),
            "manager",
            "POST",
            json!({})
        )
        .await
        .0,
        503
    );
    let other = request(
        &app,
        &format!("/servers/{}/posts/{shared}", b.id),
        "manager",
        "GET",
        json!({}),
    )
    .await
    .1;
    assert_eq!(other["post"]["tags"], json!([]));
    assert_eq!(
        request(
            &app,
            &format!("{posts}?tag=digital_art"),
            "member",
            "GET",
            json!({})
        )
        .await
        .1["total"],
        1
    );
    assert_eq!(
        request(
            &app,
            &format!("/servers/{}/posts?tag=digital_art", b.id),
            "manager",
            "GET",
            json!({})
        )
        .await
        .1["total"],
        0
    );
    assert_eq!(
        request(
            &app,
            &profile_path,
            "manager",
            "PUT",
            json!({"name":"Aoi", "category":"3D Art"})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(
            &app,
            &format!("{posts}?category=3D%20Art"),
            "manager",
            "GET",
            json!({})
        )
        .await
        .1["total"],
        1
    );
    assert_eq!(
        request(
            &app,
            &format!("{posts}?category=Photography"),
            "manager",
            "GET",
            json!({})
        )
        .await
        .1["total"],
        0
    );
    // Older profile clients omitting category must preserve it.
    request(&app, &profile_path, "manager", "PUT", json!({"name":"Aoi"})).await;
    let categories_path = format!("/servers/{}/categories", a.id);
    let categories = request(&app, &categories_path, "member", "GET", json!({})).await;
    assert_eq!(categories.0, 200);
    assert!(
        categories
            .1
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["category"] == "3D Art" && c["artists"] == 1)
    );
    assert_eq!(
        request(
            &app,
            &categories_path,
            "member",
            "PUT",
            json!({"category":"3D Art","channel_id":""})
        )
        .await
        .0,
        403
    );
    sqlx::query("INSERT INTO community_category_routes(community_id,category,channel_id,channel_name) VALUES($1,'3D Art','888','3d-art')").bind(a.id).execute(&pool).await.unwrap();
    let routed = Uuid::new_v4();
    let feed_id: Uuid = artist_a["feed_id"].as_str().unwrap().parse().unwrap();
    sqlx::query("INSERT INTO entries(id,feed_id,guid,title,author,url,published_at) VALUES($1,$2,$1::text,'Routed post','Aoi','https://example.com/routed',now())").bind(routed).bind(feed_id).execute(&pool).await.unwrap();
    let mut hook_ids = Vec::new();
    for (name, category) in [
        ("All", None),
        ("3D", Some("3D Art")),
        ("Photo", Some("Photography")),
    ] {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO webhooks(id,community_id,name,url,category_filter) VALUES($1,$2,$3,$4,$5)",
        )
        .bind(id)
        .bind(a.id)
        .bind(name)
        .bind(format!("https://example.com/{id}"))
        .bind(category)
        .execute(&pool)
        .await
        .unwrap();
        hook_ids.push(id);
    }
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("UPDATE communities SET ai_tags_enabled=true WHERE id=$1")
        .bind(a.id)
        .execute(&mut *tx)
        .await
        .unwrap();
    worker::enqueue(&mut tx, feed_id, routed).await.unwrap();
    worker::enqueue(&mut tx, feed_id, routed).await.unwrap();
    tx.commit().await.unwrap();
    let destinations: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT community_id,channel_id FROM community_deliveries WHERE entry_id=$1",
    )
    .bind(routed)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(destinations.len(), 2);
    assert!(destinations.contains(&(a.id, "888".into()))); // Works with no default channel.
    assert!(destinations.contains(&(b.id, "999".into()))); // Other server uses its default.
    let hooks: Vec<Uuid> = sqlx::query_scalar("SELECT webhook_id FROM jobs WHERE entry_id=$1")
        .bind(routed)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(hooks.len(), 2);
    assert!(hooks.contains(&hook_ids[0]) && hooks.contains(&hook_ids[1]));
    assert!(!hooks.contains(&hook_ids[2]));
    let replay = request(
        &app,
        &format!("/servers/{}/posts/{routed}/webhooks", a.id),
        "manager",
        "POST",
        json!({}),
    )
    .await;
    assert_eq!(replay.0, 200);
    assert_eq!(replay.1["queued"], 0); // Existing jobs are not duplicated.
    let mut payload = json!({"embeds":[{"fields":[]}]});
    tags::decorate(&s, a.id, shared, &mut payload)
        .await
        .unwrap();
    assert_eq!(payload["embeds"][0]["fields"][0]["value"], "3D Art");
    assert!(
        payload["embeds"][0]["fields"][1]["value"]
            .as_str()
            .unwrap()
            .contains("#digital_art")
    );
    assert_eq!(
        request(
            &app,
            &categories_path,
            "manager",
            "PUT",
            json!({"category":"3D Art","channel_id":""})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT channel_id FROM community_deliveries WHERE entry_id=$1 AND community_id=$2"
        )
        .bind(routed)
        .bind(a.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "888"
    ); // Snapshot survives route edits.
    // Exercise the automatic queue against a local provider, never a real model.
    let provider = Router::new()
        .route(
            "/v1/tag",
            axum::routing::post(|body: axum::body::Bytes| async move {
                assert_eq!(&body[..], b"image-fixture");
                axum::Json(json!({"tags":[{"name":"digital_art","score":0.9}]}))
            }),
        )
        .route(
            "/fail",
            axum::routing::post(|| async { axum::http::StatusCode::SERVICE_UNAVAILABLE }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let provider_task = tokio::spawn(async move {
        axum::serve(listener, provider).await.unwrap();
    });
    let mut ai_state = s.clone();
    ai_state.tagger = Some(Arc::new(
        crate::ai::Tagger::new(format!("{endpoint}/v1/tag"))
            .unwrap()
            .with_fixture("https://fixtures.invalid/image.png", b"image-fixture"),
    ));
    sqlx::query("UPDATE entries SET media=$2 WHERE id=$1")
        .bind(routed)
        .bind(json!([{"kind":"image","url":"https://fixtures.invalid/image.png"}]))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM community_tag_jobs WHERE entry_id=$1")
            .bind(routed)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert!(worker::delivery(&ai_state).await.unwrap()); // The other server does not wait for AI.
    assert!(!worker::delivery(&ai_state).await.unwrap()); // This server's job waits for tags.
    tag_worker::tick(&ai_state).await.unwrap();
    let generated: (Vec<String>, String) = sqlx::query_as(
        "SELECT tags,source FROM community_post_tags WHERE community_id=$1 AND entry_id=$2",
    )
    .bind(a.id)
    .bind(routed)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(generated, (vec!["digital_art".into()], "ai".into()));
    assert!(worker::delivery(&ai_state).await.unwrap());
    // Saving an empty list protects the human's decision against a late AI result.
    let routed_tags = format!("/servers/{}/posts/{routed}/tags", a.id);
    assert_eq!(
        request(&app, &routed_tags, "manager", "PUT", json!({"tags":[]}))
            .await
            .0,
        200
    );
    sqlx::query(
        "UPDATE community_tag_jobs SET status='processing' WHERE community_id=$1 AND entry_id=$2",
    )
    .bind(a.id)
    .bind(routed)
    .execute(&pool)
    .await
    .unwrap();
    tag_worker::store(
        &s,
        a.id,
        routed,
        artist.parse().unwrap(),
        &["unwanted".into()],
    )
    .await
    .unwrap();
    let manual: (Vec<String>, String) = sqlx::query_as(
        "SELECT tags,source FROM community_post_tags WHERE community_id=$1 AND entry_id=$2",
    )
    .bind(a.id)
    .bind(routed)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(manual, (vec![], "manual".into()));
    // Three provider failures end the wait and preserve the edited metadata.
    ai_state.tagger = Some(Arc::new(
        crate::ai::Tagger::new(format!("{endpoint}/fail"))
            .unwrap()
            .with_fixture("https://fixtures.invalid/image.png", b"image-fixture"),
    ));
    sqlx::query("UPDATE community_tag_jobs SET status='pending',attempts=2,next_attempt_at=now() WHERE community_id=$1 AND entry_id=$2").bind(a.id).bind(routed).execute(&pool).await.unwrap();
    tag_worker::tick(&ai_state).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM community_tag_jobs WHERE community_id=$1 AND entry_id=$2"
        )
        .bind(a.id)
        .bind(routed)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "failed"
    );
    // Text-only posts finish without contacting even an unavailable provider.
    sqlx::query("UPDATE entries SET media='[]'::jsonb WHERE id=$1")
        .bind(routed)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE community_tag_jobs SET status='pending',attempts=0,next_attempt_at=now() WHERE community_id=$1 AND entry_id=$2").bind(a.id).bind(routed).execute(&pool).await.unwrap();
    tag_worker::tick(&ai_state).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM community_tag_jobs WHERE community_id=$1 AND entry_id=$2"
        )
        .bind(a.id)
        .bind(routed)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "done"
    );
    provider_task.abort();
    sqlx::query("DELETE FROM entries WHERE id=$1")
        .bind(routed)
        .execute(&pool)
        .await
        .unwrap();

    // A removed artist cannot retain a publicly counted preference or tags on re-add.
    assert_eq!(
        request(&app, &artist_path, "manager", "DELETE", json!({}))
            .await
            .0,
        200
    );
    assert_eq!(
        request(&app, &posts, "manager", "GET", json!({})).await.1["total"],
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM community_post_tags WHERE community_id=$1"
        )
        .bind(a.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
