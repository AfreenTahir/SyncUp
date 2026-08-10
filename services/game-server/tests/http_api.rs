use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use syncup_game_server::{app, AppState, RoomManager};
use tower::ServiceExt;

fn test_app() -> axum::Router {
    app(AppState {
        rooms: RoomManager::new(vec!["Who is ready?".into()]),
    })
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn health_create_join_and_snapshot_routes_work_without_leaking_tokens() {
    let health = test_app()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);

    let create = test_app()
        .oneshot(Request::post("/api/rooms").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);
    let created = json_body(create).await;
    let code = created["roomCode"].as_str().unwrap();
    assert_eq!(code.len(), 6);
    assert!(created["hostToken"].as_str().unwrap().len() >= 32);

    let app = test_app();
    let create = app
        .clone()
        .oneshot(Request::post("/api/rooms").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let created = json_body(create).await;
    let code = created["roomCode"].as_str().unwrap();
    let join = app
        .clone()
        .oneshot(
            Request::post(format!("/api/rooms/{code}/join"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"nickname": "  Afreen  "}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(join.status(), StatusCode::OK);
    let joined = json_body(join).await;
    assert_eq!(joined["roomCode"], code);
    assert!(joined["sessionToken"].as_str().unwrap().len() >= 32);

    let snapshot = app
        .oneshot(
            Request::get(format!("/api/rooms/{code}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let snapshot_text = String::from_utf8(
        snapshot
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(snapshot_text.contains("Afreen"));
    assert!(!snapshot_text.contains("hostToken"));
    assert!(!snapshot_text.contains("sessionToken"));
    assert!(!snapshot_text.contains(joined["sessionToken"].as_str().unwrap()));
}

#[tokio::test]
async fn invalid_join_returns_structured_error() {
    let response = test_app()
        .oneshot(
            Request::post("/api/rooms/ABC234/join")
                .header("content-type", "application/json")
                .body(Body::from(json!({"nickname": "Afreen"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = json_body(response).await;
    assert_eq!(body["type"], "error");
    assert_eq!(body["code"], "ROOM_NOT_FOUND");
}

#[tokio::test]
async fn catalog_and_selected_game_room_creation_work() {
    let app = app(AppState {
        rooms: RoomManager::with_catalog(syncup_game_server::catalog::load_default_catalog()),
    });
    let catalog = app
        .clone()
        .oneshot(Request::get("/api/games").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    let catalog = json_body(catalog).await;
    assert!(catalog.as_array().unwrap().len() >= 10);
    assert!(catalog[0]["category"].is_string());
    assert!(catalog[0]["estimatedMinutes"].as_u64().unwrap() > 0);

    let create = app
        .clone()
        .oneshot(
            Request::post("/api/rooms")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"gameId": "guess-the-emoji", "rounds": 6, "theme": "ocean-blue"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);
    let created = json_body(create).await;
    let code = created["roomCode"].as_str().unwrap();
    let room = app
        .oneshot(
            Request::get(format!("/api/rooms/{code}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let room = json_body(room).await;
    assert_eq!(room["gameId"], "guess-the-emoji");
    assert_eq!(room["responseMode"], "quiz");
    assert_eq!(room["maxRounds"], 6);
    assert_eq!(room["theme"], "ocean-blue");
}

#[test]
fn this_or_that_catalog_contains_original_visual_cards() {
    let catalog = syncup_game_server::catalog::load_default_catalog();
    let game = catalog
        .iter()
        .find(|game| game.id == "this-or-that")
        .unwrap();
    assert!(
        game.questions
            .iter()
            .filter(|question| question.visual_options.len() == 2)
            .count()
            >= 4
    );
}
