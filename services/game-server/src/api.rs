use crate::{
    manager::RoomManager,
    model::{AuthenticatedRole, ClientEvent, ErrorCode, GameError, ServerEvent},
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::{HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::Level;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub rooms: RoomManager,
}

pub fn app(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([axum::http::header::CONTENT_TYPE])
        .allow_origin(AllowOrigin::predicate(|origin, _| is_local_origin(origin)));
    Router::new()
        .route(
            "/health",
            get(|| async { Json(serde_json::json!({"status": "ok"})) }),
        )
        .route("/api/games", get(game_catalog))
        .route("/api/rooms", post(create_room))
        .route("/api/rooms/{code}/join", post(join_room))
        .route("/api/rooms/{code}", get(get_room))
        .route("/api/ws", get(websocket))
        .layer(cors)
        .layer(
            TraceLayer::new_for_http().make_span_with(
                DefaultMakeSpan::new()
                    .level(Level::INFO)
                    .include_headers(false),
            ),
        )
        .with_state(Arc::new(state))
}

fn is_local_origin(origin: &HeaderValue) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    if std::env::var("SYNCUP_ALLOWED_ORIGINS")
        .ok()
        .is_some_and(|configured| {
            configured
                .split(',')
                .map(str::trim)
                .any(|allowed| allowed == origin)
        })
    {
        return true;
    }
    if std::env::var("SYNCUP_ENV").is_ok_and(|value| value == "production") {
        return false;
    }
    if matches!(
        origin,
        "http://localhost:1420"
            | "http://localhost:5173"
            | "http://localhost:5174"
            | "http://tauri.localhost"
            | "https://tauri.localhost"
            | "tauri://localhost"
    ) {
        return true;
    }
    let Ok(uri) = origin.parse::<Uri>() else {
        return false;
    };
    if uri.scheme_str() != Some("http") {
        return false;
    }
    let Some(host) = uri.host() else {
        return false;
    };
    let Ok(address) = host.parse::<std::net::Ipv4Addr>() else {
        return false;
    };
    address.is_private() || address.is_loopback()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateRoomResponse {
    room_code: String,
    host_token: String,
    game_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRoomRequest {
    #[serde(default = "default_game_id")]
    game_id: String,
    rounds: Option<u8>,
    theme: Option<String>,
    #[serde(default = "default_host_nickname")]
    host_nickname: String,
}

impl Default for CreateRoomRequest {
    fn default() -> Self {
        Self {
            game_id: default_game_id(),
            rounds: None,
            theme: None,
            host_nickname: default_host_nickname(),
        }
    }
}

fn default_game_id() -> String {
    "priority-sync".into()
}

fn default_host_nickname() -> String {
    "Afreen".into()
}

async fn game_catalog(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<crate::catalog::GameSummary>> {
    Json(state.rooms.game_catalog())
}

async fn create_room(
    State(state): State<Arc<AppState>>,
    request: Option<Json<CreateRoomRequest>>,
) -> Result<Json<CreateRoomResponse>, ApiError> {
    let request = request.map(|Json(request)| request).unwrap_or_default();
    let (room_code, host_token) = state
        .rooms
        .create_room_with_host_options(
            &request.game_id,
            request.rounds,
            request.theme.as_deref(),
            &request.host_nickname,
        )
        .await?;
    Ok(Json(CreateRoomResponse {
        room_code,
        host_token,
        game_id: request.game_id,
    }))
}

#[derive(Deserialize)]
struct JoinRoomRequest {
    nickname: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JoinRoomResponse {
    room_code: String,
    player_id: Uuid,
    session_token: String,
}

async fn join_room(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
    Json(request): Json<JoinRoomRequest>,
) -> Result<Json<JoinRoomResponse>, ApiError> {
    let normalized_code = code.trim().to_ascii_uppercase();
    let (player_id, session_token) = state
        .rooms
        .join_room(&normalized_code, &request.nickname)
        .await?;
    Ok(Json(JoinRoomResponse {
        room_code: normalized_code,
        player_id,
        session_token,
    }))
}

async fn get_room(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
) -> Result<Json<crate::model::RoomSnapshot>, ApiError> {
    Ok(Json(state.rooms.public_snapshot(&code).await?))
}

async fn websocket(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let connection_id = Uuid::new_v4();
    let auth_message = match tokio::time::timeout(Duration::from_secs(10), socket.recv()).await {
        Ok(Some(Ok(Message::Text(text)))) => text,
        _ => {
            send_event(
                &mut socket,
                &GameError::new(ErrorCode::Unauthorized, "Authenticate within 10 seconds.").into(),
            )
            .await;
            return;
        }
    };
    let event: ClientEvent = match serde_json::from_str(&auth_message) {
        Ok(event) => event,
        Err(_) => {
            send_event(
                &mut socket,
                &GameError::new(ErrorCode::Unauthorized, "Invalid authentication message.").into(),
            )
            .await;
            return;
        }
    };

    let (code, role, initial_snapshot) = match event {
        ClientEvent::AuthenticateHost {
            room_code,
            host_token,
        } => {
            let code = room_code.trim().to_ascii_uppercase();
            match state
                .rooms
                .authenticate_host(&code, &host_token, connection_id)
                .await
            {
                Ok(snapshot) => {
                    let player_id = snapshot.host_player_id;
                    (
                        code,
                        AuthenticatedRole::Host {
                            host_token,
                            player_id,
                        },
                        snapshot,
                    )
                }
                Err(error) => {
                    send_event(&mut socket, &error.into()).await;
                    return;
                }
            }
        }
        ClientEvent::AuthenticatePlayer {
            room_code,
            player_id,
            session_token,
        } => {
            let code = room_code.trim().to_ascii_uppercase();
            match state
                .rooms
                .authenticate_player(&code, player_id, &session_token, connection_id)
                .await
            {
                Ok(snapshot) => (code, AuthenticatedRole::Player { player_id }, snapshot),
                Err(error) => {
                    send_event(&mut socket, &error.into()).await;
                    return;
                }
            }
        }
        _ => {
            send_event(
                &mut socket,
                &GameError::new(ErrorCode::Unauthorized, "First message must authenticate.").into(),
            )
            .await;
            return;
        }
    };

    let mut updates = match state.rooms.subscribe(&code).await {
        Ok(updates) => updates,
        Err(error) => {
            send_event(&mut socket, &error.into()).await;
            return;
        }
    };
    let role_name = match &role {
        AuthenticatedRole::Host { .. } => "host",
        AuthenticatedRole::Player { .. } => "player",
    };
    send_event(
        &mut socket,
        &ServerEvent::Authenticated {
            role: role_name.to_owned(),
        },
    )
    .await;
    send_event(
        &mut socket,
        &ServerEvent::RoomSnapshot {
            snapshot: Box::new(initial_snapshot),
        },
    )
    .await;

    let (mut sender, mut receiver) = socket.split();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(25));
    loop {
        tokio::select! {
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        let response = match serde_json::from_str::<ClientEvent>(&text) {
                            Ok(event) => process_event(&state.rooms, &code, &role, event).await,
                            Err(_) => Some(GameError::new(ErrorCode::InternalError, "Invalid message format.").into()),
                        };
                        if let Some(event) = response {
                            if send_to_sink(&mut sender, &event).await.is_err() { break; }
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if sender.send(Message::Pong(payload)).await.is_err() { break; }
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(_)) => {}
                }
            }
            update = updates.recv() => {
                match update {
                    Ok(mut snapshot) => {
                        let player_id = match &role {
                            AuthenticatedRole::Player { player_id }
                            | AuthenticatedRole::Host { player_id, .. } => *player_id,
                        };
                        if let Ok(viewer_snapshot) = state.rooms.snapshot_for_player(&code, player_id).await {
                            snapshot.viewer_has_answered = viewer_snapshot.viewer_has_answered;
                            snapshot.viewer_answer = viewer_snapshot.viewer_answer;
                            snapshot.viewer_player_id = viewer_snapshot.viewer_player_id;
                            snapshot.viewer_has_completed = viewer_snapshot.viewer_has_completed;
                            snapshot.viewer_truth_or_dare_choice = viewer_snapshot.viewer_truth_or_dare_choice;
                            snapshot.viewer_truth_or_dare_prompt_id = viewer_snapshot.viewer_truth_or_dare_prompt_id;
                            snapshot.viewer_truth_or_dare_prompt = viewer_snapshot.viewer_truth_or_dare_prompt;
                            snapshot.viewer_reroll_available = viewer_snapshot.viewer_reroll_available;
                            snapshot.viewer_highlight_votes = viewer_snapshot.viewer_highlight_votes;
                            snapshot.viewer_skipped_highlight_voting = viewer_snapshot.viewer_skipped_highlight_voting;
                        }
                        if send_to_sink(&mut sender, &ServerEvent::RoomSnapshot { snapshot: Box::new(snapshot) }).await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        if let Ok(snapshot) = state.rooms.snapshot_for_role(&code, &role).await {
                            if send_to_sink(&mut sender, &ServerEvent::RoomSnapshot { snapshot: Box::new(snapshot) }).await.is_err() { break; }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = heartbeat.tick() => {
                if sender.send(Message::Ping(Vec::new().into())).await.is_err() { break; }
            }
        }
    }

    match role {
        AuthenticatedRole::Host { .. } => state.rooms.disconnect_host(&code, connection_id).await,
        AuthenticatedRole::Player { player_id } => {
            state
                .rooms
                .disconnect_player(&code, player_id, connection_id)
                .await
        }
    }
}

async fn process_event(
    rooms: &RoomManager,
    code: &str,
    role: &AuthenticatedRole,
    event: ClientEvent,
) -> Option<ServerEvent> {
    let actor_id = match role {
        AuthenticatedRole::Host { player_id, .. } | AuthenticatedRole::Player { player_id } => {
            *player_id
        }
    };
    let result = match (role, event) {
        (_, ClientEvent::Ping) => return Some(ServerEvent::Pong),
        (_, ClientEvent::StartGame) => rooms.start_game_by_player(code, actor_id).await,
        (_, ClientEvent::RevealResults) => rooms.reveal_results_by_player(code, actor_id).await,
        (_, ClientEvent::NextRound) => rooms.next_round_by_player(code, actor_id).await,
        (_, ClientEvent::RerollSpotlight { round }) => {
            rooms.reroll_spotlight(code, actor_id, round).await
        }
        (_, ClientEvent::SkipSpotlight { round }) => {
            rooms.skip_spotlight(code, actor_id, round).await
        }
        (_, ClientEvent::ForceSkipSpotlight { round }) => {
            rooms.force_skip_spotlight(code, actor_id, round).await
        }
        (_, ClientEvent::NextSpotlight { round }) => {
            rooms.next_spotlight(code, actor_id, round).await
        }
        (
            _,
            ClientEvent::SendReaction {
                round,
                emoji,
                reaction_id,
            },
        ) => {
            rooms
                .send_reaction(code, actor_id, round, &emoji, reaction_id)
                .await
        }
        (
            _,
            ClientEvent::SubmitHighlightVote {
                round,
                category_id,
                selected_player_id,
            },
        ) => {
            rooms
                .submit_highlight_vote(code, actor_id, round, &category_id, selected_player_id)
                .await
        }
        (_, ClientEvent::SkipHighlightVoting { round }) => {
            rooms.skip_highlight_voting(code, actor_id, round).await
        }
        (_, ClientEvent::RevealHighlights { round }) => {
            rooms.reveal_highlights(code, actor_id, round).await
        }
        (_, ClientEvent::KickPlayer { player_id }) => {
            rooms.kick_player_by_player(code, actor_id, player_id).await
        }
        (AuthenticatedRole::Host { host_token, .. }, ClientEvent::EndGame) => {
            rooms.end_game(code, host_token).await
        }
        (_, ClientEvent::LeaveRoom) => rooms.leave_room(code, actor_id).await,
        (_, ClientEvent::ChooseAnotherGame) => rooms.choose_another_game(code, actor_id).await,
        (
            _,
            ClientEvent::ConfigureGame {
                game_id,
                rounds,
                theme,
            },
        ) => {
            rooms
                .configure_game(code, actor_id, &game_id, rounds, &theme)
                .await
        }
        (_, ClientEvent::EndRoom) => rooms.end_room_by_player(code, actor_id).await,
        (
            AuthenticatedRole::Host { player_id, .. },
            ClientEvent::SubmitAnswer { round, answer },
        ) => rooms.submit_answer(code, *player_id, round, &answer).await,
        (AuthenticatedRole::Host { player_id, .. }, ClientEvent::MarkCompleted { round }) => {
            rooms.mark_completed(code, *player_id, round).await
        }
        (
            AuthenticatedRole::Host { player_id, .. },
            ClientEvent::SubmitVote {
                round,
                selected_player_id,
            },
        ) => {
            rooms
                .submit_vote(code, *player_id, round, selected_player_id)
                .await
        }
        (AuthenticatedRole::Player { player_id }, ClientEvent::SubmitAnswer { round, answer }) => {
            rooms.submit_answer(code, *player_id, round, &answer).await
        }
        (AuthenticatedRole::Player { player_id }, ClientEvent::MarkCompleted { round }) => {
            rooms.mark_completed(code, *player_id, round).await
        }
        (
            AuthenticatedRole::Player { player_id },
            ClientEvent::SubmitVote {
                round,
                selected_player_id,
            },
        ) => {
            rooms
                .submit_vote(code, *player_id, round, selected_player_id)
                .await
        }
        (_, ClientEvent::AuthenticateHost { .. } | ClientEvent::AuthenticatePlayer { .. }) => {
            Err(GameError::new(
                ErrorCode::Unauthorized,
                "Connection is already authenticated.",
            ))
        }
        _ => Err(GameError::new(
            ErrorCode::Unauthorized,
            "This role cannot perform that action.",
        )),
    };
    result.err().map(Into::into)
}

async fn send_event(socket: &mut WebSocket, event: &ServerEvent) {
    if let Ok(json) = serde_json::to_string(event) {
        let _ = socket.send(Message::Text(json.into())).await;
    }
}

async fn send_to_sink<S>(sink: &mut S, event: &ServerEvent) -> Result<(), ()>
where
    S: futures_util::Sink<Message> + Unpin,
{
    let json = serde_json::to_string(event).map_err(|_| ())?;
    sink.send(Message::Text(json.into())).await.map_err(|_| ())
}

struct ApiError(GameError);

impl From<GameError> for ApiError {
    fn from(error: GameError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0.code {
            ErrorCode::RoomNotFound | ErrorCode::GameNotFound => StatusCode::NOT_FOUND,
            ErrorCode::RoomFull | ErrorCode::RoomLocked => StatusCode::CONFLICT,
            ErrorCode::InvalidNickname
            | ErrorCode::InvalidPlayer
            | ErrorCode::InvalidAnswer
            | ErrorCode::RoundMismatch => StatusCode::BAD_REQUEST,
            ErrorCode::InvalidToken | ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        };
        (status, Json(ServerEvent::from(self.0))).into_response()
    }
}
