# SyncUp

SyncUp is a server-authoritative multiplayer party platform for up to 12 people. A host creates a private six-character room, friends join from any browser, and authenticated WebSockets keep the party synchronized. The desktop host is also a normal participating player.

The game library includes Priority Sync, Would You Rather, Most Likely To, Never Have I Ever, Truth or Dare, This or That, Guess the Emoji, Two Truths & a Lie, Rapid Fire Quiz, and Word Association.

## What changed

- Production clients use configurable public HTTPS and WSS endpoints; no production fallback uses a private IP address.
- Refreshes and short network interruptions restore the same server-generated player identity.
- The server remains authoritative for room membership, host permissions, rounds, answers, Spotlight order, reactions, highlights, scores, and lifecycle.
- Truth or Dare is now a shared Spotlight Reveal: private independent choices, one public prompt at a time, one reroll per player per match, safe skipping, live reactions, and casual highlight awards.

Rooms remain in memory and intentionally use no PostgreSQL. A production restart clears active rooms.

## Local setup

Requirements: Rust stable, Node.js 20+, npm 10+, and the Tauri 2 prerequisites only when building the native desktop app.

From the project root:

```powershell
npm.cmd install
npm.cmd run dev:server
```

In two more terminals:

```powershell
npm.cmd run dev:player
npm.cmd run dev:host:web
```

Open:

- Host: http://127.0.0.1:1420
- Player: http://127.0.0.1:5173

## Production architecture

Deploy three public components:

1. The Rust game server from `Dockerfile.server`.
2. The player web app from `Dockerfile.web` with build argument `APP=player-web`.
3. The host web app from `Dockerfile.web` with build argument `APP=host-desktop`, or distribute the Tauri build configured with the same public addresses.

Typical public addresses:

```text
https://play.example.com
https://host.example.com
https://api.example.com
wss://api.example.com/api/ws
```

The hosting platform must terminate valid TLS and proxy WebSocket upgrades. Do not use `localhost`, `192.168.x.x`, plain HTTP, or plain WS in production.

## Environment variables

Game server:

| Variable | Required in production | Purpose |
| --- | --- | --- |
| `SYNCUP_BIND` | Yes | Listen address, normally `0.0.0.0:3000` |
| `SYNCUP_ENV` | Yes | Set to `production` to disable development-origin allowances |
| `SYNCUP_ALLOWED_ORIGINS` | Yes | Exact comma-separated HTTPS host/player origins |
| `SYNCUP_PUBLIC_URL` | Recommended | Public HTTPS API address for operations and monitoring |
| `RUST_LOG` | No | Logging filter; request headers and tokens are not logged |

Player build:

| Variable | Required | Purpose |
| --- | --- | --- |
| `VITE_API_URL` | When API is cross-origin | Public HTTPS game-server address |
| `VITE_WS_URL` | When API is cross-origin | Public WSS gameplay endpoint |

Host build:

| Variable | Required | Purpose |
| --- | --- | --- |
| `VITE_API_URL` | Yes for Tauri/cross-origin | Public HTTPS game-server address |
| `VITE_WS_URL` | Yes for Tauri/cross-origin | Public WSS gameplay endpoint |
| `VITE_PLAYER_WEB_URL` | Yes | Public player invitation address |

## Truth or Dare flow

```text
choosing → preparing_reveal → spotlight → highlight_voting
         → highlight_results → next round / final celebration
```

Every player independently locks Truth or Dare. Once everyone is ready, the server shuffles every active player into a stable order. During each Spotlight, all devices see the same player, choice, and prompt. Only that player can complete, reroll, or skip. The host can advance or end the game but cannot answer for someone else.

Reactions use client-generated unique IDs and are deduplicated and rate-limited by the server. Highlight votes are keyed by authenticated voter ID, reject self-votes and duplicates, remain private until reveal, support ties, and never add competitive points.

## Verification

```powershell
cargo fmt --manifest-path services/game-server/Cargo.toml -- --check
cargo test --manifest-path services/game-server/Cargo.toml
cargo clippy --manifest-path services/game-server/Cargo.toml --all-targets --all-features -- -D warnings
npm.cmd run test
npm.cmd run build
```

Automated coverage includes authenticated room actions, reconnection, atomic host transfer, independent Truth or Dare choices, stable Spotlight membership, actor authorization, reroll limits, reaction deduplication, highlight vote privacy and duplication checks, departures, WebSocket authorization, and responsive UI interactions.

## Real internet acceptance test

After deployment, test with one host on home Wi-Fi, one browser on different Wi-Fi, and one phone with Wi-Fi disabled:

1. Create and join the same room through the public player URL.
2. Refresh each device and confirm no duplicate players appear.
3. Complete a Truth or Dare match through choices, every Spotlight, reroll, skip, reactions, highlight voting, final celebration, and **Play another game**.
4. Interrupt one connection and verify the same player returns; let another player’s grace period expire and verify readiness/order updates.
5. Disconnect the host beyond the grace period and verify exactly one longest-connected player receives host controls.

This repository supplies the implementation and deployment configuration. A genuine different-network test requires public hosting; it cannot be truthfully completed against only the local development addresses.
