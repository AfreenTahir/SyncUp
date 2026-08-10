use std::collections::HashSet;
use syncup_game_server::{
    catalog::{GameDefinition, GameExperience, Question, ResponseMode},
    model::{ErrorCode, GamePhase, TruthOrDarePhase},
    RoomManager,
};
use uuid::Uuid;

fn quiz_manager() -> RoomManager {
    RoomManager::with_catalog(vec![GameDefinition {
        id: "test-quiz".into(),
        title: "Test Quiz".into(),
        description: "A deterministic scoring test.".into(),
        icon: "timer".into(),
        category: "Quiz".into(),
        estimated_minutes: 5,
        response_mode: ResponseMode::Quiz,
        experience: GameExperience::Competitive,
        default_rounds: 1,
        min_players: 2,
        questions: vec![Question {
            prompt: "Choose the correct answer.".into(),
            category: "test".into(),
            difficulty: "easy".into(),
            options: vec!["Wrong".into(), "Correct".into()],
            visual_options: vec![],
            correct_option: Some(1),
        }],
    }])
}

#[test]
fn catalog_is_large_and_this_or_that_spans_twenty_categories() {
    let games = syncup_game_server::catalog::load_default_catalog();
    assert!(games.iter().all(|game| game.questions.len() >= 20));
    let this_or_that = games.iter().find(|game| game.id == "this-or-that").unwrap();
    assert!(this_or_that.questions.len() >= 200);
    let categories: HashSet<_> = this_or_that
        .questions
        .iter()
        .map(|question| question.category.as_str())
        .collect();
    assert!(categories.len() >= 20);
}

#[tokio::test]
async fn quiz_answers_score_and_rank_players() {
    let manager = quiz_manager();
    let (code, token) = manager
        .create_room_with_host_options("test-quiz", Some(1), None, "Host")
        .await
        .unwrap();
    manager
        .authenticate_host(&code, &token, Uuid::new_v4())
        .await
        .unwrap();
    let (afreen, _) = manager.join_room(&code, "Afreen").await.unwrap();
    let (sam, _) = manager.join_room(&code, "Sam").await.unwrap();
    manager.start_game(&code, &token).await.unwrap();
    let host = manager.public_snapshot(&code).await.unwrap().host_player_id;
    manager.submit_answer(&code, host, 1, "0").await.unwrap();
    manager.submit_answer(&code, afreen, 1, "1").await.unwrap();
    manager.submit_answer(&code, sam, 1, "0").await.unwrap();
    manager.reveal_results(&code, &token).await.unwrap();

    let snapshot = manager.public_snapshot(&code).await.unwrap();
    assert_eq!(snapshot.phase, GamePhase::Results);
    assert_eq!(snapshot.leaderboard[0].nickname, "Afreen");
    assert_eq!(snapshot.leaderboard[0].score, 100);
    assert_eq!(snapshot.leaderboard[1].score, 0);
    let correct = snapshot
        .results
        .unwrap()
        .into_iter()
        .find(|result| result.label == "Correct")
        .unwrap();
    assert_eq!(correct.votes, 1);
    assert_eq!(correct.is_correct, Some(true));
}

#[tokio::test]
async fn invalid_choice_is_rejected_and_answers_lock() {
    let manager = quiz_manager();
    let (code, token) = manager
        .create_room_for_game("test-quiz", Some(1))
        .await
        .unwrap();
    manager
        .authenticate_host(&code, &token, Uuid::new_v4())
        .await
        .unwrap();
    let (afreen, _) = manager.join_room(&code, "Afreen").await.unwrap();
    manager.join_room(&code, "Sam").await.unwrap();
    manager.start_game(&code, &token).await.unwrap();
    assert!(manager.submit_answer(&code, afreen, 1, "99").await.is_err());
    manager.submit_answer(&code, afreen, 1, "1").await.unwrap();
    assert!(manager.submit_answer(&code, afreen, 1, "0").await.is_err());
}

