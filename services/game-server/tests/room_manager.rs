use std::{collections::HashSet, time::Duration};
use syncup_game_server::{
    model::{ErrorCode, GamePhase, MAX_PLAYERS},
    RoomManager,
};
use uuid::Uuid;

fn manager() -> RoomManager {
    RoomManager::new(
        (1..=30)
            .map(|number| format!("Friendly question {number}?"))
            .collect(),
    )
}

async fn connected_host(manager: &RoomManager) -> (String, String, Uuid) {
    let (code, token) = manager.create_room().await;
    let connection = Uuid::new_v4();
    manager
        .authenticate_host(&code, &token, connection)
        .await
        .unwrap();
    (code, token, connection)
}

#[test]
fn room_code_format_is_unambiguous() {
    for _ in 0..1_000 {
        let code = RoomManager::generate_room_code();
        assert_eq!(code.len(), 6);
        assert!(code
            .chars()
            .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit()));
        assert!(!code.chars().any(|character| "O0I1".contains(character)));
    }
}

#[tokio::test]
async fn generated_room_codes_are_unique() {
    let manager = manager();
    let mut codes = HashSet::new();
    for _ in 0..500 {
        let (code, _) = manager.create_room().await;
        assert!(codes.insert(code));
    }
}

#[tokio::test]
async fn leaving_removes_player_and_transfers_host_atomically() {
    let manager = manager();
    let (code, _token, _connection) = connected_host(&manager).await;
    let original_host = manager.public_snapshot(&code).await.unwrap().host_player_id;
    let (sam, _) = manager.join_room(&code, "Sam").await.unwrap();
    let (afreen, _) = manager.join_room(&code, "Afreen").await.unwrap();

    manager.leave_room(&code, original_host).await.unwrap();
    let snapshot = manager.public_snapshot(&code).await.unwrap();
    assert_eq!(snapshot.host_player_id, sam);
    assert_eq!(snapshot.players.len(), 2);
    assert!(!snapshot
        .players
        .iter()
        .any(|player| player.id == original_host));
    assert!(snapshot
        .notice
        .unwrap()
        .message
        .contains("Sam is now the host"));

    let error = manager
        .start_game_by_player(&code, afreen)
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Unauthorized);
    manager.start_game_by_player(&code, sam).await.unwrap();
}

#[tokio::test]
async fn leaving_recalculates_votes_and_completion_requirements() {
    let manager = manager();
    let (code, _token, _connection) = connected_host(&manager).await;
    let host = manager.public_snapshot(&code).await.unwrap().host_player_id;
    let (sam, _) = manager.join_room(&code, "Sam").await.unwrap();
    let (afreen, _) = manager.join_room(&code, "Afreen").await.unwrap();
    manager.start_game_by_player(&code, host).await.unwrap();
    manager.submit_vote(&code, host, 1, sam).await.unwrap();
    manager.submit_vote(&code, sam, 1, afreen).await.unwrap();
    manager.leave_room(&code, afreen).await.unwrap();
    let snapshot = manager.public_snapshot(&code).await.unwrap();
    assert_eq!(snapshot.players.len(), 2);
    assert_eq!(snapshot.submitted_answer_count, 1);
    assert_eq!(snapshot.total_eligible_player_count, 2);
    manager.submit_vote(&code, sam, 1, host).await.unwrap();
    manager.reveal_results_by_player(&code, host).await.unwrap();
}

