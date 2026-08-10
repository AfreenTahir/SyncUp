import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { GameView, validateJoin } from "./App";
import type { RoomSnapshot } from "./types";

const snapshot: RoomSnapshot = {
  version: 1,
  roomCode: "ABC234",
  gameId: "priority-sync",
  gameTitle: "Priority Sync",
  responseMode: "player_vote",
  experience: "competitive",
  phase: "lobby",
  round: 0,
  maxRounds: 5,
  currentQuestion: null,
  currentCategory: null,
  currentDifficulty: null,
  currentOptions: [],
  currentVisualOptions: [],
  theme: "neon-night",
  roundPoints: [],
  activePlayerId: null,
  selectedAnswer: null,
  completedCount: 0,
  players: [
    { id: "afreen-id", nickname: "Afreen", connected: true },
    { id: "sam-id", nickname: "Sam", connected: false },
  ],
  submittedAnswerCount: 0,
  totalEligiblePlayerCount: 2,
  results: null,
  leaderboard: [],
  roundHistory: null,
  hostConnected: true,
  hostPlayerId: "host-player",
};

describe("join validation", () => {
  it("accepts valid input and rejects ambiguous codes and bad names", () => {
    expect(validateJoin("abc234", " Afreen ")).toBeNull();
    expect(validateJoin("ABO234", "Afreen")).toMatch(/six-character/);
    expect(validateJoin("ABC234", "   ")).toMatch(/Display name/);
  });
});

it("renders lobby snapshots", () => {
  render(<GameView snapshot={snapshot} onSend={vi.fn()} />);
  expect(screen.getByText("ABC234")).toBeInTheDocument();
  expect(screen.getByText("Afreen")).toBeInTheDocument();
  expect(screen.getByText("Reconnecting")).toBeInTheDocument();
});

it("selects and locks a vote", async () => {
  const user = userEvent.setup();
  const send = vi.fn();
  render(<GameView snapshot={{ ...snapshot, phase: "playing", round: 1, currentQuestion: "Who is ready?" }} onSend={send} />);
  const confirm = screen.getByRole("button", { name: /Confirm vote/ });
  expect(confirm).toBeDisabled();
  await user.click(screen.getByRole("radio", { name: /Sam/ }));
  expect(confirm).toBeEnabled();
  await user.click(confirm);
  await user.click(screen.getByRole("button", { name: "Lock it in" }));
  expect(send).toHaveBeenCalledWith({ type: "submit_answer", round: 1, answer: "sam-id" });
});

it("renders choice games from server-provided options", async () => {
  const user = userEvent.setup();
  const send = vi.fn();
  render(<GameView snapshot={{ ...snapshot, gameId: "this-or-that", gameTitle: "This or That", responseMode: "choice", phase: "playing", round: 1, currentQuestion: "Pick one", currentOptions: ["Tea", "Coffee"] }} onSend={send} />);
  await user.click(screen.getByRole("radio", { name: /Coffee/ }));
  await user.click(screen.getByRole("button", { name: /Lock answer/ }));
  await user.click(screen.getByRole("button", { name: "Lock it in" }));
  expect(send).toHaveBeenCalledWith({ type: "submit_answer", round: 1, answer: "1" });
});

it("locks a private choice and then shows the shared Spotlight prompt", async () => {
  const user = userEvent.setup();
  const send = vi.fn();
  const truthOrDare: RoomSnapshot = {
    ...snapshot,
    gameId: "truth-or-dare",
    gameTitle: "Truth or Dare",
    responseMode: "choice",
    experience: "casual",
    phase: "playing",
    round: 1,
    currentQuestion: "Truth or Dare?",
    currentOptions: ["Truth", "Dare"],
    viewerPlayerId: "afreen-id",
    truthOrDareRevealed: false,
    truthOrDarePhase: "choosing",
  };
  const view = render(<GameView snapshot={truthOrDare} onSend={send} />);
  const lock = screen.getByRole("button", { name: /Lock choice/ });
  expect(lock).toBeDisabled();
  await user.click(screen.getByRole("radio", { name: /Dare/ }));
  await user.click(lock);
  expect(send).toHaveBeenCalledWith({ type: "submit_answer", round: 1, answer: "dare" });

  view.rerender(<GameView snapshot={{ ...truthOrDare, viewerHasAnswered: true, viewerTruthOrDareChoice: "dare", submittedAnswerCount: 1 }} onSend={send} />);
  expect(screen.getByText("Dare locked!")).toBeInTheDocument();
  expect(screen.queryByText("Do a victory dance")).not.toBeInTheDocument();

  view.rerender(<GameView snapshot={{ ...truthOrDare, viewerHasAnswered: true, viewerTruthOrDareChoice: "dare", truthOrDareRevealed: true, truthOrDarePhase: "spotlight", spotlightOrder: ["afreen-id", "sam-id"], spotlightIndex: 0, spotlightPlayerId: "afreen-id", spotlightPlayerName: "Afreen", spotlightChoice: "dare", spotlightPrompt: "Do a victory dance" }} onSend={send} />);
  expect(screen.getByText("Do a victory dance")).toBeInTheDocument();
  expect(screen.getByText("You’re in the spotlight!")).toBeInTheDocument();
});

it("keeps personal Truth or Dare controls separate from transferred host controls", async () => {
  const user = userEvent.setup();
  const send = vi.fn();
  render(<GameView snapshot={{ ...snapshot, gameId: "truth-or-dare", gameTitle: "Truth or Dare", responseMode: "choice", experience: "casual", phase: "playing", round: 1, currentQuestion: "Truth or Dare?", currentOptions: ["Truth", "Dare"], viewerPlayerId: "host-player", submittedAnswerCount: 2, totalEligiblePlayerCount: 2, truthOrDareRevealed: false, truthOrDarePhase: "preparing_reveal" }} onSend={send} />);
  await user.click(screen.getByRole("button", { name: "Start Spotlight Reveal ✦" }));
  expect(send).toHaveBeenCalledWith({ type: "reveal_results" });
  expect(screen.getByText("Host controls")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /Lock choice/ })).toBeInTheDocument();
});