#[tokio::test]
async fn voting_games_do_not_award_points() {
    let manager = RoomManager::with_catalog(syncup_game_server::catalog::load_default_catalog());
    let (code, token) = manager
        .create_room_with_host_options("most-likely-to", Some(1), None, "Host")
        .await
        .unwrap();
    manager
        .authenticate_host(&code, &token, Uuid::new_v4())
        .await
        .unwrap();
    let (afreen, _) = manager.join_room(&code, "Afreen").await.unwrap();
    let (sam, _) = manager.join_room(&code, "Sam").await.unwrap();
    manager.start_game(&code, &token).await.unwrap();
    let host = manager.public_snapshot(&code).await.unwrap().host_player_id;
    manager.submit_vote(&code, host, 1, sam).await.unwrap();
    manager.submit_vote(&code, afreen, 1, sam).await.unwrap();
    manager.submit_vote(&code, sam, 1, afreen).await.unwrap();
    manager.reveal_results(&code, &token).await.unwrap();
    let snapshot = manager.public_snapshot(&code).await.unwrap();
    assert!(snapshot.leaderboard.iter().all(|entry| entry.score == 0));
    assert!(snapshot.round_points.is_empty());
}

#[tokio::test]
async fn truth_or_dare_keeps_choices_prompts_and_completion_private_per_player() {
    let manager = RoomManager::with_catalog(syncup_game_server::catalog::load_default_catalog());
    let (code, token) = manager
        .create_room_with_host_options("truth-or-dare", Some(2), None, "Host")
        .await
        .unwrap();
    manager
        .authenticate_host(&code, &token, Uuid::new_v4())
        .await
        .unwrap();
    let (afreen, afreen_token) = manager.join_room(&code, "Afreen").await.unwrap();
    let (sam, _) = manager.join_room(&code, "Sam").await.unwrap();
    manager.start_game(&code, &token).await.unwrap();
    let snapshot = manager.public_snapshot(&code).await.unwrap();
    let host = snapshot.host_player_id;
    assert!(snapshot.active_player_id.is_none());
    manager
        .submit_answer(&code, host, 1, "truth")
        .await
        .unwrap();
    manager
        .submit_answer(&code, afreen, 1, "dare")
        .await
        .unwrap();
    manager.submit_answer(&code, sam, 1, "truth").await.unwrap();
    assert!(manager.submit_answer(&code, host, 1, "dare").await.is_err());

    let public = manager.public_snapshot(&code).await.unwrap();
    assert_eq!(public.submitted_answer_count, 3);
    assert!(public.viewer_truth_or_dare_choice.is_none());
    assert!(public.viewer_truth_or_dare_prompt.is_none());
    let host_private = manager.snapshot_for_player(&code, host).await.unwrap();
    let afreen_private = manager.snapshot_for_player(&code, afreen).await.unwrap();
    let sam_private = manager.snapshot_for_player(&code, sam).await.unwrap();
    assert_eq!(
        host_private.viewer_truth_or_dare_choice.as_deref(),
        Some("truth")
    );
    assert_eq!(
        afreen_private.viewer_truth_or_dare_choice.as_deref(),
        Some("dare")
    );
    assert_eq!(
        sam_private.viewer_truth_or_dare_choice.as_deref(),
        Some("truth")
    );
    assert!(host_private.viewer_truth_or_dare_prompt.is_none());

    manager.reveal_results(&code, &token).await.unwrap();
    let spotlight = manager.public_snapshot(&code).await.unwrap();
    assert_eq!(
        spotlight.truth_or_dare_phase,
        Some(TruthOrDarePhase::Spotlight)
    );
    assert_eq!(spotlight.spotlight_order.len(), 3);
    assert_eq!(
        spotlight
            .spotlight_order
            .iter()
            .copied()
            .collect::<HashSet<_>>(),
        HashSet::from([host, afreen, sam])
    );
    assert!(spotlight.spotlight_prompt.is_some());
    assert_eq!(
        spotlight.spotlight_player_id,
        spotlight.spotlight_order.first().copied()
    );

    let reconnect_one = Uuid::new_v4();
    manager
        .authenticate_player(&code, afreen, &afreen_token, reconnect_one)
        .await
        .unwrap();
    manager
        .disconnect_player(&code, afreen, reconnect_one)
        .await;
    let restored = manager
        .authenticate_player(&code, afreen, &afreen_token, Uuid::new_v4())
        .await
        .unwrap();
    assert_eq!(
        restored.viewer_truth_or_dare_choice.as_deref(),
        Some("dare")
    );
    if restored.spotlight_player_id == Some(afreen) {
        assert_eq!(
            restored.viewer_truth_or_dare_prompt,
            restored.spotlight_prompt
        );
    } else {
        assert!(restored.viewer_truth_or_dare_prompt.is_none());
    }

    for _ in 0..3 {
        let current = manager.public_snapshot(&code).await.unwrap();
        let spotlight_player = current.spotlight_player_id.unwrap();
        let choice = current.spotlight_choice.as_deref().unwrap();
        let prompt = current.spotlight_prompt.as_deref().unwrap();
        assert!(!prompt.is_empty());
        assert!(matches!(choice, "truth" | "dare"));
        assert!(manager
            .mark_completed(
                &code,
                if spotlight_player == host {
                    afreen
                } else {
                    host
                },
                1
            )
            .await
            .is_err());
        manager
            .mark_completed(&code, spotlight_player, 1)
            .await
            .unwrap();
        manager.next_spotlight(&code, host, 1).await.unwrap();
    }
    let voting = manager.public_snapshot(&code).await.unwrap();
    assert_eq!(
        voting.truth_or_dare_phase,
        Some(TruthOrDarePhase::HighlightVoting)
    );
    for player in [host, afreen, sam] {
        manager
            .skip_highlight_voting(&code, player, 1)
            .await
            .unwrap();
    }
    manager.reveal_highlights(&code, host, 1).await.unwrap();
    manager.next_round(&code, &token).await.unwrap();
    let snapshot = manager.public_snapshot(&code).await.unwrap();
    assert_eq!(snapshot.round, 2);
    assert_eq!(snapshot.phase, GamePhase::Playing);
    assert_eq!(snapshot.submitted_answer_count, 0);
    assert_eq!(snapshot.completed_count, 0);
    assert!(!snapshot.truth_or_dare_revealed);
    assert_eq!(
        snapshot.truth_or_dare_phase,
        Some(TruthOrDarePhase::Choosing)
    );
    let reset = manager.snapshot_for_player(&code, host).await.unwrap();
    assert!(reset.viewer_truth_or_dare_choice.is_none());
    assert!(reset.viewer_truth_or_dare_prompt.is_none());
    assert!(snapshot.leaderboard.iter().all(|entry| entry.score == 0));
}

