export type GamePhase = "lobby" | "playing" | "results" | "finished" | "choosing" | "closed";
export type ResponseMode = "player_vote" | "choice" | "quiz";
export type GameExperience = "competitive" | "voting" | "casual";

export interface GameSummary {
  id: string;
  title: string;
  description: string;
  icon: string;
  category: string;
  estimatedMinutes: number;
  responseMode: ResponseMode;
  experience: GameExperience;
  defaultRounds: number;
  minPlayers: number;
  questionCount: number;
}

export interface Player { id: string; nickname: string; connected: boolean; role?: "host" | "player" }
export type TruthOrDarePhase = "choosing" | "preparing_reveal" | "spotlight" | "highlight_voting" | "highlight_results" | "round_complete";
export interface SpotlightReaction { id: string; playerId: string; emoji: string; spotlightIndex: number }
export interface HighlightCategory { id: string; label: string; emoji: string; eligiblePlayerIds: string[] }
export interface HighlightResult { categoryId: string; label: string; emoji: string; winnerPlayerIds: string[]; votes: number }
export interface RoundResult { id: string; label: string; votes: number; playerId?: string; isCorrect?: boolean }
export interface LeaderboardEntry { playerId: string; nickname: string; score: number; rank: number }
export interface RoundSummary { round: number; question: string; category: string; results: RoundResult[] }
export interface RoundPoint { playerId: string; nickname: string; points: number }

export interface RoomSnapshot {
  version: number;
  roomCode: string;
  gameId: string;
  gameTitle: string;
  responseMode: ResponseMode;
  experience: GameExperience;
  phase: GamePhase;
  round: number;
  maxRounds: number;
  currentQuestion: string | null;
  currentCategory: string | null;
  currentDifficulty: string | null;
  currentOptions: string[];
  currentVisualOptions: string[];
  theme: string;
  roundPoints: RoundPoint[];
  activePlayerId: string | null;
  selectedAnswer: string | null;
  completedCount: number;
  truthOrDareRevealed?: boolean;
  truthOrDarePhase?: TruthOrDarePhase;
  spotlightOrder?: string[];
  spotlightIndex?: number;
  spotlightPlayerId?: string;
  spotlightPlayerName?: string;
  spotlightChoice?: "truth" | "dare";
  spotlightPrompt?: string;
  spotlightCompleted?: boolean;
  spotlightSkipped?: boolean;
  viewerRerollAvailable?: boolean;
  reactions?: SpotlightReaction[];
  highlightCategories?: HighlightCategory[];
  highlightResults?: HighlightResult[];
  historicalHighlights?: HighlightResult[];
  reactionTotals?: Record<string, number>;
  viewerHighlightVotes?: Record<string, string>;
  viewerSkippedHighlightVoting?: boolean;
  highlightReadyCount?: number;
  players: Player[];
  submittedAnswerCount: number;
  totalEligiblePlayerCount: number;
  results: RoundResult[] | null;
  leaderboard: LeaderboardEntry[];
  roundHistory: RoundSummary[] | null;
  hostConnected: boolean;
  hostPlayerId: string;
  notice?: { id: number; message: string; kind: string } | null;
  viewerHasAnswered?: boolean;
  viewerPlayerId?: string;
  viewerAnswer?: string;
  viewerHasCompleted?: boolean;
  viewerTruthOrDareChoice?: "truth" | "dare";
  viewerTruthOrDarePromptId?: string;
  viewerTruthOrDarePrompt?: string;
}

export interface HostCredentials { roomCode: string; hostToken: string }
export type ServerEvent =
  | { type: "authenticated"; role: "host" }
  | { type: "room_snapshot"; snapshot: RoomSnapshot }
  | { type: "error"; code: string; message: string }
  | { type: "pong" };
