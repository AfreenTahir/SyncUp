use crate::{
    catalog::{
        priority_sync_from_strings, GameDefinition, GameExperience, GameSummary, ResponseMode,
    },
    model::{
        AuthenticatedRole, ErrorCode, GameError, GamePhase, HighlightCategory, HighlightResult,
        Player, Room, RoomSnapshot, RoundSummary, SpotlightReaction, TruthOrDarePhase,
        TruthOrDarePlayerState, MAX_PLAYERS, MAX_ROUNDS,
    },
};
use chrono::Utc;
use dashmap::{mapref::entry::Entry, DashMap};
use rand::{rngs::OsRng, seq::SliceRandom, RngCore};
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

const ROOM_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const DISCONNECT_GRACE: Duration = Duration::from_secs(30);
const ACTIVE_ROOM_LIFETIME: Duration = Duration::from_secs(2 * 60 * 60);

#[derive(Clone)]
pub struct RoomManager {
    rooms: Arc<DashMap<String, Arc<RwLock<Room>>>>,
    games: Arc<Vec<GameDefinition>>,
    recent_questions: Arc<RwLock<HashMap<String, VecDeque<String>>>>,
}

impl RoomManager {
    pub fn new(questions: Vec<String>) -> Self {
        Self::with_catalog(vec![priority_sync_from_strings(questions)])
    }

    pub fn with_catalog(games: Vec<GameDefinition>) -> Self {
        assert!(!games.is_empty(), "game catalog cannot be empty");
        assert!(
            games.iter().all(|game| !game.questions.is_empty()),
            "every game needs at least one question"
        );
        Self {
            rooms: Arc::new(DashMap::new()),
            games: Arc::new(games),
            recent_questions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn game_catalog(&self) -> Vec<GameSummary> {
        self.games.iter().map(GameSummary::from).collect()
    }

    pub fn generate_room_code() -> String {
        let mut random = [0_u8; 6];
        OsRng.fill_bytes(&mut random);
        random
            .iter()
            .map(|byte| ROOM_ALPHABET[*byte as usize % ROOM_ALPHABET.len()] as char)
            .collect()
    }

    fn generate_token() -> String {
        let mut bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut bytes);
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    pub async fn create_room(&self) -> (String, String) {
        self.create_room_for_game("priority-sync", None)
            .await
            .expect("default game must exist")
    }

    pub async fn create_room_for_game(
        &self,
        game_id: &str,
        requested_rounds: Option<u8>,
    ) -> Result<(String, String), GameError> {
        self.create_room_with_options(game_id, requested_rounds, None)
            .await
    }

    pub async fn create_room_with_options(
        &self,
        game_id: &str,
        requested_rounds: Option<u8>,
        requested_theme: Option<&str>,
    ) -> Result<(String, String), GameError> {
        self.create_room_with_host_options(game_id, requested_rounds, requested_theme, "Afreen")
            .await
    }

    pub async fn create_room_with_host_options(
        &self,
        game_id: &str,
        requested_rounds: Option<u8>,
        requested_theme: Option<&str>,
        host_nickname: &str,
    ) -> Result<(String, String), GameError> {
        let game = self
            .games
            .iter()
            .find(|game| game.id == game_id)
            .or_else(|| self.games.first().filter(|_| game_id == "priority-sync"))
            .ok_or_else(|| GameError::new(ErrorCode::GameNotFound, "That game is unavailable."))?
            .clone();
        let rounds = requested_rounds
            .unwrap_or(game.default_rounds)
            .clamp(1, MAX_ROUNDS)
            .min(game.questions.len() as u8);
        let host_nickname = Self::validate_nickname(host_nickname)?;
        let question_queue = self.fresh_balanced_questions(&game, rounds as usize).await;
        let (truth_prompts, dare_prompts) = Self::truth_or_dare_pools(&game);
        let theme = match requested_theme {
            Some("ocean-blue" | "sunset-glow" | "emerald" | "purple-galaxy" | "sakura") => {
                requested_theme.unwrap()
            }
            _ => "neon-night",
        }
        .to_owned();

        loop {
            let code = Self::generate_room_code();
            let host_token = Self::generate_token();
            let host_player_id = Uuid::new_v4();
            let now = std::time::Instant::now();
            let (updates, _) = broadcast::channel(64);
            let room = Room {
                id: Uuid::new_v4(),
                code: code.clone(),
                host_token: host_token.clone(),
                host_player_id,
                host_connected: false,
                host_connection_id: None,
                host_last_seen: now,
                players: vec![Player {
                    id: host_player_id,
                    nickname: host_nickname.clone(),
                    session_token: Self::generate_token(),
                    connected: false,
                    last_seen: now,
                    connection_id: None,
                }],
                game_id: game.id.clone(),
                game_title: game.title.clone(),
                response_mode: game.response_mode,
                experience: game.experience,
                theme: theme.clone(),
                phase: GamePhase::Lobby,
                question_queue: question_queue.clone(),
                current_question: None,
                current_round: 0,
                max_rounds: rounds,
                answers: Default::default(),
                scores: HashMap::from([(host_player_id, 0)]),
                current_round_points: Default::default(),
                active_player_id: None,
                completions: Default::default(),
                truth_or_dare_states: Default::default(),
                truth_or_dare_revealed: false,
                truth_or_dare_phase: (game.id == "truth-or-dare")
                    .then_some(TruthOrDarePhase::Choosing),
                spotlight_order: vec![],
                spotlight_index: 0,
                rerolls_used: Default::default(),
                reactions: vec![],
                highlight_categories: vec![],
                highlight_votes: Default::default(),
                highlight_skips: Default::default(),
                highlight_results: vec![],
                historical_highlights: vec![],
                reaction_totals: Default::default(),
                truth_prompts: truth_prompts.clone(),
                dare_prompts: dare_prompts.clone(),
                used_truth_prompts: Default::default(),
                used_dare_prompts: Default::default(),
                round_history: vec![],
                created_at: Utc::now(),
                last_activity: now,
                expires_at: now + DISCONNECT_GRACE,
                version: 1,
                notice: None,
                updates,
            };
            match self.rooms.entry(code.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(Arc::new(RwLock::new(room)));
                    return Ok((code, host_token));
                }
                Entry::Occupied(_) => continue,
            }
        }
    }

    async fn fresh_balanced_questions(
        &self,
        game: &GameDefinition,
        count: usize,
    ) -> Vec<crate::catalog::Question> {
        let mut recent_by_game = self.recent_questions.write().await;
        let recent = recent_by_game.entry(game.id.clone()).or_default();
        let mut recent_set: HashSet<_> = recent.iter().cloned().collect();
        while game
            .questions
            .iter()
            .filter(|question| !recent_set.contains(&question.prompt))
            .count()
            < count
        {
            let Some(oldest) = recent.pop_front() else {
                break;
            };
            recent_set.remove(&oldest);
        }

        let mut categories: BTreeMap<String, Vec<crate::catalog::Question>> = BTreeMap::new();
        for question in game
            .questions
            .iter()
            .filter(|question| !recent_set.contains(&question.prompt))
        {
            categories
                .entry(question.category.clone())
                .or_default()
                .push(question.clone());
        }
        let mut category_order: Vec<_> = categories.keys().cloned().collect();
        category_order.shuffle(&mut OsRng);
        for questions in categories.values_mut() {
            questions.shuffle(&mut OsRng);
        }
        let mut selected = Vec::with_capacity(count);
        while selected.len() < count {
            let mut added = false;
            for category in &category_order {
                if let Some(question) = categories.get_mut(category).and_then(Vec::pop) {
                    selected.push(question);
                    added = true;
                    if selected.len() == count {
                        break;
                    }
                }
            }
            if !added {
                break;
            }
        }
        for question in &selected {
            recent.push_back(question.prompt.clone());
        }
        let history_limit = (game.questions.len() * 3 / 4).max(count);
        while recent.len() > history_limit {
            recent.pop_front();
        }
        selected
    }

    fn truth_or_dare_pools(game: &GameDefinition) -> (Vec<String>, Vec<String>) {
        if game.id != "truth-or-dare" {
            return (vec![], vec![]);
        }
        let mut truths = vec![];
        let mut dares = vec![];
        for option in game.questions.iter().flat_map(|question| &question.options) {
            if let Some(prompt) = option.strip_prefix("Truth: ") {
                truths.push(prompt.to_owned());
            } else if let Some(prompt) = option.strip_prefix("Dare: ") {
                dares.push(prompt.to_owned());
            }
        }
        (truths, dares)
    }

    fn room(&self, code: &str) -> Result<Arc<RwLock<Room>>, GameError> {
        self.rooms
            .get(&code.trim().to_ascii_uppercase())
            .map(|entry| Arc::clone(entry.value()))
            .ok_or_else(|| GameError::new(ErrorCode::RoomNotFound, "The room could not be found."))
    }

    pub async fn join_room(&self, code: &str, nickname: &str) -> Result<(Uuid, String), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        if !matches!(room.phase, GamePhase::Lobby | GamePhase::Choosing) {
            return Err(GameError::new(
                ErrorCode::RoomLocked,
                "The game has already started.",
            ));
        }
        if room.players.len() >= MAX_PLAYERS {
            return Err(GameError::new(ErrorCode::RoomFull, "The room is full."));
        }
        let nickname = Self::validate_nickname(nickname)?;
        let nickname = Self::unique_nickname(&room.players, &nickname);
        let now = std::time::Instant::now();
        let player_id = Uuid::new_v4();
        let token = Self::generate_token();
        room.players.push(Player {
            id: player_id,
            nickname,
            session_token: token.clone(),
            connected: false,
            last_seen: now,
            connection_id: None,
        });
        room.scores.insert(player_id, 0);
        Self::changed(&mut room);
        Ok((player_id, token))
    }

