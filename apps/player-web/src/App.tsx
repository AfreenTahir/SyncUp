import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { PlayerConnection } from "./connection";
import { BrandLogo, VisualOption } from "./BrandLogo";
import type { Credentials, RoomSnapshot, ServerEvent } from "./types";
import "./styles.css";

const API_URL = import.meta.env.VITE_API_URL ?? "";
const WS_URL = import.meta.env.VITE_WS_URL ?? (import.meta.env.DEV
  ? `${window.location.protocol === "https:" ? "wss" : "ws"}://${window.location.host}/api/ws`
  : `${window.location.protocol === "https:" ? "wss" : "ws"}://${window.location.host}/api/ws`);
const STORAGE_KEY = "syncup-player-session";
const CODE_PATTERN = /^[A-HJ-NP-Z2-9]{6}$/;

export function validateJoin(roomCode: string, nickname: string): string | null {
  if (!CODE_PATTERN.test(roomCode.trim().toUpperCase())) return "Enter a valid six-character room code.";
  const cleanName = nickname.trim();
  if (cleanName.length < 1 || [...cleanName].length > 20 || [...cleanName].some((character) => /[\u0000-\u001f\u007f]/.test(character))) return "Display name must be between 1 and 20 characters.";
  return null;
}

function loadCredentials(): Credentials | null {
  try { return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null") as Credentials | null; }
  catch { return null; }
}

export default function App() {
  const query = new URLSearchParams(window.location.search); const queryCode = query.get("room") ?? "";
  const [roomCode, setRoomCode] = useState(queryCode.toUpperCase());
  const [nickname, setNickname] = useState(query.get("name") ?? "");
  const [credentials, setCredentials] = useState<Credentials | null>(() => loadCredentials());
  const [snapshot, setSnapshot] = useState<RoomSnapshot | null>(null);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState("");
  const [joining, setJoining] = useState(false);
  const [confirmLeave, setConfirmLeave] = useState(false);
  const [splash,setSplash]=useState(()=>sessionStorage.getItem("syncup-player-splash-seen")!=="yes");
  const [notice,setNotice]=useState(""); const noticeId=useRef(0);
  const connection = useRef<PlayerConnection | null>(null);

  useEffect(()=>{if(!splash)return;sessionStorage.setItem("syncup-player-splash-seen","yes");const timer=window.setTimeout(()=>setSplash(false),3000);return()=>clearTimeout(timer)},[splash]);
  useEffect(()=>{if(!confirmLeave)return;const close=(event:KeyboardEvent)=>{if(event.key==="Escape")setConfirmLeave(false)};window.addEventListener("keydown",close);return()=>window.removeEventListener("keydown",close)},[confirmLeave]);

  useEffect(() => {
    if (!credentials) return;
    const client = new PlayerConnection(WS_URL, credentials, (event: ServerEvent) => {
      if (event.type === "room_snapshot") { if(!event.snapshot.players.some(player=>player.id===credentials.playerId)){setError("You are no longer in this room.");window.setTimeout(clearSession,500);return;} setSnapshot(event.snapshot); if(event.snapshot.notice&&event.snapshot.notice.id>noticeId.current){noticeId.current=event.snapshot.notice.id;setNotice(event.snapshot.notice.message);window.setTimeout(()=>setNotice(""),3600)} if(event.snapshot.phase==="closed")window.setTimeout(clearSession,1200); }
      if (event.type === "error") {
        setError(event.message);
        if (["INVALID_TOKEN", "INVALID_PLAYER", "ROOM_NOT_FOUND"].includes(event.code)) clearSession();
      }
    }, setConnected);
    connection.current = client;
    client.connect();
    return () => client.stop();
  }, [credentials]);

  useEffect(() => { document.documentElement.dataset.theme = snapshot?.theme ?? "neon-night"; }, [snapshot?.theme]);

  async function join(event: FormEvent) {
    event.preventDefault();
    const validation = validateJoin(roomCode, nickname);
    if (validation) return setError(validation);
    setJoining(true); setError("");
    try {
      const code = roomCode.trim().toUpperCase();
      const response = await fetch(`${API_URL}/api/rooms/${code}/join`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ nickname: nickname.trim() }) });
      const body = await response.json();
      if (!response.ok) throw new Error(body.message ?? "Could not join the room.");
      const next = { roomCode: body.roomCode, playerId: body.playerId, sessionToken: body.sessionToken };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
      setCredentials(next);
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Could not join the room."); }
    finally { setJoining(false); }
  }

  function clearSession() {
    connection.current?.stop();
    localStorage.removeItem(STORAGE_KEY);
    setCredentials(null); setSnapshot(null); setConnected(false); setError(""); setConfirmLeave(false);
  }

  function leaveRoom(){connection.current?.send({type:"leave_room"});window.setTimeout(clearSession,120);}

  if(splash)return <SplashScreen/>;
  if (!credentials) return <JoinScreen roomCode={roomCode} nickname={nickname} error={error} joining={joining} setRoomCode={setRoomCode} setNickname={setNickname} join={join} />;

  return <main className="player-app">
    <header className="player-header"><BrandLogo compact/><div className={`status ${connected ? "online" : ""}`}><i/>{connected ? "Live" : "Reconnecting…"}</div><button className="leave-button" onClick={() => setConfirmLeave(true)}>Leave</button></header>
    {(error||notice) && <div className="error-banner toast-notice" role="status"><span>{error?"!":"✦"}</span>{error||notice}</div>}
    {snapshot ? <GameView snapshot={snapshot} onSend={(event) => connection.current?.send(event)} /> : <Loading roomCode={credentials.roomCode} />}
    {confirmLeave && <div className="modal-backdrop" onMouseDown={event=>{if(event.target===event.currentTarget)setConfirmLeave(false)}}><div className="modal" role="dialog" aria-modal="true" aria-labelledby="leave-title"><button className="modal-close" onClick={()=>setConfirmLeave(false)} aria-label="Close">×</button><span className="modal-icon">↗</span><h2 id="leave-title">Leave the game?</h2><p>{snapshot?.viewerPlayerId===snapshot?.hostPlayerId&&(snapshot?.players.length??0)>1?"Host control will pass to the longest-connected player. Are you sure you want to leave this room?":"Are you sure you want to leave this room?"}</p><div><button className="secondary" onClick={() => setConfirmLeave(false)}>No, stay</button><button className="primary danger-action" onClick={leaveRoom}>Yes, leave</button></div></div></div>}
  </main>;
}