#[tokio::test]
async fn spotlight_rerolls_reactions_and_highlight_votes_are_authoritative() {
    let manager = RoomManager::with_catalog(syncup_game_server::catalog::load_default_catalog());
    let (code, token) = manager
        .create_room_with_host_options("truth-or-dare", Some(2), None, "Host")
        .await
        .unwrap();
    manager
        .authenticate_host(&code, &token, Uuid::new_v4())
        .await
        .unwrap();
    let host = manager.public_snapshot(&code).await.unwrap().host_player_id;
    let (afreen, _) = manager.join_room(&code, "Afreen").await.unwrap();
    let (sam, _) = manager.join_room(&code, "Sam").await.unwrap();
    manager.start_game(&code, &token).await.unwrap();
    manager.submit_answer(&code, host, 1, "dare").await.unwrap();
    manager
        .submit_answer(&code, afreen, 1, "dare")
        .await
        .unwrap();
    manager.submit_answer(&code, sam, 1, "truth").await.unwrap();
    manager.reveal_results(&code, &token).await.unwrap();

    let first = manager.public_snapshot(&code).await.unwrap();
    let current = first.spotlight_player_id.unwrap();
    let original = first.spotlight_prompt.unwrap();
    manager.reroll_spotlight(&code, current, 1).await.unwrap();
    let rerolled = manager.public_snapshot(&code).await.unwrap();
    assert_eq!(rerolled.spotlight_choice, first.spotlight_choice);
    assert_ne!(
        rerolled.spotlight_prompt.as_deref(),
        Some(original.as_str())
    );
    assert!(manager.reroll_spotlight(&code, current, 1).await.is_err());
    let reaction_id = Uuid::new_v4();
    manager
        .send_reaction(&code, afreen, 1, "🔥", reaction_id)
        .await
        .unwrap();
    manager
        .send_reaction(&code, afreen, 1, "🔥", reaction_id)
        .await
        .unwrap();
    assert_eq!(
        manager
            .public_snapshot(&code)
            .await
            .unwrap()
            .reactions
            .len(),
        1
    );

    for _ in 0..3 {
        let current = manager
            .public_snapshot(&code)
            .await
            .unwrap()
            .spotlight_player_id
            .unwrap();
        manager.mark_completed(&code, current, 1).await.unwrap();
        manager.next_spotlight(&code, host, 1).await.unwrap();
    }
    let voting = manager.public_snapshot(&code).await.unwrap();
    assert_eq!(
        voting.truth_or_dare_phase,
        Some(TruthOrDarePhase::HighlightVoting)
    );
    assert!(voting.highlight_categories.len() >= 2);
    for voter in [host, afreen, sam] {
        for category in &voting.highlight_categories {
            let selected = category
                .eligible_player_ids
                .iter()
                .copied()
                .find(|candidate| *candidate != voter)
                .unwrap();
            manager
                .submit_highlight_vote(&code, voter, 1, &category.id, selected)
                .await
                .unwrap();
            assert!(manager
                .submit_highlight_vote(&code, voter, 1, &category.id, selected)
                .await
                .is_err());
        }
    }
    manager.reveal_highlights(&code, host, 1).await.unwrap();
    let results = manager.public_snapshot(&code).await.unwrap();
    assert_eq!(
        results.truth_or_dare_phase,
        Some(TruthOrDarePhase::HighlightResults)
    );
    assert_eq!(
        results.highlight_results.len(),
        voting.highlight_categories.len()
    );
}

