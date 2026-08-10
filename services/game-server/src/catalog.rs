use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseMode {
    PlayerVote,
    Choice,
    Quiz,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameExperience {
    #[default]
    Competitive,
    Voting,
    Casual,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub prompt: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default = "default_difficulty")]
    pub difficulty: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub visual_options: Vec<String>,
    #[serde(default)]
    pub correct_option: Option<usize>,
}

fn default_category() -> String {
    "party".into()
}

fn default_difficulty() -> String {
    "easy".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDefinition {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    #[serde(default = "default_game_category")]
    pub category: String,
    #[serde(default = "default_estimated_minutes")]
    pub estimated_minutes: u8,
    pub response_mode: ResponseMode,
    #[serde(default)]
    pub experience: GameExperience,
    #[serde(default = "default_rounds")]
    pub default_rounds: u8,
    #[serde(default = "default_min_players")]
    pub min_players: usize,
    pub questions: Vec<Question>,
}

fn default_rounds() -> u8 {
    5
}

fn default_min_players() -> usize {
    2
}

fn default_game_category() -> String {
    "Party".into()
}
fn default_estimated_minutes() -> u8 {
    10
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSummary {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub category: String,
    pub estimated_minutes: u8,
    pub response_mode: ResponseMode,
    pub experience: GameExperience,
    pub default_rounds: u8,
    pub min_players: usize,
    pub question_count: usize,
}

impl From<&GameDefinition> for GameSummary {
    fn from(game: &GameDefinition) -> Self {
        Self {
            id: game.id.clone(),
            title: game.title.clone(),
            description: game.description.clone(),
            icon: game.icon.clone(),
            category: game.category.clone(),
            estimated_minutes: game.estimated_minutes,
            response_mode: game.response_mode,
            experience: game.experience,
            default_rounds: game.default_rounds,
            min_players: game.min_players,
            question_count: game.questions.len(),
        }
    }
}

pub fn load_default_catalog() -> Vec<GameDefinition> {
    serde_json::from_str(include_str!("../../../question-packs/games.json"))
        .expect("game catalog must be valid JSON")
}

pub fn priority_sync_from_strings(questions: Vec<String>) -> GameDefinition {
    GameDefinition {
        id: "priority-sync".into(),
        title: "Priority Sync".into(),
        description: "Vote for the friend who fits the prompt best.".into(),
        icon: "spark".into(),
        category: "Friends".into(),
        estimated_minutes: 10,
        response_mode: ResponseMode::PlayerVote,
        experience: GameExperience::Competitive,
        default_rounds: 5,
        min_players: 2,
        questions: questions
            .into_iter()
            .map(|prompt| Question {
                prompt,
                category: "friends".into(),
                difficulty: "easy".into(),
                options: vec![],
                visual_options: vec![],
                correct_option: None,
            })
            .collect(),
    }
}
