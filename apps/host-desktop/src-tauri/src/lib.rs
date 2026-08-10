use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use std::{net::UdpSocket, time::Duration};
use syncup_game_server::{app, catalog::load_default_catalog, AppState, RoomManager};
use tauri::State;

include!(concat!(env!("OUT_DIR"), "/player_assets.rs"));

#[derive(Clone)]
struct LocalNetworkInfo {
    player_url: String,
}

#[tauri::command]
fn local_player_url(info: State<'_, LocalNetworkInfo>) -> String {
    info.player_url.clone()
}

fn local_ipv4() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("1.1.1.1:80")?;
            socket.local_addr()
        })
        .map(|address| address.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}

async fn player_web(uri: Uri) -> Response {
    let requested = uri.path().trim_start_matches('/');
    if requested.starts_with("api/") || requested == "health" {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = if requested.is_empty() {
        "index.html"
    } else {
        requested
    };
    let asset = embedded_player_asset(path).or_else(|| {
        (!path.contains('.'))
            .then(|| embedded_player_asset("index.html"))
            .flatten()
    });
    let Some((bytes, content_type)) = asset else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut response = Body::from(bytes).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response
}

async fn run_local_server() {
    let rooms = RoomManager::with_catalog(load_default_catalog());
    let cleanup_rooms = rooms.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            cleanup_rooms.cleanup_once().await;
        }
    });
    let router = app(AppState { rooms }).fallback(player_web);
    match tokio::net::TcpListener::bind("0.0.0.0:3000").await {
        Ok(listener) => {
            if let Err(error) = axum::serve(listener, router).await {
                eprintln!("SyncUp local server stopped: {error}");
            }
        }
        Err(error) => eprintln!("SyncUp could not start the local server: {error}"),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let player_url = format!("http://{}:3000", local_ipv4());
    tauri::Builder::default()
        .manage(LocalNetworkInfo { player_url })
        .invoke_handler(tauri::generate_handler![local_player_url])
        .setup(|_| {
            std::thread::spawn(|| {
                let runtime = tokio::runtime::Runtime::new().expect("create SyncUp runtime");
                runtime.block_on(run_local_server());
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running SyncUp host");
}