    pub fn validate_nickname(nickname: &str) -> Result<String, GameError> {
        let nickname = nickname.trim();
        let length = nickname.chars().count();
        if !(1..=20).contains(&length) || nickname.chars().any(char::is_control) {
            return Err(GameError::new(
                ErrorCode::InvalidNickname,
                "Nickname must be 1 to 20 characters and contain no control characters.",
            ));
        }
        Ok(nickname.to_owned())
    }

    fn unique_nickname(players: &[Player], requested: &str) -> String {
        if !players
            .iter()
            .any(|player| player.nickname.eq_ignore_ascii_case(requested))
        {
            return requested.to_owned();
        }
        for suffix in 2..=MAX_PLAYERS + 1 {
            let suffix_text = format!(" ({suffix})");
            let keep = 20_usize.saturating_sub(suffix_text.chars().count());
            let base: String = requested.chars().take(keep).collect();
            let candidate = format!("{base}{suffix_text}");
            if !players
                .iter()
                .any(|player| player.nickname.eq_ignore_ascii_case(&candidate))
            {
                return candidate;
            }
        }
        requested.to_owned()
    }

    pub async fn public_snapshot(&self, code: &str) -> Result<RoomSnapshot, GameError> {
        let room = self.room(code)?;
        let snapshot = room.read().await.snapshot(None);
        Ok(snapshot)
    }

    pub async fn snapshot_for_player(
        &self,
        code: &str,
        player_id: Uuid,
    ) -> Result<RoomSnapshot, GameError> {
        let room = self.room(code)?;
        let snapshot = room.read().await.snapshot(Some(player_id));
        Ok(snapshot)
    }

    pub async fn snapshot_for_role(
        &self,
        code: &str,
        role: &AuthenticatedRole,
    ) -> Result<RoomSnapshot, GameError> {
        match role {
            AuthenticatedRole::Host { player_id, .. } => {
                self.snapshot_for_player(code, *player_id).await
            }
            AuthenticatedRole::Player { player_id } => {
                self.snapshot_for_player(code, *player_id).await
            }
        }
    }

    pub async fn subscribe(
        &self,
        code: &str,
    ) -> Result<broadcast::Receiver<RoomSnapshot>, GameError> {
        let room = self.room(code)?;
        let receiver = room.read().await.updates.subscribe();
        Ok(receiver)
    }