function SplashScreen(){return <main className="splash" aria-label="Getting SyncUp ready"><div className="splash-decor" aria-hidden="true"><i>?</i><i>★</i><i>⚡</i><i>☺</i></div><BrandLogo/><h1>Getting the party ready…</h1><div className="splash-track"><i/></div><p>Questions shuffled. Good vibes loaded.</p></main>}

function JoinScreen({ roomCode, nickname, error, joining, setRoomCode, setNickname, join }: { roomCode: string; nickname: string; error: string; joining: boolean; setRoomCode: (value: string) => void; setNickname: (value: string) => void; join: (event: FormEvent) => void }) {
  return <main className="join-shell"><div className="join-ambient one"/><div className="join-ambient two"/><section className="join-card screen-enter">
    <div className="join-brand"><BrandLogo/></div>
    <div className="join-copy"><p className="eyebrow">Your party is waiting</p><h1>Join the room.</h1><p>Enter the code on the host screen and the name your friends know.</p></div>
    <form onSubmit={join} noValidate><label htmlFor="room-code">Room code</label><div className="input-wrap code-input"><span>#</span><input id="room-code" inputMode="text" autoCapitalize="characters" autoComplete="off" maxLength={6} value={roomCode} onChange={(event) => setRoomCode(event.target.value.toUpperCase().replace(/[^A-Z0-9]/g, ""))} placeholder="ABC234" /></div>
      <label htmlFor="nickname">Display name</label><div className="input-wrap"><span>☺</span><input id="nickname" autoComplete="nickname" maxLength={20} value={nickname} onChange={(event) => setNickname(event.target.value)} placeholder="Afreen" /></div>
      {error && <div className="form-error" role="alert"><span>!</span>{error}</div>}<button className="primary join-button" disabled={joining}>{joining ? <><i className="tiny-spinner"/>Joining…</> : <>Join room <span>→</span></>}</button>
    </form><p className="privacy">No account needed. Your session stays on this device.</p>
  </section></main>;
}

