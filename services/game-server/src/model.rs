use crate::catalog::{GameExperience, Question, ResponseMode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};
use tokio::sync::broadcast;
use uuid::Uuid;

pub const MAX_PLAYERS: usize = 12;
pub const MAX_ROUNDS: u8 = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePhase {
    #[default]
    Lobby,
    Playing,
    Results,
    Finished,
    Choosing,
    Closed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TruthOrDarePhase {
    #[default]
    Choosing,
    PreparingReveal,
    Spotlight,
    HighlightVoting,
    HighlightResults,
    RoundComplete,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomNotice {
    pub id: u64,
    pub message: String,
    pub kind: String,
}

#[derive(Clone, Debug)]
pub struct Player {
    pub id: Uuid,
    pub nickname: String,
    pub session_token: String,
    pub connected: bool,
    pub last_seen: Instant,
    pub connection_id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoundResult {
    pub id: String,
    pub label: String,
    pub votes: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_correct: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundSummary {
    pub round: u8,
    pub question: String,
    pub category: String,
    pub results: Vec<RoundResult>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardEntry {
    pub player_id: Uuid,
    pub nickname: String,
    pub score: u32,
    pub rank: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicPlayer {
    pub id: Uuid,
    pub nickname: String,
    pub connected: bool,
    pub role: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoundPoint {
    pub player_id: Uuid,
    pub nickname: String,
    pub points: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TruthOrDarePlayerState {
    pub choice: String,
    pub prompt_id: String,
    pub prompt: String,
    pub completed: bool,
    pub skipped: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpotlightReaction {
    pub id: Uuid,
    pub player_id: Uuid,
    pub emoji: String,
    pub spotlight_index: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HighlightCategory {
    pub id: String,
    pub label: String,
    pub emoji: String,
    pub eligible_player_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HighlightResult {
    pub category_id: String,
    pub label: String,
    pub emoji: String,
    pub winner_player_ids: Vec<Uuid>,
    pub votes: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomSnapshot {
    pub version: u64,
    pub room_code: String,
    pub game_id: String,
    pub game_title: String,
    pub response_mode: ResponseMode,
    pub experience: GameExperience,
    pub phase: GamePhase,
    pub round: u8,
    pub max_rounds: u8,
    pub current_question: Option<String>,
    pub current_category: Option<String>,
    pub current_difficulty: Option<String>,
    pub current_options: Vec<String>,
    pub current_visual_options: Vec<String>,
    pub theme: String,
    pub round_points: Vec<RoundPoint>,
    pub active_player_id: Option<Uuid>,
    pub selected_answer: Option<String>,
    pub completed_count: usize,
    pub truth_or_dare_revealed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truth_or_dare_phase: Option<TruthOrDarePhase>,
    pub spotlight_order: Vec<Uuid>,
    pub spotlight_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spotlight_player_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spotlight_player_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spotlight_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spotlight_prompt: Option<String>,
    pub spotlight_completed: bool,
    pub spotlight_skipped: bool,
    pub viewer_reroll_available: bool,
    pub reactions: Vec<SpotlightReaction>,
    pub highlight_categories: Vec<HighlightCategory>,
    pub highlight_results: Vec<HighlightResult>,
    pub historical_highlights: Vec<HighlightResult>,
    pub reaction_totals: HashMap<Uuid, u32>,
    pub viewer_highlight_votes: HashMap<String, Uuid>,
    pub viewer_skipped_highlight_voting: bool,
    pub highlight_ready_count: usize,
    pub players: Vec<PublicPlayer>,
    pub submitted_answer_count: usize,
    pub total_eligible_player_count: usize,
    pub results: Option<Vec<RoundResult>>,
    pub leaderboard: Vec<LeaderboardEntry>,
    pub round_history: Option<Vec<RoundSummary>>,
    pub host_connected: bool,
    pub host_player_id: Uuid,
    pub notice: Option<RoomNotice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewer_has_answered: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewer_player_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewer_answer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewer_has_completed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewer_truth_or_dare_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewer_truth_or_dare_prompt_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewer_truth_or_dare_prompt: Option<String>,
}

#[derive(Debug)]
pub struct Room {
    pub id: Uuid,
    pub code: String,
    pub host_token: String,
    pub host_player_id: Uuid,
    pub host_connected: bool,
    pub host_connection_id: Option<Uuid>,
    pub host_last_seen: Instant,
    pub players: Vec<Player>,
    pub game_id: String,
    pub game_title: String,
    pub response_mode: ResponseMode,
    pub experience: GameExperience,
    pub theme: String,
    pub phase: GamePhase,
    pub question_queue: Vec<Question>,
    pub current_question: Option<Question>,
    pub current_round: u8,
    pub max_rounds: u8,
    pub answers: HashMap<Uuid, String>,
    pub scores: HashMap<Uuid, u32>,
    pub current_round_points: HashMap<Uuid, u32>,
    pub active_player_id: Option<Uuid>,
    pub completions: HashSet<Uuid>,
    pub truth_or_dare_states: HashMap<Uuid, TruthOrDarePlayerState>,
    pub truth_or_dare_revealed: bool,
    pub truth_or_dare_phase: Option<TruthOrDarePhase>,
    pub spotlight_order: Vec<Uuid>,
    pub spotlight_index: usize,
    pub rerolls_used: HashSet<Uuid>,
    pub reactions: Vec<SpotlightReaction>,
    pub highlight_categories: Vec<HighlightCategory>,
    pub highlight_votes: HashMap<String, HashMap<Uuid, Uuid>>,
    pub highlight_skips: HashSet<Uuid>,
    pub highlight_results: Vec<HighlightResult>,
    pub historical_highlights: Vec<HighlightResult>,
    pub reaction_totals: HashMap<Uuid, u32>,
    pub truth_prompts: Vec<String>,
    pub dare_prompts: Vec<String>,
    pub used_truth_prompts: HashMap<Uuid, HashSet<String>>,
    pub used_dare_prompts: HashMap<Uuid, HashSet<String>>,
    pub round_history: Vec<RoundSummary>,
    pub created_at: DateTime<Utc>,
    pub last_activity: Instant,
    pub expires_at: Instant,
    pub version: u64,
    pub notice: Option<RoomNotice>,
    pub updates: broadcast::Sender<RoomSnapshot>,
}

impl Room {
    pub fn snapshot(&self, viewer: Option<Uuid>) -> RoomSnapshot {
        let players = self
            .players
            .iter()
            .map(|player| PublicPlayer {
                id: player.id,
                nickname: player.nickname.clone(),
                connected: player.connected,
                role: if player.id == self.host_player_id {
                    "host"
                } else {
                    "player"
                }
                .into(),
            })
            .collect();
        let question = self.current_question.as_ref();
        let spotlight_player_id = self.spotlight_order.get(self.spotlight_index).copied();
        let spotlight_state = spotlight_player_id.and_then(|id| self.truth_or_dare_states.get(&id));
        let spotlight_player_name = spotlight_player_id.and_then(|id| {
            self.players
                .iter()
                .find(|player| player.id == id)
                .map(|player| player.nickname.clone())
        });
        let viewer_votes = viewer
            .map(|viewer_id| {
                self.highlight_votes
                    .iter()
                    .filter_map(|(category, votes)| {
                        votes
                            .get(&viewer_id)
                            .map(|selected| (category.clone(), *selected))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let highlight_ready_count = self
            .players
            .iter()
            .filter(|player| {
                self.highlight_skips.contains(&player.id)
                    || (!self.highlight_categories.is_empty()
                        && self.highlight_categories.iter().all(|category| {
                            self.highlight_votes
                                .get(&category.id)
                                .is_some_and(|votes| votes.contains_key(&player.id))
                        }))
            })
            .count();
        RoomSnapshot {
            version: self.version,
            room_code: self.code.clone(),
            game_id: self.game_id.clone(),
            game_title: self.game_title.clone(),
            response_mode: self.response_mode,
            experience: self.experience,
            phase: self.phase,
            round: self.current_round,
            max_rounds: self.max_rounds,
            current_question: if self.game_id == "truth-or-dare" {
                Some("Truth or Dare?".into())
            } else {
                question.map(|question| question.prompt.clone())
            },
            current_category: question.map(|question| question.category.clone()),
            current_difficulty: question.map(|question| question.difficulty.clone()),
            current_options: if self.game_id == "truth-or-dare" {
                vec!["Truth".into(), "Dare".into()]
            } else {
                question
                    .map(|question| question.options.clone())
                    .unwrap_or_default()
            },
            current_visual_options: question
                .map(|question| question.visual_options.clone())
                .unwrap_or_default(),
            theme: self.theme.clone(),
            round_points: self
                .current_round_points
                .iter()
                .filter_map(|(id, points)| {
                    self.players
                        .iter()
                        .find(|player| player.id == *id)
                        .map(|player| RoundPoint {
                            player_id: *id,
                            nickname: player.nickname.clone(),
                            points: *points,
                        })
                })
                .collect(),
            active_player_id: self.active_player_id,
            selected_answer: (self.game_id != "truth-or-dare")
                .then(|| {
                    self.active_player_id
                        .and_then(|id| self.answers.get(&id).cloned())
                })
                .flatten(),
            completed_count: self.completions.len(),
            truth_or_dare_revealed: self.truth_or_dare_revealed,
            truth_or_dare_phase: self.truth_or_dare_phase,
            spotlight_order: self.spotlight_order.clone(),
            spotlight_index: self.spotlight_index,
            spotlight_player_id,
            spotlight_player_name,
            spotlight_choice: spotlight_state.map(|state| state.choice.clone()),
            spotlight_prompt: spotlight_state.map(|state| state.prompt.clone()),
            spotlight_completed: spotlight_state.is_some_and(|state| state.completed),
            spotlight_skipped: spotlight_state.is_some_and(|state| state.skipped),
            viewer_reroll_available: viewer.is_some_and(|id| !self.rerolls_used.contains(&id)),
            reactions: self.reactions.clone(),
            highlight_categories: self.highlight_categories.clone(),
            highlight_results: self.highlight_results.clone(),
            historical_highlights: self.historical_highlights.clone(),
            reaction_totals: self.reaction_totals.clone(),
            viewer_highlight_votes: viewer_votes,
            viewer_skipped_highlight_voting: viewer
                .is_some_and(|id| self.highlight_skips.contains(&id)),
            highlight_ready_count,
            players,
            submitted_answer_count: self.answers.len(),
            total_eligible_player_count: self.players.len(),
            results: (self.phase == GamePhase::Results).then(|| self.calculate_results()),
            leaderboard: self.leaderboard(),
            round_history: (self.phase == GamePhase::Finished).then(|| self.round_history.clone()),
            host_connected: self.host_connected,
            host_player_id: self.host_player_id,
            notice: self.notice.clone(),
            viewer_has_answered: viewer.map(|id| self.answers.contains_key(&id)),
            viewer_player_id: viewer,
            viewer_answer: viewer.and_then(|id| self.answers.get(&id).cloned()),
            viewer_has_completed: viewer.map(|id| self.completions.contains(&id)),
            viewer_truth_or_dare_choice: viewer.and_then(|id| {
                self.truth_or_dare_states
                    .get(&id)
                    .map(|state| state.choice.clone())
            }),
            viewer_truth_or_dare_prompt_id: viewer.and_then(|id| {
                (spotlight_player_id == Some(id))
                    .then(|| {
                        self.truth_or_dare_states
                            .get(&id)
                            .map(|state| state.prompt_id.clone())
                    })
                    .flatten()
            }),
            viewer_truth_or_dare_prompt: viewer.and_then(|id| {
                (spotlight_player_id == Some(id))
                    .then(|| {
                        self.truth_or_dare_states
                            .get(&id)
                            .map(|state| state.prompt.clone())
                    })
                    .flatten()
            }),
        }
    }

    pub fn calculate_results(&self) -> Vec<RoundResult> {
        match self.response_mode {
            ResponseMode::PlayerVote => {
                let mut totals: HashMap<Uuid, u32> =
                    self.players.iter().map(|player| (player.id, 0)).collect();
                for answer in self.answers.values() {
                    if let Ok(selected) = Uuid::parse_str(answer) {
                        if let Some(total) = totals.get_mut(&selected) {
                            *total += 1;
                        }
                    }
                }
                let mut results: Vec<_> = self
                    .players
                    .iter()
                    .map(|player| RoundResult {
                        id: player.id.to_string(),
                        label: player.nickname.clone(),
                        votes: totals.get(&player.id).copied().unwrap_or_default(),
                        player_id: Some(player.id),
                        is_correct: None,
                    })
                    .collect();
                results.sort_by(|a, b| b.votes.cmp(&a.votes).then(a.label.cmp(&b.label)));
                results
            }
            ResponseMode::Choice | ResponseMode::Quiz => {
                let Some(question) = self.current_question.as_ref() else {
                    return vec![];
                };
                let mut totals = vec![0_u32; question.options.len()];
                for answer in self.answers.values() {
                    if let Ok(index) = answer.parse::<usize>() {
                        if let Some(total) = totals.get_mut(index) {
                            *total += 1;
                        }
                    }
                }
                let mut results: Vec<_> = question
                    .options
                    .iter()
                    .enumerate()
                    .map(|(index, label)| RoundResult {
                        id: index.to_string(),
                        label: label.clone(),
                        votes: totals[index],
                        player_id: None,
                        is_correct: (self.response_mode == ResponseMode::Quiz)
                            .then_some(question.correct_option == Some(index)),
                    })
                    .collect();
                results.sort_by(|a, b| b.votes.cmp(&a.votes).then(a.label.cmp(&b.label)));
                results
            }
        }
    }

    pub fn leaderboard(&self) -> Vec<LeaderboardEntry> {
        let mut entries: Vec<_> = self
            .players
            .iter()
            .map(|player| LeaderboardEntry {
                player_id: player.id,
                nickname: player.nickname.clone(),
                score: self.scores.get(&player.id).copied().unwrap_or_default(),
                rank: 0,
            })
            .collect();
        entries.sort_by(|a, b| b.score.cmp(&a.score).then(a.nickname.cmp(&b.nickname)));
        let mut previous_score = None;
        let mut previous_rank = 0;
        for (index, entry) in entries.iter_mut().enumerate() {
            let rank = if previous_score == Some(entry.score) {
                previous_rank
            } else {
                index + 1
            };
            entry.rank = rank;
            previous_score = Some(entry.score);
            previous_rank = rank;
        }
        entries
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientEvent {
    AuthenticateHost {
        #[serde(rename = "roomCode")]
        room_code: String,
        #[serde(rename = "hostToken")]
        host_token: String,
    },
    AuthenticatePlayer {
        #[serde(rename = "roomCode")]
        room_code: String,
        #[serde(rename = "playerId")]
        player_id: Uuid,
        #[serde(rename = "sessionToken")]
        session_token: String,
    },
    StartGame,
    SubmitAnswer {
        round: u8,
        answer: String,
    },
    SubmitVote {
        round: u8,
        #[serde(rename = "selectedPlayerId")]
        selected_player_id: Uuid,
    },
    MarkCompleted {
        round: u8,
    },
    RerollSpotlight {
        round: u8,
    },
    SkipSpotlight {
        round: u8,
    },
    ForceSkipSpotlight {
        round: u8,
    },
    NextSpotlight {
        round: u8,
    },
    SendReaction {
        round: u8,
        emoji: String,
        #[serde(rename = "reactionId")]
        reaction_id: Uuid,
    },
    SubmitHighlightVote {
        round: u8,
        #[serde(rename = "categoryId")]
        category_id: String,
        #[serde(rename = "selectedPlayerId")]
        selected_player_id: Uuid,
    },
    SkipHighlightVoting {
        round: u8,
    },
    RevealHighlights {
        round: u8,
    },
    RevealResults,
    NextRound,
    KickPlayer {
        #[serde(rename = "playerId")]
        player_id: Uuid,
    },
    EndGame,
    LeaveRoom,
    ChooseAnotherGame,
    ConfigureGame {
        #[serde(rename = "gameId")]
        game_id: String,
        rounds: u8,
        theme: String,
    },
    EndRoom,
    Ping,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    Authenticated { role: String },
    RoomSnapshot { snapshot: Box<RoomSnapshot> },
    Error { code: ErrorCode, message: String },
    Pong,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    RoomNotFound,
    RoomFull,
    RoomLocked,
    GameNotFound,
    InvalidNickname,
    InvalidToken,
    Unauthorized,
    InvalidPhase,
    NotEnoughPlayers,
    AlreadyAnswered,
    AlreadyVoted,
    InvalidAnswer,
    InvalidPlayer,
    RoundMismatch,
    HostDisconnected,
    InternalError,
}

#[derive(Clone, Debug)]
pub struct GameError {
    pub code: ErrorCode,
    pub message: String,
}

impl GameError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<GameError> for ServerEvent {
    fn from(error: GameError) -> Self {
        Self::Error {
            code: error.code,
            message: error.message,
        }
    }
}

#[derive(Clone, Debug)]
pub enum AuthenticatedRole {
    Host { host_token: String, player_id: Uuid },
    Player { player_id: Uuid },
}