#[tokio::test]
async fn another_game_keeps_room_and_roster_but_resets_match_state() {
    let manager = manager();
    let (code, _token, _connection) = connected_host(&manager).await;
    let host = manager.public_snapshot(&code).await.unwrap().host_player_id;
    let (sam, _) = manager.join_room(&code, "Sam").await.unwrap();
    manager.start_game_by_player(&code, host).await.unwrap();
    manager.submit_vote(&code, host, 1, sam).await.unwrap();
    manager.submit_vote(&code, sam, 1, host).await.unwrap();
    // Finish through the current host's supported flow, then configure the same game as a fresh match.
    manager.reveal_results_by_player(&code, host).await.unwrap();
    for _ in 1..5 {
        manager.next_round_by_player(&code, host).await.unwrap();
        let round = manager.public_snapshot(&code).await.unwrap().round;
        manager.submit_vote(&code, host, round, sam).await.unwrap();
        manager.submit_vote(&code, sam, round, host).await.unwrap();
        manager.reveal_results_by_player(&code, host).await.unwrap();
    }
    manager.next_round_by_player(&code, host).await.unwrap();
    manager.choose_another_game(&code, host).await.unwrap();
    manager
        .configure_game(&code, host, "priority-sync", 5, "sakura")
        .await
        .unwrap();
    let snapshot = manager.public_snapshot(&code).await.unwrap();
    assert_eq!(snapshot.room_code, code);
    assert_eq!(snapshot.players.len(), 2);
    assert_eq!(snapshot.phase, GamePhase::Lobby);
    assert_eq!(snapshot.round, 0);
    assert_eq!(snapshot.submitted_answer_count, 0);
    assert!(snapshot.leaderboard.iter().all(|entry| entry.score == 0));
}

#[tokio::test]
async fn room_capacity_is_twelve() {
    let manager = manager();
    let (code, _) = manager.create_room().await;
    for number in 0..(MAX_PLAYERS - 1) {
        manager
            .join_room(&code, &format!("Player {number}"))
            .await
            .unwrap();
    }
    let error = manager.join_room(&code, "Too many").await.unwrap_err();
    assert_eq!(error.code, ErrorCode::RoomFull);
}

#[tokio::test]
async fn nicknames_are_trimmed_validated_and_deduplicated() {
    let manager = manager();
    let (code, _) = manager.create_room().await;
    let (first, _) = manager.join_room(&code, "  Afreen  ").await.unwrap();
    let (second, _) = manager.join_room(&code, "afreen").await.unwrap();
    let snapshot = manager.public_snapshot(&code).await.unwrap();
    assert_eq!(
        snapshot
            .players
            .iter()
            .find(|player| player.id == first)
            .unwrap()
            .nickname,
        "Afreen (2)"
    );
    assert_eq!(
        snapshot
            .players
            .iter()
            .find(|player| player.id == second)
            .unwrap()
            .nickname,
        "afreen (3)"
    );
    for invalid in ["", "   ", "line\nbreak", "123456789012345678901"] {
        let error = manager.join_room(&code, invalid).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidNickname);
    }
}

#[tokio::test]
async fn invalid_authentication_tokens_are_rejected() {
    let manager = manager();
    let (code, host_token) = manager.create_room().await;
    let (player_id, session_token) = manager.join_room(&code, "Afreen").await.unwrap();
    assert_eq!(
        manager
            .authenticate_host(&code, "wrong", Uuid::new_v4())
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidToken
    );
    assert_eq!(
        manager
            .authenticate_player(&code, player_id, "wrong", Uuid::new_v4())
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidToken
    );
    manager
        .authenticate_host(&code, &host_token, Uuid::new_v4())
        .await
        .unwrap();
    manager
        .authenticate_player(&code, player_id, &session_token, Uuid::new_v4())
        .await
        .unwrap();
}

#[tokio::test]
async fn host_actions_require_auth_and_two_players() {
    let manager = manager();
    let (code, token, _) = connected_host(&manager).await;
    assert_eq!(
        manager
            .start_game(&code, "not-the-token")
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unauthorized
    );
    assert_eq!(
        manager.start_game(&code, &token).await.unwrap_err().code,
        ErrorCode::NotEnoughPlayers
    );
    manager.join_room(&code, "Afreen").await.unwrap();
    manager.start_game(&code, &token).await.unwrap();
}