    pub async fn authenticate_host(
        &self,
        code: &str,
        token: &str,
        connection_id: Uuid,
    ) -> Result<RoomSnapshot, GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        if room.host_token != token {
            return Err(GameError::new(
                ErrorCode::InvalidToken,
                "Invalid host token.",
            ));
        }
        room.host_connected = true;
        room.host_connection_id = Some(connection_id);
        room.host_last_seen = std::time::Instant::now();
        room.expires_at = std::time::Instant::now() + ACTIVE_ROOM_LIFETIME;
        let host_player_id = room.host_player_id;
        if let Some(host) = room
            .players
            .iter_mut()
            .find(|player| player.id == host_player_id)
        {
            host.connected = true;
            host.connection_id = Some(connection_id);
            host.last_seen = std::time::Instant::now();
        }
        Self::changed(&mut room);
        Ok(room.snapshot(Some(room.host_player_id)))
    }

    pub async fn authenticate_player(
        &self,
        code: &str,
        player_id: Uuid,
        token: &str,
        connection_id: Uuid,
    ) -> Result<RoomSnapshot, GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        let player = room
            .players
            .iter_mut()
            .find(|player| player.id == player_id)
            .ok_or_else(|| {
                GameError::new(ErrorCode::InvalidPlayer, "Player is not in this room.")
            })?;
        if player.session_token != token {
            return Err(GameError::new(
                ErrorCode::InvalidToken,
                "Invalid player session token.",
            ));
        }
        player.connected = true;
        player.connection_id = Some(connection_id);
        player.last_seen = std::time::Instant::now();
        if room.host_player_id == player_id {
            room.host_connected = true;
            room.host_connection_id = Some(connection_id);
        }
        Self::changed(&mut room);
        Ok(room.snapshot(Some(player_id)))
    }

    pub async fn disconnect_host(&self, code: &str, connection_id: Uuid) {
        if let Ok(room) = self.room(code) {
            let mut room = room.write().await;
            if room.host_connection_id == Some(connection_id) {
                room.host_connected = false;
                room.host_connection_id = None;
                room.host_last_seen = std::time::Instant::now();
                room.expires_at = std::time::Instant::now() + DISCONNECT_GRACE;
                let host_player_id = room.host_player_id;
                if let Some(host) = room
                    .players
                    .iter_mut()
                    .find(|player| player.id == host_player_id)
                {
                    host.connected = false;
                    host.connection_id = None;
                    host.last_seen = std::time::Instant::now();
                }
                Self::changed(&mut room);
            }
        }
    }

    pub async fn disconnect_player(&self, code: &str, player_id: Uuid, connection_id: Uuid) {
        if let Ok(room) = self.room(code) {
            let mut room = room.write().await;
            if let Some(player) = room.players.iter_mut().find(|player| {
                player.id == player_id && player.connection_id == Some(connection_id)
            }) {
                player.connected = false;
                player.connection_id = None;
                player.last_seen = std::time::Instant::now();
                if room.host_player_id == player_id {
                    room.host_connected = false;
                    room.host_connection_id = None;
                    room.host_last_seen = std::time::Instant::now();
                }
                Self::changed(&mut room);
            }
        }
    }

    fn authorize_player_host(room: &Room, player_id: Uuid) -> Result<(), GameError> {
        if room.host_player_id != player_id || !room.players.iter().any(|p| p.id == player_id) {
            return Err(GameError::new(
                ErrorCode::Unauthorized,
                "Only the current host can do that.",
            ));
        }
        Ok(())
    }

    fn remove_player_state(room: &mut Room, player_id: Uuid) -> Option<String> {
        let index = room
            .players
            .iter()
            .position(|player| player.id == player_id)?;
        let nickname = room.players.remove(index).nickname;
        room.answers.remove(&player_id);
        room.scores.remove(&player_id);
        room.current_round_points.remove(&player_id);
        room.completions.remove(&player_id);
        room.truth_or_dare_states.remove(&player_id);
        let was_current_spotlight =
            room.spotlight_order.get(room.spotlight_index).copied() == Some(player_id);
        room.spotlight_order.retain(|id| *id != player_id);
        if room.spotlight_index >= room.spotlight_order.len() && !room.spotlight_order.is_empty() {
            room.spotlight_index = room.spotlight_order.len() - 1;
        }
        room.rerolls_used.remove(&player_id);
        room.highlight_skips.remove(&player_id);
        for votes in room.highlight_votes.values_mut() {
            votes.remove(&player_id);
            votes.retain(|_, selected| *selected != player_id);
        }
        for category in &mut room.highlight_categories {
            category.eligible_player_ids.retain(|id| *id != player_id);
        }
        room.used_truth_prompts.remove(&player_id);
        room.used_dare_prompts.remove(&player_id);
        room.answers
            .retain(|_, answer| answer != &player_id.to_string());
        if room.truth_or_dare_phase == Some(TruthOrDarePhase::Choosing)
            && !room.players.is_empty()
            && room.answers.len() == room.players.len()
        {
            room.truth_or_dare_phase = Some(TruthOrDarePhase::PreparingReveal);
        }
        if room.active_player_id == Some(player_id) {
            room.active_player_id = room.players.first().map(|player| player.id);
        }
        if was_current_spotlight
            && room.truth_or_dare_phase == Some(TruthOrDarePhase::Spotlight)
            && (room.spotlight_order.is_empty()
                || room.spotlight_order.iter().all(|id| {
                    room.truth_or_dare_states
                        .get(id)
                        .is_some_and(|state| state.completed || state.skipped)
                }))
        {
            Self::prepare_highlight_voting(room);
        }
        Some(nickname)
    }

    pub async fn leave_room(&self, code: &str, player_id: Uuid) -> Result<(), GameError> {
        let room_ref = self.room(code)?;
        let mut room = room_ref.write().await;
        let was_host = room.host_player_id == player_id;
        let nickname = Self::remove_player_state(&mut room, player_id).ok_or_else(|| {
            GameError::new(ErrorCode::InvalidPlayer, "Player is not in this room.")
        })?;
        if room.players.is_empty() {
            room.phase = GamePhase::Closed;
            room.notice = Some(crate::model::RoomNotice {
                id: room.version + 1,
                message: "The room has ended.".into(),
                kind: "room_ended".into(),
            });
        } else if was_host {
            room.host_player_id = room.players[0].id;
            room.host_token = Self::generate_token();
            room.host_connected = room.players[0].connected;
            room.host_connection_id = room.players[0].connection_id;
            let next = room.players[0].nickname.clone();
            room.notice = Some(crate::model::RoomNotice {
                id: room.version + 1,
                message: format!("{nickname} left. {next} is now the host."),
                kind: "host_transferred".into(),
            });
        } else {
            room.notice = Some(crate::model::RoomNotice {
                id: room.version + 1,
                message: format!("{nickname} left the game."),
                kind: "player_left".into(),
            });
        }
        Self::changed(&mut room);
        Ok(())
    }

    pub async fn choose_another_game(&self, code: &str, player_id: Uuid) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_player_host(&room, player_id)?;
        if room.phase != GamePhase::Finished {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Finish the current game first.",
            ));
        }
        room.phase = GamePhase::Choosing;
        room.answers.clear();
        room.completions.clear();
        room.truth_or_dare_states.clear();
        room.truth_or_dare_revealed = false;
        room.truth_or_dare_phase = None;
        room.spotlight_order.clear();
        room.reactions.clear();
        room.highlight_categories.clear();
        room.highlight_votes.clear();
        room.highlight_skips.clear();
        room.highlight_results.clear();
        room.current_round_points.clear();
        room.current_question = None;
        room.active_player_id = None;
        room.notice = Some(crate::model::RoomNotice {
            id: room.version + 1,
            message: "The host is choosing the next game.".into(),
            kind: "choosing_game".into(),
        });
        Self::changed(&mut room);
        Ok(())
    }

    pub async fn configure_game(
        &self,
        code: &str,
        player_id: Uuid,
        game_id: &str,
        requested_rounds: u8,
        requested_theme: &str,
    ) -> Result<(), GameError> {
        let game = self
            .games
            .iter()
            .find(|game| game.id == game_id)
            .ok_or_else(|| GameError::new(ErrorCode::GameNotFound, "That game is unavailable."))?
            .clone();
        let rounds = requested_rounds
            .clamp(1, MAX_ROUNDS)
            .min(game.questions.len() as u8);
        let queue = self.fresh_balanced_questions(&game, rounds as usize).await;
        let (truth_prompts, dare_prompts) = Self::truth_or_dare_pools(&game);
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_player_host(&room, player_id)?;
        if !matches!(room.phase, GamePhase::Choosing | GamePhase::Finished) {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "The party is not choosing a game.",
            ));
        }
        room.game_id = game.id;
        room.game_title = game.title;
        room.response_mode = game.response_mode;
        room.experience = game.experience;
        room.theme = match requested_theme {
            "ocean-blue" | "sunset-glow" | "emerald" | "purple-galaxy" | "sakura" => {
                requested_theme
            }
            _ => "neon-night",
        }
        .to_owned();
        room.max_rounds = rounds;
        room.question_queue = queue;
        room.current_question = None;
        room.current_round = 0;
        room.answers.clear();
        room.completions.clear();
        room.truth_or_dare_states.clear();
        room.truth_or_dare_revealed = false;
        room.truth_or_dare_phase =
            (room.game_id == "truth-or-dare").then_some(TruthOrDarePhase::Choosing);
        room.spotlight_order.clear();
        room.spotlight_index = 0;
        room.rerolls_used.clear();
        room.reactions.clear();
        room.highlight_categories.clear();
        room.highlight_votes.clear();
        room.highlight_skips.clear();
        room.highlight_results.clear();
        room.historical_highlights.clear();
        room.reaction_totals.clear();
        room.truth_prompts = truth_prompts;
        room.dare_prompts = dare_prompts;
        room.used_truth_prompts.clear();
        room.used_dare_prompts.clear();
        room.scores = room.players.iter().map(|player| (player.id, 0)).collect();
        room.current_round_points.clear();
        room.round_history.clear();
        room.active_player_id = None;
        room.phase = GamePhase::Lobby;
        room.notice = Some(crate::model::RoomNotice {
            id: room.version + 1,
            message: format!("{} is ready!", room.game_title),
            kind: "game_selected".into(),
        });
        Self::changed(&mut room);
        Ok(())
    }

    pub async fn end_room_by_player(&self, code: &str, player_id: Uuid) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_player_host(&room, player_id)?;
        room.phase = GamePhase::Closed;
        room.notice = Some(crate::model::RoomNotice {
            id: room.version + 1,
            message: "The host ended the room. Thanks for playing!".into(),
            kind: "room_ended".into(),
        });
        room.expires_at = std::time::Instant::now() + Duration::from_secs(10);
        Self::changed(&mut room);
        Ok(())
    }

    pub async fn start_game_by_player(&self, code: &str, player_id: Uuid) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_player_host(&room, player_id)?;
        Self::start_game_locked(&mut room)
    }

    fn start_game_locked(room: &mut Room) -> Result<(), GameError> {
        if room.phase != GamePhase::Lobby {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Game is not in the lobby.",
            ));
        }
        if room.players.len() < 2 {
            return Err(GameError::new(
                ErrorCode::NotEnoughPlayers,
                "At least two players are required.",
            ));
        }
        room.current_round = 1;
        room.phase = GamePhase::Playing;
        room.answers.clear();
        room.completions.clear();
        room.truth_or_dare_states.clear();
        room.truth_or_dare_revealed = false;
        room.truth_or_dare_phase =
            (room.game_id == "truth-or-dare").then_some(TruthOrDarePhase::Choosing);
        room.spotlight_order.clear();
        room.spotlight_index = 0;
        room.reactions.clear();
        room.highlight_categories.clear();
        room.highlight_votes.clear();
        room.highlight_skips.clear();
        room.highlight_results.clear();
        Self::load_current_question(room);
        Self::changed(room);
        Ok(())
    }

    pub async fn start_game(&self, code: &str, token: &str) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_host(&room, token)?;
        Self::start_game_locked(&mut room)
    }

    pub async fn submit_answer(
        &self,
        code: &str,
        player_id: Uuid,
        round: u8,
        answer: &str,
    ) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        if room.phase != GamePhase::Playing {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Answers are not open.",
            ));
        }
        if room.game_id == "truth-or-dare"
            && room.truth_or_dare_phase != Some(TruthOrDarePhase::Choosing)
        {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Truth or Dare choices are closed.",
            ));
        }
        if round != room.current_round {
            return Err(GameError::new(
                ErrorCode::RoundMismatch,
                "Round does not match.",
            ));
        }
        if !room.players.iter().any(|player| player.id == player_id) {
            return Err(GameError::new(
                ErrorCode::Unauthorized,
                "Player is not in the room.",
            ));
        }
        if room.answers.contains_key(&player_id) {
            return Err(GameError::new(
                ErrorCode::AlreadyAnswered,
                "Your answer is already locked.",
            ));
        }
        let stored_answer = if room.game_id == "truth-or-dare" {
            let choice = answer.trim().to_ascii_lowercase();
            if !matches!(choice.as_str(), "truth" | "dare") {
                return Err(GameError::new(
                    ErrorCode::InvalidAnswer,
                    "Choose either Truth or Dare.",
                ));
            }
            let state = Self::assign_truth_or_dare_prompt(&mut room, player_id, &choice)?;
            room.truth_or_dare_states.insert(player_id, state);
            choice
        } else {
            Self::validate_answer(&room, answer)?;
            answer.to_owned()
        };
        room.answers.insert(player_id, stored_answer);
        if room.game_id == "truth-or-dare" && room.answers.len() == room.players.len() {
            room.truth_or_dare_phase = Some(TruthOrDarePhase::PreparingReveal);
        }
        Self::changed(&mut room);
        Ok(())
    }

    fn assign_truth_or_dare_prompt(
        room: &mut Room,
        player_id: Uuid,
        choice: &str,
    ) -> Result<TruthOrDarePlayerState, GameError> {
        let pool = if choice == "truth" {
            &room.truth_prompts
        } else {
            &room.dare_prompts
        };
        if pool.is_empty() {
            return Err(GameError::new(
                ErrorCode::InternalError,
                "No prompts are available for that choice.",
            ));
        }
        let used_for_player = if choice == "truth" {
            room.used_truth_prompts.get(&player_id)
        } else {
            room.used_dare_prompts.get(&player_id)
        };
        let occupied: HashSet<_> = room
            .truth_or_dare_states
            .values()
            .map(|state| state.prompt.as_str())
            .collect();
        let mut candidates: Vec<_> = pool
            .iter()
            .enumerate()
            .filter(|(_, prompt)| {
                !occupied.contains(prompt.as_str())
                    && used_for_player.is_none_or(|used| !used.contains(prompt.as_str()))
            })
            .collect();
        if candidates.is_empty() {
            candidates = pool
                .iter()
                .enumerate()
                .filter(|(_, prompt)| !occupied.contains(prompt.as_str()))
                .collect();
        }
        if candidates.is_empty() {
            candidates = pool.iter().enumerate().collect();
        }
        let (index, prompt) = candidates
            .choose(&mut OsRng)
            .copied()
            .expect("non-empty prompt pool");
        let prompt = prompt.clone();
        let used = if choice == "truth" {
            room.used_truth_prompts.entry(player_id).or_default()
        } else {
            room.used_dare_prompts.entry(player_id).or_default()
        };
        if used.len() >= pool.len() {
            used.clear();
        }
        used.insert(prompt.clone());
        Ok(TruthOrDarePlayerState {
            choice: choice.into(),
            prompt_id: format!("{choice}-prompt-{index}"),
            prompt,
            completed: false,
            skipped: false,
        })
    }

    pub async fn submit_vote(
        &self,
        code: &str,
        player_id: Uuid,
        round: u8,
        selected_player_id: Uuid,
    ) -> Result<(), GameError> {
        self.submit_answer(code, player_id, round, &selected_player_id.to_string())
            .await
            .map_err(|mut error| {
                if error.code == ErrorCode::AlreadyAnswered {
                    error.code = ErrorCode::AlreadyVoted;
                }
                error
            })
    }

    fn validate_answer(room: &Room, answer: &str) -> Result<(), GameError> {
        match room.response_mode {
            ResponseMode::PlayerVote => {
                let selected = Uuid::parse_str(answer)
                    .map_err(|_| GameError::new(ErrorCode::InvalidAnswer, "Choose a player."))?;
                if !room.players.iter().any(|player| player.id == selected) {
                    return Err(GameError::new(
                        ErrorCode::InvalidPlayer,
                        "Selected player is not in the room.",
                    ));
                }
            }
            ResponseMode::Choice | ResponseMode::Quiz => {
                let index = answer.parse::<usize>().map_err(|_| {
                    GameError::new(
                        ErrorCode::InvalidAnswer,
                        "Choose one of the available answers.",
                    )
                })?;
                let valid = room
                    .current_question
                    .as_ref()
                    .is_some_and(|question| index < question.options.len());
                if !valid {
                    return Err(GameError::new(
                        ErrorCode::InvalidAnswer,
                        "Choose one of the available answers.",
                    ));
                }
            }
        }
        Ok(())
    }

    pub async fn reveal_results(&self, code: &str, token: &str) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_host(&room, token)?;
        Self::reveal_results_locked(&mut room)
    }

    pub async fn reveal_results_by_player(
        &self,
        code: &str,
        player_id: Uuid,
    ) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_player_host(&room, player_id)?;
        Self::reveal_results_locked(&mut room)
    }

    fn reveal_results_locked(room: &mut Room) -> Result<(), GameError> {
        if room.phase != GamePhase::Playing {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Game is not accepting answers.",
            ));
        }
        if room.answers.len() < room.players.len() {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Everyone must lock an answer before results are revealed.",
            ));
        }
        if room.game_id == "truth-or-dare" {
            if room.truth_or_dare_phase != Some(TruthOrDarePhase::PreparingReveal) {
                return Err(GameError::new(
                    ErrorCode::InvalidPhase,
                    "The Spotlight is not ready yet.",
                ));
            }
            room.spotlight_order = room.players.iter().map(|player| player.id).collect();
            room.spotlight_order.shuffle(&mut OsRng);
            room.spotlight_index = 0;
            room.truth_or_dare_revealed = true;
            room.truth_or_dare_phase = Some(TruthOrDarePhase::Spotlight);
            room.reactions.clear();
            Self::changed(room);
            return Ok(());
        }
        Self::apply_scores(room);
        room.phase = GamePhase::Results;
        let question = room.current_question.clone();
        let round = room.current_round;
        let results = room.calculate_results();
        room.round_history.push(RoundSummary {
            round,
            question: question
                .as_ref()
                .map(|q| q.prompt.clone())
                .unwrap_or_default(),
            category: question
                .as_ref()
                .map(|q| q.category.clone())
                .unwrap_or_default(),
            results,
        });
        Self::changed(room);
        Ok(())
    }

    fn apply_scores(room: &mut Room) {
        room.current_round_points.clear();
        if room.experience != GameExperience::Competitive {
            return;
        }
        match room.response_mode {
            ResponseMode::PlayerVote => {
                for answer in room.answers.values() {
                    if let Ok(selected) = Uuid::parse_str(answer) {
                        *room.scores.entry(selected).or_default() += 100;
                        *room.current_round_points.entry(selected).or_default() += 100;
                    }
                }
            }
            ResponseMode::Quiz => {
                let correct = room
                    .current_question
                    .as_ref()
                    .and_then(|question| question.correct_option);
                for (player, answer) in &room.answers {
                    if answer.parse::<usize>().ok() == correct {
                        *room.scores.entry(*player).or_default() += 100;
                        room.current_round_points.insert(*player, 100);
                    }
                }
            }
            ResponseMode::Choice => {}
        }
    }

    pub async fn mark_completed(
        &self,
        code: &str,
        player_id: Uuid,
        round: u8,
    ) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        if room.game_id != "truth-or-dare"
            || room.phase != GamePhase::Playing
            || round != room.current_round
        {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "There is no active challenge to complete.",
            ));
        }
        if !room.players.iter().any(|player| player.id == player_id) {
            return Err(GameError::new(
                ErrorCode::Unauthorized,
                "Player is not in the room.",
            ));
        }
        if room.truth_or_dare_phase != Some(TruthOrDarePhase::Spotlight) {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "There is no player in the Spotlight.",
            ));
        }
        if room.spotlight_order.get(room.spotlight_index).copied() != Some(player_id) {
            return Err(GameError::new(
                ErrorCode::Unauthorized,
                "Only the Spotlight player can complete this prompt.",
            ));
        }
        let player_state = room
            .truth_or_dare_states
            .get_mut(&player_id)
            .ok_or_else(|| GameError::new(ErrorCode::InvalidPhase, "Lock a choice first."))?;
        if player_state.completed {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "You already completed this prompt.",
            ));
        }
        player_state.completed = true;
        room.completions.insert(player_id);
        Self::changed(&mut room);
        Ok(())
    }

    pub async fn reroll_spotlight(
        &self,
        code: &str,
        player_id: Uuid,
        round: u8,
    ) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_spotlight_actor(&room, player_id, round)?;
        if room.rerolls_used.contains(&player_id) {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Your free reroll has already been used.",
            ));
        }
        let choice = room
            .truth_or_dare_states
            .get(&player_id)
            .map(|state| state.choice.clone())
            .ok_or_else(|| GameError::new(ErrorCode::InvalidPhase, "No prompt is assigned."))?;
        let replacement = Self::assign_truth_or_dare_prompt(&mut room, player_id, &choice)?;
        room.truth_or_dare_states.insert(player_id, replacement);
        room.rerolls_used.insert(player_id);
        Self::changed(&mut room);
        Ok(())
    }

    pub async fn skip_spotlight(
        &self,
        code: &str,
        player_id: Uuid,
        round: u8,
    ) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_spotlight_actor(&room, player_id, round)?;
        let state = room
            .truth_or_dare_states
            .get_mut(&player_id)
            .ok_or_else(|| GameError::new(ErrorCode::InvalidPhase, "No prompt is assigned."))?;
        if state.completed || state.skipped {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "This turn is already finished.",
            ));
        }
        state.skipped = true;
        room.completions.insert(player_id);
        Self::changed(&mut room);
        Ok(())
    }

    fn authorize_spotlight_actor(room: &Room, player_id: Uuid, round: u8) -> Result<(), GameError> {
        if room.game_id != "truth-or-dare"
            || room.phase != GamePhase::Playing
            || room.truth_or_dare_phase != Some(TruthOrDarePhase::Spotlight)
            || room.current_round != round
        {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "There is no active Spotlight turn.",
            ));
        }
        if room.spotlight_order.get(room.spotlight_index).copied() != Some(player_id) {
            return Err(GameError::new(
                ErrorCode::Unauthorized,
                "Only the Spotlight player can do that.",
            ));
        }
        Ok(())
    }

    pub async fn next_spotlight(
        &self,
        code: &str,
        host_id: Uuid,
        round: u8,
    ) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_player_host(&room, host_id)?;
        if room.current_round != round
            || room.truth_or_dare_phase != Some(TruthOrDarePhase::Spotlight)
        {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "There is no active Spotlight turn.",
            ));
        }
        let current = room
            .spotlight_order
            .get(room.spotlight_index)
            .copied()
            .ok_or_else(|| {
                GameError::new(ErrorCode::InvalidPhase, "The Spotlight order is empty.")
            })?;
        let finished = room
            .truth_or_dare_states
            .get(&current)
            .is_some_and(|state| state.completed || state.skipped);
        if !finished {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Wait for the Spotlight player to complete or skip.",
            ));
        }
        if room.spotlight_index + 1 < room.spotlight_order.len() {
            room.spotlight_index += 1;
            room.reactions.clear();
        } else {
            Self::prepare_highlight_voting(&mut room);
        }
        Self::changed(&mut room);
        Ok(())
    }

    fn prepare_highlight_voting(room: &mut Room) {
        let completed_truths: Vec<_> = room
            .truth_or_dare_states
            .iter()
            .filter(|(_, state)| state.completed && !state.skipped && state.choice == "truth")
            .map(|(id, _)| *id)
            .collect();
        let completed_dares: Vec<_> = room
            .truth_or_dare_states
            .iter()
            .filter(|(_, state)| state.completed && !state.skipped && state.choice == "dare")
            .map(|(id, _)| *id)
            .collect();
        let completed_all: Vec<_> = room
            .truth_or_dare_states
            .iter()
            .filter(|(_, state)| state.completed && !state.skipped)
            .map(|(id, _)| *id)
            .collect();
        room.highlight_categories.clear();
        if completed_dares.len() >= 2 {
            room.highlight_categories.push(HighlightCategory {
                id: "most-daring".into(),
                label: "Most Daring".into(),
                emoji: "🔥".into(),
                eligible_player_ids: completed_dares,
            });
        }
        if completed_truths.len() >= 2 {
            room.highlight_categories.push(HighlightCategory {
                id: "best-truth".into(),
                label: "Best Truth".into(),
                emoji: "💬".into(),
                eligible_player_ids: completed_truths,
            });
        }
        if completed_all.len() >= 2 && room.highlight_categories.len() < 3 {
            room.highlight_categories.push(HighlightCategory {
                id: "crowd-favourite".into(),
                label: "Crowd Favourite".into(),
                emoji: "👏".into(),
                eligible_player_ids: completed_all,
            });
        }
        room.highlight_votes.clear();
        room.highlight_skips.clear();
        room.highlight_results.clear();
        room.truth_or_dare_phase = if room.highlight_categories.is_empty() {
            Some(TruthOrDarePhase::RoundComplete)
        } else {
            Some(TruthOrDarePhase::HighlightVoting)
        };
    }

    pub async fn send_reaction(
        &self,
        code: &str,
        player_id: Uuid,
        round: u8,
        emoji: &str,
        reaction_id: Uuid,
    ) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        if room.current_round != round
            || room.truth_or_dare_phase != Some(TruthOrDarePhase::Spotlight)
            || !room.players.iter().any(|player| player.id == player_id)
        {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Reactions are not open.",
            ));
        }
        if !matches!(emoji, "😂" | "😱" | "👏" | "🔥" | "❤️") {
            return Err(GameError::new(
                ErrorCode::InvalidAnswer,
                "Choose an available reaction.",
            ));
        }
        if room
            .reactions
            .iter()
            .any(|reaction| reaction.id == reaction_id)
        {
            return Ok(());
        }
        let count = room
            .reactions
            .iter()
            .filter(|reaction| {
                reaction.player_id == player_id && reaction.spotlight_index == room.spotlight_index
            })
            .count();
        if count >= 5 {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Take a moment before reacting again.",
            ));
        }
        let spotlight_index = room.spotlight_index;
        room.reactions.push(SpotlightReaction {
            id: reaction_id,
            player_id,
            emoji: emoji.into(),
            spotlight_index,
        });
        if let Some(spotlight_player) = room.spotlight_order.get(room.spotlight_index).copied() {
            *room.reaction_totals.entry(spotlight_player).or_default() += 1;
        }
        Self::changed(&mut room);
        Ok(())
    }

    pub async fn force_skip_spotlight(
        &self,
        code: &str,
        host_id: Uuid,
        round: u8,
    ) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_player_host(&room, host_id)?;
        if room.current_round != round
            || room.truth_or_dare_phase != Some(TruthOrDarePhase::Spotlight)
        {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "There is no active Spotlight turn.",
            ));
        }
        let current = room
            .spotlight_order
            .get(room.spotlight_index)
            .copied()
            .ok_or_else(|| GameError::new(ErrorCode::InvalidPhase, "The Spotlight is empty."))?;
        let state = room
            .truth_or_dare_states
            .get_mut(&current)
            .ok_or_else(|| GameError::new(ErrorCode::InvalidPhase, "No prompt is assigned."))?;
        state.skipped = true;
        room.completions.insert(current);
        Self::changed(&mut room);
        Ok(())
    }

    pub async fn submit_highlight_vote(
        &self,
        code: &str,
        player_id: Uuid,
        round: u8,
        category_id: &str,
        selected_player_id: Uuid,
    ) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        if room.current_round != round
            || room.truth_or_dare_phase != Some(TruthOrDarePhase::HighlightVoting)
            || !room.players.iter().any(|player| player.id == player_id)
        {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Highlight voting is not open.",
            ));
        }
        if player_id == selected_player_id {
            return Err(GameError::new(
                ErrorCode::InvalidAnswer,
                "Choose someone else for this highlight.",
            ));
        }
        let category = room
            .highlight_categories
            .iter()
            .find(|category| category.id == category_id)
            .ok_or_else(|| {
                GameError::new(ErrorCode::InvalidAnswer, "That highlight is unavailable.")
            })?;
        if !category.eligible_player_ids.contains(&selected_player_id) {
            return Err(GameError::new(
                ErrorCode::InvalidPlayer,
                "That player is not eligible for this highlight.",
            ));
        }
        let votes = room.highlight_votes.entry(category_id.into()).or_default();
        if votes.contains_key(&player_id) {
            return Err(GameError::new(
                ErrorCode::AlreadyVoted,
                "Your highlight vote is already locked.",
            ));
        }
        votes.insert(player_id, selected_player_id);
        Self::changed(&mut room);
        Ok(())
    }

    pub async fn skip_highlight_voting(
        &self,
        code: &str,
        player_id: Uuid,
        round: u8,
    ) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        if room.current_round != round
            || room.truth_or_dare_phase != Some(TruthOrDarePhase::HighlightVoting)
            || !room.players.iter().any(|player| player.id == player_id)
        {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Highlight voting is not open.",
            ));
        }
        room.highlight_skips.insert(player_id);
        Self::changed(&mut room);
        Ok(())
    }

    pub async fn reveal_highlights(
        &self,
        code: &str,
        host_id: Uuid,
        round: u8,
    ) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_player_host(&room, host_id)?;
        if room.current_round != round
            || room.truth_or_dare_phase != Some(TruthOrDarePhase::HighlightVoting)
        {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Highlight voting is not open.",
            ));
        }
        let ready = room.players.iter().all(|player| {
            room.highlight_skips.contains(&player.id)
                || room.highlight_categories.iter().all(|category| {
                    room.highlight_votes
                        .get(&category.id)
                        .is_some_and(|votes| votes.contains_key(&player.id))
                })
        });
        if !ready {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Wait for everyone to vote or skip.",
            ));
        }
        room.highlight_results = room
            .highlight_categories
            .iter()
            .filter_map(|category| {
                let mut totals: HashMap<Uuid, u32> = HashMap::new();
                for selected in room
                    .highlight_votes
                    .get(&category.id)
                    .into_iter()
                    .flat_map(|votes| votes.values())
                {
                    *totals.entry(*selected).or_default() += 1;
                }
                let max = totals.values().copied().max()?;
                if max == 0 {
                    return None;
                }
                let mut winners: Vec<_> = totals
                    .into_iter()
                    .filter_map(|(id, votes)| (votes == max).then_some(id))
                    .collect();
                winners.sort();
                Some(HighlightResult {
                    category_id: category.id.clone(),
                    label: category.label.clone(),
                    emoji: category.emoji.clone(),
                    winner_player_ids: winners,
                    votes: max,
                })
            })
            .collect();
        let revealed_highlights = room.highlight_results.clone();
        room.historical_highlights.extend(revealed_highlights);
        room.truth_or_dare_phase = Some(TruthOrDarePhase::HighlightResults);
        Self::changed(&mut room);
        Ok(())
    }

    pub async fn next_round(&self, code: &str, token: &str) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_host(&room, token)?;
        Self::next_round_locked(&mut room)
    }

    pub async fn next_round_by_player(&self, code: &str, player_id: Uuid) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_player_host(&room, player_id)?;
        Self::next_round_locked(&mut room)
    }

    fn next_round_locked(room: &mut Room) -> Result<(), GameError> {
        let truth_or_dare_ready = room.game_id == "truth-or-dare"
            && room.phase == GamePhase::Playing
            && matches!(
                room.truth_or_dare_phase,
                Some(TruthOrDarePhase::HighlightResults | TruthOrDarePhase::RoundComplete)
            );
        if room.phase != GamePhase::Results && !truth_or_dare_ready {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                if room.game_id == "truth-or-dare" {
                    "Finish the Spotlight highlights first."
                } else {
                    "Results are not showing."
                },
            ));
        }
        if truth_or_dare_ready {
            Self::record_current_round(room);
        }
        Self::advance_round(room);
        Self::changed(room);
        Ok(())
    }

    fn advance_round(room: &mut Room) {
        if room.current_round >= room.max_rounds {
            room.phase = GamePhase::Finished;
        } else {
            room.current_round += 1;
            room.phase = GamePhase::Playing;
            room.answers.clear();
            room.completions.clear();
            room.current_round_points.clear();
            room.truth_or_dare_states.clear();
            room.truth_or_dare_revealed = false;
            room.truth_or_dare_phase =
                (room.game_id == "truth-or-dare").then_some(TruthOrDarePhase::Choosing);
            room.spotlight_order.clear();
            room.spotlight_index = 0;
            room.reactions.clear();
            room.highlight_categories.clear();
            room.highlight_votes.clear();
            room.highlight_skips.clear();
            room.highlight_results.clear();
            Self::load_current_question(room);
        }
    }

    fn record_current_round(room: &mut Room) {
        let question = room.current_question.clone();
        room.round_history.push(RoundSummary {
            round: room.current_round,
            question: question
                .as_ref()
                .map(|value| value.prompt.clone())
                .unwrap_or_default(),
            category: question
                .as_ref()
                .map(|value| value.category.clone())
                .unwrap_or_default(),
            results: room.calculate_results(),
        });
    }

    fn load_current_question(room: &mut Room) {
        room.current_question = room
            .question_queue
            .get((room.current_round - 1) as usize)
            .cloned();
        room.active_player_id = None;
    }

    pub async fn kick_player(
        &self,
        code: &str,
        token: &str,
        player_id: Uuid,
    ) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_host(&room, token)?;
        if player_id == room.host_player_id {
            return Err(GameError::new(
                ErrorCode::Unauthorized,
                "The host cannot remove themselves.",
            ));
        }
        if room.phase != GamePhase::Lobby {
            return Err(GameError::new(
                ErrorCode::InvalidPhase,
                "Players can only be removed in the lobby.",
            ));
        }
        let before = room.players.len();
        room.players.retain(|player| player.id != player_id);
        if before == room.players.len() {
            return Err(GameError::new(
                ErrorCode::InvalidPlayer,
                "Player was not found.",
            ));
        }
        room.scores.remove(&player_id);
        Self::changed(&mut room);
        Ok(())
    }

    pub async fn kick_player_by_player(
        &self,
        code: &str,
        host_id: Uuid,
        player_id: Uuid,
    ) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_player_host(&room, host_id)?;
        if player_id == room.host_player_id {
            return Err(GameError::new(
                ErrorCode::Unauthorized,
                "The host cannot remove themselves.",
            ));
        }
        let nickname = Self::remove_player_state(&mut room, player_id)
            .ok_or_else(|| GameError::new(ErrorCode::InvalidPlayer, "Player was not found."))?;
        room.notice = Some(crate::model::RoomNotice {
            id: room.version + 1,
            message: format!("{nickname} was removed from the party."),
            kind: "player_left".into(),
        });
        Self::changed(&mut room);
        Ok(())
    }

    pub async fn end_game(&self, code: &str, token: &str) -> Result<(), GameError> {
        let room = self.room(code)?;
        let mut room = room.write().await;
        Self::authorize_host(&room, token)?;
        room.phase = GamePhase::Finished;
        Self::changed(&mut room);
        Ok(())
    }

    fn authorize_host(room: &Room, token: &str) -> Result<(), GameError> {
        if room.host_token != token {
            return Err(GameError::new(
                ErrorCode::Unauthorized,
                "Host authorization required.",
            ));
        }
        if !room.host_connected {
            return Err(GameError::new(
                ErrorCode::HostDisconnected,
                "Host is disconnected.",
            ));
        }
        Ok(())
    }

    fn changed(room: &mut Room) {
        room.version += 1;
        room.last_activity = std::time::Instant::now();
        if room.players.iter().any(|player| player.connected) {
            room.expires_at = room.last_activity + ACTIVE_ROOM_LIFETIME;
        }
        let _ = room.updates.send(room.snapshot(None));
    }

    pub async fn cleanup_once(&self) {
        self.cleanup_at(std::time::Instant::now()).await;
    }

    pub async fn cleanup_at(&self, now: std::time::Instant) {
        let entries: Vec<_> = self
            .rooms
            .iter()
            .map(|entry| (entry.key().clone(), Arc::clone(entry.value())))
            .collect();
        for (code, room) in entries {
            let mut room = room.write().await;
            let expired_players: Vec<_> = room
                .players
                .iter()
                .filter(|player| {
                    !player.connected && now.duration_since(player.last_seen) >= DISCONNECT_GRACE
                })
                .map(|player| player.id)
                .collect();
            for player_id in expired_players {
                let was_host = room.host_player_id == player_id;
                if let Some(name) = Self::remove_player_state(&mut room, player_id) {
                    if was_host && !room.players.is_empty() {
                        room.host_player_id = room.players[0].id;
                        room.host_token = Self::generate_token();
                        room.host_connected = room.players[0].connected;
                        let next = room.players[0].nickname.clone();
                        room.notice = Some(crate::model::RoomNotice {
                            id: room.version + 1,
                            message: format!("{name} disconnected. {next} is now the host."),
                            kind: "host_transferred".into(),
                        });
                    } else {
                        room.notice = Some(crate::model::RoomNotice {
                            id: room.version + 1,
                            message: format!("{name} left the game."),
                            kind: "player_left".into(),
                        });
                    }
                    Self::changed(&mut room);
                }
            }
            let expired = now >= room.expires_at
                || now.duration_since(room.last_activity) >= ACTIVE_ROOM_LIFETIME;
            drop(room);
            if expired {
                self.rooms.remove(&code);
            }
        }
    }

    pub async fn room_count(&self) -> usize {
        self.rooms.len()
    }
}
