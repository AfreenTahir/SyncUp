use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use syncup_game_server::{app, AppState, RoomManager};
use tokio_tungstenite::{connect_async, tungstenite::Message};

async fn running_server(manager: RoomManager) -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app(AppState { rooms: manager }))
            .await
            .unwrap();
    });
    address
}

async fn next_json<S>(socket: &mut tokio_tungstenite::WebSocketStream<S>) -> Value
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    loop {
        match socket.next().await.unwrap().unwrap() {
            Message::Text(text) => return serde_json::from_str(&text).unwrap(),
            Message::Ping(payload) => socket.send(Message::Pong(payload)).await.unwrap(),
            _ => {}
        }
    }
}

#[tokio::test]
async fn websocket_rejects_commands_before_authentication() {
    let address = running_server(RoomManager::new(vec!["Question?".into()])).await;
    let (mut socket, _) = connect_async(format!("ws://{address}/api/ws"))
        .await
        .unwrap();
    socket
        .send(Message::Text(
            json!({"type": "start_game"}).to_string().into(),
        ))
        .await
        .unwrap();
    let error = next_json(&mut socket).await;
    assert_eq!(error["type"], "error");
    assert_eq!(error["code"], "UNAUTHORIZED");
}

#[tokio::test]
async fn authenticated_host_and_player_receive_safe_lobby_snapshots() {
    let manager = RoomManager::new(vec!["Question?".into()]);
    let (code, host_token) = manager.create_room().await;
    let (player_id, player_token) = manager.join_room(&code, "Afreen").await.unwrap();
    let address = running_server(manager).await;

    let (mut host, _) = connect_async(format!("ws://{address}/api/ws"))
        .await
        .unwrap();
    host.send(Message::Text(
        json!({"type": "authenticate_host", "roomCode": code, "hostToken": host_token})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    assert_eq!(next_json(&mut host).await["type"], "authenticated");
    let host_snapshot = next_json(&mut host).await;
    assert_eq!(host_snapshot["type"], "room_snapshot");
    let serialized = host_snapshot.to_string();
    assert!(!serialized.contains("hostToken"));
    assert!(!serialized.contains("sessionToken"));

    let (mut player, _) = connect_async(format!("ws://{address}/api/ws"))
        .await
        .unwrap();
    player
        .send(Message::Text(
            json!({
                "type": "authenticate_player",
                "roomCode": code,
                "playerId": player_id,
                "sessionToken": player_token
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
    assert_eq!(next_json(&mut player).await["type"], "authenticated");
    let player_snapshot = next_json(&mut player).await;
    assert_eq!(
        player_snapshot["snapshot"]["players"][0]["nickname"],
        "Afreen"
    );
    assert_eq!(player_snapshot["snapshot"]["viewerHasAnswered"], false);
    let serialized = player_snapshot.to_string();
    assert!(!serialized.contains("hostToken"));
    assert!(!serialized.contains("sessionToken"));
}

#[tokio::test]
async fn authenticated_player_cannot_overwrite_another_truth_or_dare_choice() {
    let manager = RoomManager::with_catalog(syncup_game_server::catalog::load_default_catalog());
    let (code, host_token) = manager
        .create_room_with_host_options("truth-or-dare", Some(2), None, "Host")
        .await
        .unwrap();
    manager
        .authenticate_host(&code, &host_token, uuid::Uuid::new_v4())
        .await
        .unwrap();
    let host = manager.public_snapshot(&code).await.unwrap().host_player_id;
    let (afreen, afreen_token) = manager.join_room(&code, "Afreen").await.unwrap();
    let (sam, _) = manager.join_room(&code, "Sam").await.unwrap();
    manager.start_game_by_player(&code, host).await.unwrap();
    let address = running_server(manager.clone()).await;
    let (mut player, _) = connect_async(format!("ws://{address}/api/ws"))
        .await
        .unwrap();
    player
        .send(Message::Text(
            json!({
                "type": "authenticate_player",
                "roomCode": code,
                "playerId": afreen,
                "sessionToken": afreen_token
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
    assert_eq!(next_json(&mut player).await["type"], "authenticated");
    let _ = next_json(&mut player).await;
    player
        .send(Message::Text(
            json!({
                "type": "submit_answer",
                "round": 1,
                "answer": "dare",
                "playerId": sam
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
    let _ = next_json(&mut player).await;
    let afreen_snapshot = manager.snapshot_for_player(&code, afreen).await.unwrap();
    let sam_snapshot = manager.snapshot_for_player(&code, sam).await.unwrap();
    assert_eq!(
        afreen_snapshot.viewer_truth_or_dare_choice.as_deref(),
        Some("dare")
    );
    assert!(!sam_snapshot.viewer_has_answered.unwrap());
    assert!(sam_snapshot.viewer_truth_or_dare_choice.is_none());
}