#[tokio::test]
async fn voting_validates_round_target_and_duplicate() {
    let manager = manager();
    let (code, token, _) = connected_host(&manager).await;
    let (afreen, _) = manager.join_room(&code, "Afreen").await.unwrap();
    let (sam, _) = manager.join_room(&code, "Sam").await.unwrap();
    manager.start_game(&code, &token).await.unwrap();

    assert_eq!(
        manager
            .submit_vote(&code, afreen, 2, sam)
            .await
            .unwrap_err()
            .code,
        ErrorCode::RoundMismatch
    );
    assert_eq!(
        manager
            .submit_vote(&code, afreen, 1, Uuid::new_v4())
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidPlayer
    );
    manager.submit_vote(&code, afreen, 1, sam).await.unwrap();
    assert_eq!(
        manager
            .submit_vote(&code, afreen, 1, afreen)
            .await
            .unwrap_err()
            .code,
        ErrorCode::AlreadyVoted
    );
}

#[tokio::test]
async fn phase_transitions_results_and_history_are_authoritative() {
    let manager = manager();
    let (code, token, _) = connected_host(&manager).await;
    let (afreen, _) = manager.join_room(&code, "Afreen").await.unwrap();
    let (sam, _) = manager.join_room(&code, "Sam").await.unwrap();
    manager.start_game(&code, &token).await.unwrap();
    let mut seen_questions = HashSet::new();
    let total_rounds = manager.public_snapshot(&code).await.unwrap().max_rounds;
    let host = manager.public_snapshot(&code).await.unwrap().host_player_id;

    for round in 1..=total_rounds {
        let voting = manager.public_snapshot(&code).await.unwrap();
        assert_eq!(voting.phase, GamePhase::Playing);
        assert_eq!(voting.round, round);
        assert!(voting.results.is_none());
        assert!(seen_questions.insert(voting.current_question.unwrap()));
        manager
            .submit_vote(&code, afreen, round, sam)
            .await
            .unwrap();
        manager.submit_vote(&code, sam, round, sam).await.unwrap();
        manager.submit_vote(&code, host, round, sam).await.unwrap();
        manager.reveal_results(&code, &token).await.unwrap();
        let results = manager.public_snapshot(&code).await.unwrap();
        assert_eq!(results.phase, GamePhase::Results);
        let totals = results.results.unwrap();
        assert_eq!(totals[0].player_id, Some(sam));
        assert_eq!(totals[0].votes, 3);
        manager.next_round(&code, &token).await.unwrap();
    }

    let finished = manager.public_snapshot(&code).await.unwrap();
    assert_eq!(finished.phase, GamePhase::Finished);
    assert_eq!(finished.round_history.unwrap().len(), total_rounds as usize);
}

#[tokio::test]
async fn disconnect_and_reconnect_restore_connection_state() {
    let manager = manager();
    let (code, host_token, host_connection) = connected_host(&manager).await;
    let (player_id, player_token) = manager.join_room(&code, "Afreen").await.unwrap();
    let player_connection = Uuid::new_v4();
    manager
        .authenticate_player(&code, player_id, &player_token, player_connection)
        .await
        .unwrap();
    manager
        .disconnect_player(&code, player_id, player_connection)
        .await;
    manager.disconnect_host(&code, host_connection).await;
    let disconnected = manager.public_snapshot(&code).await.unwrap();
    assert!(!disconnected.host_connected);
    assert!(!disconnected.players[0].connected);

    manager
        .authenticate_host(&code, &host_token, Uuid::new_v4())
        .await
        .unwrap();
    manager
        .authenticate_player(&code, player_id, &player_token, Uuid::new_v4())
        .await
        .unwrap();
    let connected = manager.public_snapshot(&code).await.unwrap();
    assert!(connected.host_connected);
    assert!(connected.players[0].connected);
}

#[tokio::test]
async fn rooms_expire_after_disconnected_grace_period() {
    let manager = manager();
    manager.create_room().await;
    manager
        .cleanup_at(std::time::Instant::now() + Duration::from_secs(91))
        .await;
    assert_eq!(manager.room_count().await, 0);
}