export function GameView({ snapshot, onSend }: { snapshot: RoomSnapshot; onSend: (event: object) => unknown }) {
  const [selected, setSelected] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  useEffect(() => { setSelected(null); setConfirming(false); }, [snapshot.round]);

  const choices = useMemo(() => snapshot.responseMode === "player_vote"
    ? snapshot.players.map((player) => ({ id: player.id, label: player.nickname, helper: player.connected ? "Ready" : "Offline", visual: undefined }))
    : snapshot.currentOptions.map((option, index) => ({ id: index.toString(), label: option, helper: String.fromCharCode(65 + index), visual: snapshot.currentVisualOptions[index] })), [snapshot]);

  const isHost=snapshot.viewerPlayerId===snapshot.hostPlayerId;
  if(snapshot.phase==="closed")return <section className="player-card loading-card"><h1>Room ended</h1><p className="muted">Thanks for playing. Taking you home…</p></section>;
  if(snapshot.phase==="choosing")return <WaitingForGame snapshot={snapshot} onSend={onSend} isHost={isHost}/>;
  if (snapshot.phase === "lobby") return <Lobby snapshot={snapshot} onSend={onSend} isHost={isHost} />;
  if (snapshot.gameId === "truth-or-dare" && snapshot.phase === "playing") return <TruthOrDareTurn snapshot={snapshot} onSend={onSend} isHost={isHost} />;
  if (snapshot.phase === "playing" && snapshot.viewerHasAnswered) return <AnswerLocked snapshot={snapshot} />;
  if (snapshot.phase === "playing") return <RoundView snapshot={snapshot} choices={choices} selected={selected} setSelected={setSelected} confirming={confirming} setConfirming={setConfirming} submit={() => { if (selected !== null) onSend({ type: "submit_answer", round: snapshot.round, answer: selected }); setConfirming(false); }} />;
  if (snapshot.phase === "results") return <><Results snapshot={snapshot} />{isHost&&<button className="primary floating-host-action" onClick={()=>onSend({type:"next_round"})}>{snapshot.round>=snapshot.maxRounds?"See final results":"Next round →"}</button>}</>;
  return <><FinalLeaderboard snapshot={snapshot} />{isHost&&<div className="mobile-host-actions"><button className="primary" onClick={()=>onSend({type:"choose_another_game"})}>Play another game</button><button className="secondary" onClick={()=>onSend({type:"configure_game",gameId:snapshot.gameId,rounds:snapshot.maxRounds,theme:snapshot.theme})}>Play again</button><button className="ghost danger" onClick={()=>onSend({type:"end_room"})}>End room</button></div>}</>;
}

function Lobby({ snapshot,onSend,isHost }: { snapshot: RoomSnapshot;onSend:(event:object)=>unknown;isHost:boolean }) {
  return <section className="player-card lobby-card screen-enter"><div className="game-label"><span>✦</span>{snapshot.gameTitle}</div><p className="eyebrow">Room code</p><div className="room-code">{snapshot.roomCode}</div><button className="copy-room secondary" onClick={()=>void navigator.clipboard?.writeText(snapshot.roomCode)}>Copy code</button><h1>{isHost?"You’re the host":"Everyone’s gathering"}</h1><p className="muted">{isHost?"You now control this party from your phone.":"The host will start when everyone is ready."}</p><div className="lobby-progress"><span>{snapshot.players.length}/12 joined</span><div><i style={{ width: `${snapshot.players.length / 12 * 100}%` }}/></div></div><PlayerList snapshot={snapshot} />{isHost&&<button className="primary submit-button" disabled={snapshot.players.length<2} onClick={()=>onSend({type:"start_game"})}>{snapshot.players.length<2?"Waiting for a friend":"Start game →"}</button>}</section>;
}

