use std::{net::SocketAddr, time::Duration};
use syncup_game_server::{app, catalog::load_default_catalog, AppState, RoomManager};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "syncup_game_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let rooms = RoomManager::with_catalog(load_default_catalog());
    let cleanup_rooms = rooms.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            cleanup_rooms.cleanup_once().await;
        }
    });

    let bind_address = std::env::var("SYNCUP_BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let address: SocketAddr = bind_address
        .parse()
        .expect("SYNCUP_BIND must be a valid socket address");
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("failed to bind server");
    tracing::info!(%address, "SyncUp game server listening");
    axum::serve(listener, app(AppState { rooms }))
        .await
        .expect("server failed");
}