#[tokio::test]
async fn truth_or_dare_departure_recalculates_readiness_and_removes_private_state() {
    let manager = RoomManager::with_catalog(syncup_game_server::catalog::load_default_catalog());
    let (code, token) = manager
        .create_room_with_host_options("truth-or-dare", Some(2), None, "Host")
        .await
        .unwrap();
    manager
        .authenticate_host(&code, &token, Uuid::new_v4())
        .await
        .unwrap();
    let host = manager.public_snapshot(&code).await.unwrap().host_player_id;
    let (afreen, old_token) = manager.join_room(&code, "Afreen").await.unwrap();
    let (sam, _) = manager.join_room(&code, "Sam").await.unwrap();
    manager.start_game(&code, &token).await.unwrap();
    manager
        .submit_answer(&code, host, 1, "truth")
        .await
        .unwrap();
    manager
        .submit_answer(&code, afreen, 1, "dare")
        .await
        .unwrap();
    manager.submit_answer(&code, sam, 1, "truth").await.unwrap();
    manager.leave_room(&code, afreen).await.unwrap();
    let snapshot = manager.public_snapshot(&code).await.unwrap();
    assert_eq!(snapshot.submitted_answer_count, 2);
    assert_eq!(snapshot.total_eligible_player_count, 2);
    manager.reveal_results(&code, &token).await.unwrap();
    let error = manager
        .authenticate_player(&code, afreen, &old_token, Uuid::new_v4())
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidPlayer);
}