type MiniGame={id:string;title:string;icon:string;experience:string;defaultRounds:number};
function WaitingForGame({snapshot,onSend,isHost}:{snapshot:RoomSnapshot;onSend:(event:object)=>unknown;isHost:boolean}){const[games,setGames]=useState<MiniGame[]>([]);const[selected,setSelected]=useState<MiniGame|null>(null);useEffect(()=>{if(isHost)fetch(`${API_URL}/api/games`).then(r=>r.json()).then(setGames).catch(()=>setGames([]))},[isHost]);if(!isHost)return <section className="player-card waiting-game screen-enter"><div className="waiting-orbit">✦</div><p className="eyebrow">Room {snapshot.roomCode}</p><h1>The host is picking the next game</h1><p className="muted">Stay here—your party and room code are still connected.</p><button className="secondary copy-room" onClick={()=>void navigator.clipboard?.writeText(snapshot.roomCode)}>Copy room code</button><div className="waiting-pulse"><i/><span>Choosing something fun…</span></div></section>;return <section className="player-card waiting-game screen-enter"><p className="eyebrow">You’re the host · room {snapshot.roomCode}</p><h1>{selected?`Ready for ${selected.title}?`:"Pick the next game"}</h1>{!selected?<div className="mobile-game-grid">{games.map((game,index)=><button key={game.id} onClick={()=>setSelected(game)}><span className={`game-bubble tone-${index%4}`}>{game.title.slice(0,1)}</span><strong>{game.title}</strong><small>{game.experience}</small></button>)}</div>:<div className="mobile-setup"><button className="secondary" onClick={()=>setSelected(null)}>← Back</button><button className="primary" onClick={()=>onSend({type:"configure_game",gameId:selected.id,rounds:selected.defaultRounds,theme:snapshot.theme})}>Ready this game →</button></div>}</section>}

function RoundView({ snapshot, choices, selected, setSelected, confirming, setConfirming, submit }: { snapshot: RoomSnapshot; choices: { id: string; label: string; helper: string; visual?: string }[]; selected: string | null; setSelected: (value: string) => void; confirming: boolean; setConfirming: (value: boolean) => void; submit: () => void }) {
  return <section className="player-card round-card screen-enter"><div className="round-top"><span>Round {snapshot.round} of {snapshot.maxRounds}</span><em>{snapshot.currentCategory}</em></div><p className="eyebrow">{snapshot.gameTitle}</p><h1 className="question">{snapshot.currentQuestion}</h1><div className={`choice-grid ${choices.length <= 2 ? "two" : ""} ${snapshot.gameId === "this-or-that" ? "visual" : ""}`} role="radiogroup" aria-label="Choose your answer">{choices.map((choice, index) => <button type="button" role="radio" aria-checked={selected === choice.id} className={`choice ${selected === choice.id ? "selected" : ""}`} key={choice.id} onClick={() => setSelected(choice.id)}><VisualOption id={choice.visual || (snapshot.gameId === "this-or-that" ? "generated" : undefined)} label={choice.label}/><span className="choice-index">{snapshot.responseMode === "player_vote" ? choice.label.slice(0,1).toUpperCase() : String.fromCharCode(65 + index)}</span><strong>{choice.label}</strong><small>{snapshot.responseMode === "player_vote" ? choice.helper : ""}</small><i>✓</i></button>)}</div><button className="primary submit-button" disabled={selected === null} onClick={() => setConfirming(true)}>{snapshot.responseMode === "player_vote" ? "Confirm vote" : "Lock answer"}<span>→</span></button>
    {confirming && <div className="confirm-sheet" role="dialog"><span className="drag-handle"/><p className="eyebrow">Final answer?</p><h2>Your choice locks for this round.</h2><div><button className="secondary" onClick={() => setConfirming(false)}>Go back</button><button className="primary" onClick={submit}>Lock it in</button></div></div>}
  </section>;
}

function AnswerLocked({ snapshot }: { snapshot: RoomSnapshot }) {
  const progress = snapshot.totalEligiblePlayerCount ? snapshot.submittedAnswerCount / snapshot.totalEligiblePlayerCount * 100 : 0;
  return <section className="player-card locked-card screen-enter"><div className="success-ring"><span>✓</span></div><p className="eyebrow">Round {snapshot.round} of {snapshot.maxRounds}</p><h1>Locked in.</h1><p className="muted">Your answer is safe. Waiting for the rest of the room.</p><div className="waiting-count"><div><strong>{snapshot.submittedAnswerCount}</strong><span>of {snapshot.totalEligiblePlayerCount} answered</span></div><div className="progress-track"><i style={{ width: `${progress}%` }}/></div></div><div className="waiting-pulse"><i/><span>Results appear when the host reveals them</span></div></section>;
}

function Results({ snapshot }: { snapshot: RoomSnapshot }) {
  const max = Math.max(0, ...(snapshot.results ?? []).map((result) => result.votes));
  const total = (snapshot.results ?? []).reduce((sum, result) => sum + result.votes, 0);
  if (snapshot.experience !== "competitive") {
    if (snapshot.gameId === "most-likely-to") { const winner = snapshot.results?.[0]; const titles = ["Main Character", "Chaos Magnet", "Drama Royalty", "Snack Legend", "Always Online", "Lucky One"]; return <section className="player-card results-card spotlight-result screen-enter"><Confetti/><p className="eyebrow">The room has spoken</p><div className="spotlight-avatar">{winner?.label.slice(0,1).toUpperCase()}</div><h1>{winner?.label}</h1><div className="fun-title">♛ {titles[snapshot.round % titles.length]}</div><p className="muted">{winner?.votes ?? 0} votes this round</p><ResultCountdown version={snapshot.version}/></section>; }
    if (snapshot.gameId === "never-have-i-ever") { const yes = snapshot.results?.find(result => result.label === "I have")?.votes ?? 0; return <section className="player-card results-card social-result screen-enter"><p className="eyebrow">Honesty unlocked</p><h1><em>{yes}</em> out of <em>{total}</em> players have done this 😭</h1><SplitResults snapshot={snapshot}/><ResultCountdown version={snapshot.version}/></section>; }
    return <section className="player-card results-card voting-result screen-enter"><p className="eyebrow">The room has decided</p><h1>{snapshot.currentQuestion}</h1><SplitResults snapshot={snapshot}/><p className="popular-choice">Most popular: <strong>{snapshot.results?.[0]?.label}</strong> · {total} votes</p><ResultCountdown version={snapshot.version}/></section>;
  }
  return <section className="player-card results-card screen-enter"><Confetti/><div className="round-top"><span>Round {snapshot.round} results</span><em>{snapshot.currentCategory}</em></div><h1>{snapshot.currentQuestion}</h1><div className="results-list">{snapshot.results?.map((result, index) => <div className={`result-row ${result.isCorrect ? "correct" : ""}`} style={{ "--delay": `${index * 80}ms` } as React.CSSProperties} key={result.id}><span>{result.isCorrect ? "✓" : result.playerId ? result.label.slice(0,1).toUpperCase() : index + 1}</span><div><strong>{result.label}</strong><i><b style={{ width: `${max ? result.votes / max * 100 : 0}%` }}/></i></div><em><AnimatedNumber value={result.votes}/></em></div>)}</div><MiniLeaderboard snapshot={snapshot}/><ResultCountdown version={snapshot.version}/></section>;
}

function SplitResults({ snapshot }: { snapshot: RoomSnapshot }) { const total = (snapshot.results ?? []).reduce((sum,result) => sum + result.votes,0); return <div className="split-results">{snapshot.results?.map(result => <div key={result.id}><strong>{result.label}</strong><b>{total ? Math.round(result.votes / total * 100) : 0}%</b><span>{result.votes} responses</span><i style={{ transform:`scaleX(${total ? result.votes / total : 0})` }}/></div>)}</div>; }

function TruthOrDareTurn({ snapshot, onSend, isHost }: { snapshot: RoomSnapshot; onSend: (event: object) => unknown; isHost: boolean }) {
  const [choice,setChoice]=useState<"truth"|"dare"|null>(null); useEffect(()=>setChoice(null),[snapshot.round]);
  const phase=snapshot.truthOrDarePhase??"choosing"; const isSpotlight=snapshot.viewerPlayerId===snapshot.spotlightPlayerId;
  const sendReaction=(emoji:string)=>onSend({type:"send_reaction",round:snapshot.round,emoji,reactionId:globalThis.crypto?.randomUUID?.()??`${Date.now()}-${Math.random()}`});
  useEffect(()=>{if(phase!=="spotlight"||!isHost||(!snapshot.spotlightCompleted&&!snapshot.spotlightSkipped))return;const timer=window.setTimeout(()=>onSend({type:"next_spotlight",round:snapshot.round}),3000);return()=>clearTimeout(timer)},[phase,isHost,snapshot.spotlightCompleted,snapshot.spotlightSkipped,snapshot.spotlightIndex,snapshot.round]);
  if(phase==="choosing"||phase==="preparing_reveal")return <section className="player-card truth-card screen-enter"><div className="round-top"><span>Round {snapshot.round} of {snapshot.maxRounds}</span><em>Everyone chooses</em></div><p className="eyebrow">Your private choice</p><h1>Truth or Dare?</h1>
    {!snapshot.viewerHasAnswered&&<><p className="muted">Pick your own path. Nobody sees it until your Spotlight turn.</p><div className="truth-choice-grid" role="radiogroup" aria-label="Choose Truth or Dare"><button type="button" role="radio" aria-checked={choice==="truth"} className={`truth-option ${choice==="truth"?"selected":""}`} onClick={()=>setChoice("truth")}><span>◌</span><strong>Truth</strong><small>Answer something honestly</small><i>✓</i></button><button type="button" role="radio" aria-checked={choice==="dare"} className={`dare-option ${choice==="dare"?"selected":""}`} onClick={()=>setChoice("dare")}><span>ϟ</span><strong>Dare</strong><small>Take on a playful challenge</small><i>✓</i></button></div><button className="primary submit-button" disabled={!choice} onClick={()=>choice&&onSend({type:"submit_answer",round:snapshot.round,answer:choice})}>Lock choice →</button></>}
    {snapshot.viewerHasAnswered&&<div className="choice-locked"><span>✓</span><h2>{snapshot.viewerTruthOrDareChoice==="truth"?"Truth":"Dare"} locked!</h2><p>{phase==="preparing_reveal"?"Everyone is ready for the Spotlight.":"Waiting for the others…"}</p></div>}
    <div className="waiting-count"><div><strong>{snapshot.submittedAnswerCount}</strong><span>of {snapshot.totalEligiblePlayerCount} ready</span></div><div className="progress-track"><i style={{transform:`scaleX(${snapshot.totalEligiblePlayerCount?snapshot.submittedAnswerCount/snapshot.totalEligiblePlayerCount:0})`}}/></div></div>{isHost&&<div className="mobile-host-panel"><small>Host controls</small><button className="secondary" disabled={phase!=="preparing_reveal"} onClick={()=>onSend({type:"reveal_results"})}>{phase==="preparing_reveal"?"Start Spotlight Reveal ✦":"Waiting for everyone"}</button></div>}
  </section>;
  if(phase==="spotlight")return <section className="player-card truth-card spotlight-shared screen-enter"><div className="round-top"><span>Round {snapshot.round}</span><em>Player {(snapshot.spotlightIndex??0)+1} of {snapshot.spotlightOrder?.length??0}</em></div><p className="eyebrow">{isSpotlight?"You’re in the spotlight!":`${snapshot.spotlightPlayerName} is in the spotlight!`}</p><div className="spotlight-avatar">{snapshot.spotlightPlayerName?.slice(0,1).toUpperCase()}</div><h1>{snapshot.spotlightPlayerName} chose <em>{snapshot.spotlightChoice}</em></h1><div className={`challenge-card ${snapshot.spotlightChoice}`}><small>{snapshot.spotlightChoice}</small><strong>{snapshot.spotlightPrompt}</strong></div><div className="reaction-tray" aria-label="React to this moment">{["😂","😱","👏","🔥","❤️"].map(emoji=><button key={emoji} onClick={()=>sendReaction(emoji)}>{emoji}</button>)}</div><div className="reaction-burst" aria-live="polite">{snapshot.reactions?.slice(-5).map(reaction=><span key={reaction.id}>{reaction.emoji}</span>)}</div>
    {isSpotlight&&!snapshot.spotlightCompleted&&!snapshot.spotlightSkipped&&<div className="spotlight-actions"><button className="primary" onClick={()=>onSend({type:"mark_completed",round:snapshot.round})}>Completed ✓</button><button className="secondary" disabled={!snapshot.viewerRerollAvailable} onClick={()=>onSend({type:"reroll_spotlight",round:snapshot.round})}>{snapshot.viewerRerollAvailable?"Use free reroll":"Reroll used"}</button><button className="ghost" onClick={()=>onSend({type:"skip_spotlight",round:snapshot.round})}>Skip safely</button></div>}{(snapshot.spotlightCompleted||snapshot.spotlightSkipped)&&<p className="spotlight-finished">{snapshot.spotlightSkipped?"Prompt skipped — no pressure.":"Moment complete! 🎉"}</p>}{isHost&&<div className="mobile-host-panel"><small>Host controls</small>{!snapshot.spotlightCompleted&&!snapshot.spotlightSkipped&&<button className="ghost" onClick={()=>window.confirm("Skip this inactive player’s turn?")&&onSend({type:"force_skip_spotlight",round:snapshot.round})}>Skip inactive player</button>}<button className="secondary" disabled={!snapshot.spotlightCompleted&&!snapshot.spotlightSkipped} onClick={()=>onSend({type:"next_spotlight",round:snapshot.round})}>Next player →</button></div>}
  </section>;
  if(phase==="highlight_voting")return <section className="player-card truth-card screen-enter"><p className="eyebrow">Celebrate the round</p><h1>Pick the highlights</h1><p className="muted">Votes stay private. You can’t vote for yourself.</p><div className="highlight-ballot">{snapshot.highlightCategories?.map(category=><div key={category.id}><h2>{category.emoji} {category.label}</h2><div>{category.eligiblePlayerIds.filter(id=>id!==snapshot.viewerPlayerId).map(id=>{const player=snapshot.players.find(item=>item.id===id);return <button className={snapshot.viewerHighlightVotes?.[category.id]===id?"selected":""} disabled={Boolean(snapshot.viewerHighlightVotes?.[category.id])} key={id} onClick={()=>onSend({type:"submit_highlight_vote",round:snapshot.round,categoryId:category.id,selectedPlayerId:id})}>{player?.nickname}</button>})}</div></div>)}</div><button className="ghost" disabled={snapshot.viewerSkippedHighlightVoting} onClick={()=>onSend({type:"skip_highlight_voting",round:snapshot.round})}>{snapshot.viewerSkippedHighlightVoting?"Voting skipped":"Skip voting"}</button><p className="muted">{snapshot.highlightReadyCount??0} of {snapshot.totalEligiblePlayerCount} finished</p>{isHost&&<div className="mobile-host-panel"><button className="primary" disabled={(snapshot.highlightReadyCount??0)<snapshot.totalEligiblePlayerCount} onClick={()=>onSend({type:"reveal_highlights",round:snapshot.round})}>Reveal highlights ✦</button></div>}</section>;
  return <section className="player-card truth-card highlight-reveal screen-enter"><Confetti/><p className="eyebrow">Round {snapshot.round} highlights</p><h1>Favourite moments</h1><div className="highlight-winners">{snapshot.highlightResults?.map(result=><div key={result.categoryId}><span>{result.emoji}</span><strong>{result.label}</strong><p>{result.winnerPlayerIds.map(id=>snapshot.players.find(player=>player.id===id)?.nickname).join(" & ")}</p></div>)}</div>{isHost&&<button className="primary submit-button" onClick={()=>onSend({type:"next_round"})}>{snapshot.round>=snapshot.maxRounds?"Finish game":"Next round →"}</button>}{!isHost&&<p className="waiting-note"><i/>Waiting for the host</p>}</section>;
}

function MiniLeaderboard({ snapshot }: { snapshot: RoomSnapshot }) { return <div className="mini-board"><p className="eyebrow">Overall standings</p>{snapshot.leaderboard.slice(0,3).map((entry) => <div key={entry.playerId}><span>#{entry.rank}</span><strong>{entry.nickname}</strong><b>{entry.score} pts</b></div>)}</div>; }

function FinalLeaderboard({ snapshot }: { snapshot: RoomSnapshot }) {
  if(snapshot.gameId==="truth-or-dare")return <section className="player-card final-card casual-final screen-enter"><Confetti/><div className="thanks-icon">🎉</div><p className="eyebrow">Truth or Dare complete</p><h1>Your party highlights</h1><div className="highlight-winners">{snapshot.historicalHighlights?.map((result,index)=><div key={`${result.categoryId}-${index}`}><span>{result.emoji}</span><strong>{result.label}</strong><p>{result.winnerPlayerIds.map(id=>snapshot.players.find(player=>player.id===id)?.nickname).join(" & ")}</p></div>)}</div><div className="reaction-totals">{snapshot.players.map(player=><span key={player.id}><strong>{player.nickname}</strong> {snapshot.reactionTotals?.[player.id]??0} reactions</span>)}</div><p className="waiting-note"><i/>The room stays together for another game</p></section>;
  if (snapshot.experience === "casual") return <section className="player-card final-card casual-final screen-enter"><Confetti/><div className="thanks-icon">🎉</div><p className="eyebrow">{snapshot.gameTitle}</p><h1>Thanks for playing!</h1><p className="muted">Hope you had fun. The host can start another game whenever the room is ready.</p><p className="waiting-note"><i/>Waiting for the host</p></section>;
  if (snapshot.experience === "voting") return <section className="player-card final-card screen-enter"><p className="eyebrow">{snapshot.gameTitle} complete</p><h1>Round highlights</h1><p className="muted">No winners or losers—just the room’s favourite choices.</p><div className="highlight-list">{snapshot.roundHistory?.slice(0,3).map((round,index) => <div key={round.round}><span>{index+1}</span><p>{round.question}<strong>{round.results[0]?.label} · {round.results[0]?.votes ?? 0} votes</strong></p></div>)}</div><p className="waiting-note"><i/>Waiting for the host</p></section>;
  return <section className="player-card final-card screen-enter"><Confetti/><div className="trophy">♛</div><p className="eyebrow">{snapshot.gameTitle} complete</p><h1>Final leaderboard</h1><p className="muted">Great game. Here’s how the room finished.</p><div className="leaderboard">{snapshot.leaderboard.map((entry, index) => <div className={index === 0 ? "winner" : ""} style={{ "--delay": `${index * 90}ms` } as React.CSSProperties} key={entry.playerId}><span>{index === 0 ? "♛" : `#${entry.rank}`}</span><strong>{entry.nickname}</strong><b><AnimatedNumber value={entry.score}/></b><small>pts</small></div>)}</div><p className="waiting-note"><i/>The host can start another room</p></section>;
}

function PlayerList({ snapshot }: { snapshot: RoomSnapshot }) { return <div className="player-list">{snapshot.players.map((player, index) => <div className="lobby-player" style={{ "--delay": `${index * 60}ms` } as React.CSSProperties} key={player.id}><span>{player.nickname.slice(0,1).toUpperCase()}</span><strong>{player.nickname}</strong><small><i className={player.connected ? "online" : ""}/>{player.connected ? "Connected" : "Reconnecting"}</small></div>)}</div>; }
function Loading({ roomCode }: { roomCode: string }) { return <section className="player-card loading-card"><div className="spinner"/><p className="eyebrow">Room {roomCode}</p><h1>Getting you in sync…</h1><p className="muted">Restoring the latest game state.</p></section>; }
function AnimatedNumber({ value }: { value: number }) { const [shown, setShown] = useState(0); useEffect(() => { let frame = 0; const started = performance.now(); const tick = (now: number) => { setShown(Math.round(value * Math.min(1, (now - started) / 500))); if (now - started < 500) frame = requestAnimationFrame(tick); }; frame = requestAnimationFrame(tick); return () => cancelAnimationFrame(frame); }, [value]); return <>{shown}</>; }
function ResultCountdown({ version }: { version: number }) { const [seconds, setSeconds] = useState(3); useEffect(() => { setSeconds(3); const timer = window.setInterval(() => setSeconds(value => Math.max(0, value - 1)), 1000); return () => clearInterval(timer); }, [version]); return <p className="waiting-note"><i/>{seconds ? `Next round in ${seconds}s` : "Moving to the next round"}</p>; }
function Confetti() { return <div className="confetti" aria-hidden="true">{Array.from({ length: 12 }, (_, index) => <i key={index} style={{ "--i": index } as React.CSSProperties}/>)}</div>; }
