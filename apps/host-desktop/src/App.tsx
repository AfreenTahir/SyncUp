import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { QRCodeSVG } from "qrcode.react";
import { HostConnection } from "./connection";
import { BrandLogo, VisualOption } from "./BrandLogo";
import type { GameSummary, HostCredentials, RoomSnapshot, ServerEvent } from "./types";
import "./styles.css";

const IS_TAURI = "__TAURI_INTERNALS__" in window;
const API_URL = import.meta.env.VITE_API_URL ?? (import.meta.env.DEV || IS_TAURI ? "http://127.0.0.1:3000" : window.location.origin);
const WS_URL = import.meta.env.VITE_WS_URL ?? `${API_URL.replace(/^http/, "ws").replace(/\/$/, "")}/api/ws`;
const DEFAULT_PLAYER_URL = import.meta.env.VITE_PLAYER_WEB_URL ?? (import.meta.env.DEV ? "http://127.0.0.1:5173" : IS_TAURI ? "http://127.0.0.1:3000" : window.location.origin);
const STORAGE_KEY = "syncup-host-session";
const SETTINGS_KEY = "syncup-host-settings";
const THEMES = [["neon-night", "Neon Night"], ["ocean-blue", "Ocean Blue"], ["sunset-glow", "Sunset Glow"], ["emerald", "Emerald"], ["purple-galaxy", "Purple Galaxy"], ["sakura", "Sakura"]] as const;

type HomeScreen = "home" | "games" | "settings";
type Confirmation = { title: string; message: string; action: () => void } | null;

function storedCredentials(): HostCredentials | null {
  try { return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null"); } catch { return null; }
}

function storedSettings() {
  try { return JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? '{"sound":false,"motion":true}') as { sound: boolean; motion: boolean }; }
  catch { return { sound: false, motion: true }; }
}

const iconFor = (icon: string) => ({
  spark: "✦", split: "⇄", crown: "♛", hand: "◌", bolt: "ϟ",
  switch: "↔", smile: "☺", mask: "◑", timer: "◷", words: "Aa",
}[icon] ?? "✦");

function playCue(cue: "join" | "reveal" | "winner") {
  try {
    const context = new AudioContext();
    const notes = cue === "join" ? [440, 660] : cue === "reveal" ? [330, 440, 550] : [392, 523, 659, 784];
    notes.forEach((frequency, index) => {
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      const start = context.currentTime + index * .08;
      oscillator.type = "sine";
      oscillator.frequency.value = frequency;
      gain.gain.setValueAtTime(.0001, start);
      gain.gain.exponentialRampToValueAtTime(.075, start + .015);
      gain.gain.exponentialRampToValueAtTime(.0001, start + .13);
      oscillator.connect(gain).connect(context.destination);
      oscillator.start(start);
      oscillator.stop(start + .14);
    });
    window.setTimeout(() => void context.close(), 750);
  } catch { /* Audio is an optional enhancement. */ }
}

export default function App() {
  const [credentials, setCredentials] = useState<HostCredentials | null>(() => storedCredentials());
  const [snapshot, setSnapshot] = useState<RoomSnapshot | null>(null);
  const [connected, setConnected] = useState(false);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState("");
  const [screen, setScreen] = useState<HomeScreen>("home");
  const [games, setGames] = useState<GameSummary[]>([]);
  const [rounds, setRounds] = useState(5);
  const [theme, setTheme] = useState("neon-night");
  const [hostNickname, setHostNickname] = useState(() => localStorage.getItem("syncup-host-name") ?? "Afreen");
  const [settings, setSettings] = useState(storedSettings);
  const [confirmation, setConfirmation] = useState<Confirmation>(null);
  const [joinOpen, setJoinOpen] = useState(false);
  const [splash, setSplash] = useState(() => sessionStorage.getItem("syncup-splash-seen") !== "yes");
  const [notice, setNotice] = useState("");
  const [playerUrl, setPlayerUrl] = useState(DEFAULT_PLAYER_URL);
  const connection = useRef<HostConnection | null>(null);
  const previousSnapshot = useRef<RoomSnapshot | null>(null);
  const noticeId = useRef(0);

  useEffect(() => {
    if (!splash) return;
    sessionStorage.setItem("syncup-splash-seen", "yes");
    const timer = window.setTimeout(() => setSplash(false), 3000);
    return () => window.clearTimeout(timer);
  }, [splash]);

  useEffect(() => {
    if (!confirmation && !joinOpen) return;
    const close = (event: KeyboardEvent) => { if (event.key === "Escape") { setConfirmation(null); setJoinOpen(false); } };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, [confirmation, joinOpen]);

  useEffect(() => {
    let cancelled = false;
    const loadCatalog = async () => {
      for (let attempt = 0; attempt < 20 && !cancelled; attempt += 1) {
        try {
          const response = await fetch(`${API_URL}/api/games`);
          if (!response.ok) throw new Error("catalog unavailable");
          const catalog: GameSummary[] = await response.json();
          if (!cancelled) { setGames(catalog); setError(""); }
          return;
        } catch {
          await new Promise((resolve) => window.setTimeout(resolve, 250));
        }
      }
      if (!cancelled) setError("The local game server could not start. Close SyncUp and open it again.");
    };
    void loadCatalog();
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (!IS_TAURI || import.meta.env.VITE_PLAYER_WEB_URL) return;
    invoke<string>("local_player_url").then(setPlayerUrl).catch(() => undefined);
  }, []);

  useEffect(() => {
    const previous = previousSnapshot.current;
    if (settings.sound && snapshot && previous) {
      if (snapshot.players.length > previous.players.length) playCue("join");
      if (snapshot.phase === "results" && previous.phase !== "results") playCue("reveal");
      if (snapshot.phase === "finished" && previous.phase !== "finished") playCue("winner");
    }
    previousSnapshot.current = snapshot;
  }, [snapshot, settings.sound]);

  useEffect(() => {
    document.documentElement.dataset.motion = settings.motion ? "on" : "off";
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
  }, [settings]);

  useEffect(() => { document.documentElement.dataset.theme = snapshot?.theme ?? theme; }, [snapshot?.theme, theme]);

  useEffect(() => {
    if (!credentials) return;
    const client = new HostConnection(WS_URL, credentials, (event: ServerEvent) => {
      if (event.type === "room_snapshot") {
        setSnapshot(event.snapshot);
        if (event.snapshot.notice && event.snapshot.notice.id > noticeId.current) {
          noticeId.current = event.snapshot.notice.id;
          setNotice(event.snapshot.notice.message);
          window.setTimeout(() => setNotice(""), 3600);
        }
        if (event.snapshot.phase === "closed") window.setTimeout(resetSession, 1200);
      }
      if (event.type === "error") {
        setError(event.message);
        if (["INVALID_TOKEN", "ROOM_NOT_FOUND"].includes(event.code)) resetSession();
      }
    }, setConnected);
    connection.current = client;
    client.connect();
    return () => client.stop();
  }, [credentials]);

  async function createRoom(game: GameSummary) {
    setCreating(true); setError(""); setSnapshot(null);
    try {
      const selectedRounds = Math.min(rounds, game.questionCount);
      const response = await fetch(`${API_URL}/api/rooms`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ gameId: game.id, rounds: selectedRounds, theme, hostNickname: hostNickname.trim() || "Afreen" }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.message ?? "Could not create a room.");
      const next = { roomCode: body.roomCode, hostToken: body.hostToken };
      localStorage.setItem("syncup-host-name", hostNickname.trim() || "Afreen");
      localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
      setCredentials(next);
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Could not create a room."); }
    finally { setCreating(false); }
  }

  function resetSession() {
    connection.current?.stop();
    localStorage.removeItem(STORAGE_KEY);
    setCredentials(null); setSnapshot(null); setConnected(false); setError(""); setScreen("home");
  }

  const send = (event: object) => {
    setError("");
    if (!connection.current?.send(event)) setError("The host is reconnecting. Please try again shortly.");
  };

  if (splash) return <SplashScreen />;

  if (!credentials) return (
    <main className="app-shell home-shell">
      <nav className="home-nav">
        <BrandLogo />
        <button className="icon-button" onClick={() => setScreen(screen === "settings" ? "home" : "settings")} aria-label="Settings">⚙</button>
      </nav>
      {(error || notice) && <Toast message={error || notice} />}
      {screen === "home" && <Home onCreate={() => setScreen("games")} onJoin={() => setJoinOpen(true)} />}
      {screen === "games" && <GameLibrary games={games} rounds={rounds} setRounds={setRounds} theme={theme} setTheme={setTheme} hostNickname={hostNickname} setHostNickname={setHostNickname} creating={creating} onChoose={createRoom} onBack={() => setScreen("home")} />}
      {screen === "settings" && <Settings settings={settings} setSettings={setSettings} onBack={() => setScreen("home")} />}
      {joinOpen && <JoinDialog playerUrl={playerUrl} close={() => setJoinOpen(false)} />}
      <footer><span>Private rooms</span><span>•</span><span>Up to 12 players</span><span>•</span><span>Real-time play</span></footer>
    </main>
  );

  return (
    <main className="app-shell game-shell">
      <header className="game-header">
        <BrandLogo compact />
        <div className="header-actions">
          <ConnectionBadge connected={connected} />
          {snapshot && <button className="ghost danger header-end" onClick={() => setConfirmation({ title: "End room for everyone?", message: "This closes the party for every connected player. This cannot be undone.", action: () => send({ type: "end_room" }) })}>End room</button>}
          <button className="icon-button" onClick={() => setConfirmation({ title: "Leave the game?", message: "Host control will transfer to the longest-connected player. Are you sure you want to leave this room?", action: () => { send({ type: "leave_room" }); window.setTimeout(resetSession, 150); } })} aria-label="Leave room">↗</button>
        </div>
      </header>
      {(error || notice) && <Toast message={error || notice} />}
      {snapshot
        ? <HostGame snapshot={snapshot} connected={connected} playerUrl={`${playerUrl.replace(/\/$/, "")}/?room=${encodeURIComponent(snapshot.roomCode)}`} send={send} resetSession={resetSession} games={games} rounds={rounds} setRounds={setRounds} theme={theme} setTheme={setTheme} confirm={setConfirmation} />
        : <Loading title={`Restoring room ${credentials.roomCode}`} subtitle="Syncing the latest room state…" />}
      {confirmation && <ConfirmDialog confirmation={confirmation} close={() => setConfirmation(null)} />}
    </main>
  );
}

function Home({ onCreate, onJoin }: { onCreate: () => void; onJoin: () => void }) {
  return <section className="hero view-enter">
    <div className="party-scene" aria-hidden="true"><span>?</span><span>★</span><span>☺</span><span>⚡</span><div className="hero-logo"><BrandLogo compact /></div></div>
    <p className="eyebrow">Games are better together</p>
    <h1>Ready to <em>sync up?</em></h1>
    <p className="hero-copy">Make a room, bring your favourite people, and start playing.</p>
    <div className="hero-actions"><button className="primary xl" onClick={onCreate}>Create a room <span>→</span></button><button className="secondary xl" onClick={onJoin}>Join with code</button></div>
  </section>;
}

function SplashScreen() { return <main className="splash" aria-label="Getting SyncUp ready"><div className="splash-decor" aria-hidden="true"><i>?</i><i>★</i><i>⚡</i><i>☺</i></div><BrandLogo/><h1>Getting the party ready…</h1><div className="splash-track"><i/></div><p>Questions shuffled. Good vibes loaded.</p></main>; }

function JoinDialog({ playerUrl, close }: { playerUrl: string; close: () => void }) {
  const [code,setCode]=useState(""); const [name,setName]=useState(""); const [message,setMessage]=useState("");
  const submit=(event:React.FormEvent)=>{event.preventDefault();const clean=code.trim().toUpperCase();if(!/^[A-HJ-NP-Z2-9]{6}$/.test(clean))return setMessage("Enter the six-character code from the host screen.");if(!name.trim())return setMessage("Add the name your friends know.");window.location.href=`${playerUrl.replace(/\/$/,"")}/?room=${encodeURIComponent(clean)}&name=${encodeURIComponent(name.trim())}`;};
  const paste=async()=>{try{setCode((await navigator.clipboard.readText()).trim().toUpperCase().replace(/[^A-Z0-9]/g,"").slice(0,6));}catch{setMessage("Paste isn’t available here. Type the code instead.");}};
  return <div className="modal-backdrop" onMouseDown={(event)=>{if(event.target===event.currentTarget)close();}}><form className="modal join-modal" role="dialog" aria-modal="true" aria-labelledby="join-title" onSubmit={submit}><button type="button" className="modal-close" onClick={close} aria-label="Close">×</button><span className="modal-icon">#</span><h2 id="join-title">Join with code</h2><p>Use the code on your friend’s screen.</p><label><span>Room code</span><div className="inline-input"><input autoFocus value={code} maxLength={6} onChange={event=>setCode(event.target.value.toUpperCase().replace(/[^A-Z0-9]/g,""))} placeholder="ABC234"/><button type="button" onClick={paste}>Paste</button></div></label><label><span>Display name</span><input value={name} maxLength={20} onChange={event=>setName(event.target.value)} placeholder="Afreen"/></label>{message&&<small className="form-message" role="alert">{message}</small>}<button className="primary" type="submit">Join room →</button></form></div>;
}

function GameLibrary({ games, rounds, setRounds, theme, setTheme, hostNickname, setHostNickname, creating, onChoose, onBack }: { games: GameSummary[]; rounds: number; setRounds: (value: number) => void; theme: string; setTheme: (value: string) => void; hostNickname: string; setHostNickname: (value: string) => void; creating: boolean; onChoose: (game: GameSummary) => void; onBack: () => void }) {
  const [selected, setSelected] = useState<GameSummary | null>(null); const [step, setStep] = useState(1);
  const back = () => { if (step > 1) setStep(step - 1); else if (selected) setSelected(null); else onBack(); };
  return <section className="library setup-flow view-enter"><div className="page-heading"><button className="back-button" onClick={back}>← Back</button><p className="eyebrow">{selected ? `Step ${step} of 3` : "Game library"}</p><h1>{!selected ? "Pick your game" : step === 1 ? "Who’s hosting?" : step === 2 ? "How long?" : "Choose a vibe"}</h1></div>
    {!selected && (games.length === 0 ? <Loading title="Loading the game library" subtitle="Finding tonight’s best games…" /> : <div className="game-grid playful-grid">{games.map((game, index) => <button className="game-tile compact-tile" style={{ "--delay": `${index * 35}ms` } as React.CSSProperties} key={game.id} onClick={() => { setSelected(game); setRounds(game.defaultRounds); }}><span className={`game-icon tone-${index % 4}`}>{iconFor(game.icon)}</span><span className="game-meta"><strong>{game.title}</strong><small>{game.experience === "competitive" ? "Competitive" : game.experience === "voting" ? "Voting" : "Casual"}</small></span><span className="tile-arrow">→</span></button>)}</div>)}
    {selected && <div className="setup-card glass-card"><div className="setup-progress"><i style={{ transform: `scaleX(${step / 3})` }}/></div>{step === 1 && <label className="setup-field"><span>Your display name</span><input autoFocus value={hostNickname} maxLength={20} onChange={(event) => setHostNickname(event.target.value)} placeholder="Afreen"/><small>You’ll answer and score with everyone else.</small></label>}{step === 2 && <div className="visual-picker"><span>Number of rounds</span>{[5,6,8,10].map(value => <button key={value} className={rounds === value ? "active" : ""} onClick={() => setRounds(value)}><strong>{value}</strong><small>rounds</small></button>)}</div>}{step === 3 && <div className="visual-picker themes"><span>Room theme</span>{THEMES.map(([id,label]) => <button key={id} className={theme === id ? "active" : ""} onClick={() => setTheme(id)}><i className={`theme-dot ${id}`}/><strong>{label}</strong></button>)}</div>}<div className="setup-actions"><button className="secondary" onClick={back}>Back</button><button className="primary" disabled={creating || (step === 1 && !hostNickname.trim())} onClick={() => step < 3 ? setStep(step + 1) : onChoose(selected)}>{step < 3 ? "Continue →" : creating ? "Creating…" : "Create room"}</button></div></div>}
  </section>;
}

function Settings({ settings, setSettings, onBack }: { settings: { sound: boolean; motion: boolean }; setSettings: (value: { sound: boolean; motion: boolean }) => void; onBack: () => void }) {
  return <section className="settings-page view-enter"><div className="page-heading"><button className="back-button" onClick={onBack}>← Back</button><p className="eyebrow">Preferences</p><h1>Make it yours.</h1></div><div className="settings-card">
    <SettingRow icon="♪" title="Game sounds" description="Lightweight cues for joins, reveals, and winners." checked={settings.sound} onChange={(sound) => { if (sound) playCue("join"); setSettings({ ...settings, sound }); }} />
    <SettingRow icon="✦" title="Motion effects" description="Smooth transitions and result animations." checked={settings.motion} onChange={(motion) => setSettings({ ...settings, motion })} />
    <div className="setting-note"><span>i</span><p><strong>Designed for the room</strong><br/>Player screens automatically adapt for phones and tablets.</p></div>
  </div></section>;
}

function SettingRow({ icon, title, description, checked, onChange }: { icon: string; title: string; description: string; checked: boolean; onChange: (value: boolean) => void }) {
  return <label className="setting-row"><span className="setting-icon">{icon}</span><span><strong>{title}</strong><small>{description}</small></span><input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} /><i /></label>;
}

function HostGame({ snapshot, connected, playerUrl, send, resetSession, games, rounds, setRounds, theme, setTheme, confirm }: { snapshot: RoomSnapshot; connected: boolean; playerUrl: string; send: (event: object) => void; resetSession: () => void; games: GameSummary[]; rounds: number; setRounds: (value:number)=>void; theme:string; setTheme:(value:string)=>void; confirm: (value: Confirmation) => void }) {
  if (snapshot.phase === "lobby") return <Lobby snapshot={snapshot} connected={connected} playerUrl={playerUrl} send={send} />;
  if (snapshot.phase === "choosing") return <InRoomGamePicker snapshot={snapshot} games={games} rounds={rounds} setRounds={setRounds} theme={theme} setTheme={setTheme} send={send}/>;
  if (snapshot.phase === "closed") return <Loading title="Room ended" subtitle="Taking everyone home…"/>;
  if (snapshot.phase === "playing") return <InteractiveRoundStage snapshot={snapshot} connected={connected} send={send} confirm={confirm} />;
  if (snapshot.phase === "results") return <ResultsStage snapshot={snapshot} connected={connected} send={send} />;
  return <FinalLeaderboard snapshot={snapshot} resetSession={resetSession} replay={() => send({type:"configure_game",gameId:snapshot.gameId,rounds:snapshot.maxRounds,theme:snapshot.theme})} chooseAnother={() => send({type:"choose_another_game"})} />;
}

function InRoomGamePicker({snapshot,games,rounds,setRounds,theme,setTheme,send}:{snapshot:RoomSnapshot;games:GameSummary[];rounds:number;setRounds:(v:number)=>void;theme:string;setTheme:(v:string)=>void;send:(e:object)=>void}) { const [selected,setSelected]=useState<GameSummary|null>(null); return <section className="library room-picker view-enter"><RoomCodeBar code={snapshot.roomCode}/><div className="page-heading"><p className="eyebrow">Same party, same code</p><h1>{selected?`Set up ${selected.title}`:"Pick the next game"}</h1></div>{!selected?<div className="game-grid playful-grid">{games.map((game,index)=><button className="game-tile compact-tile" key={game.id} onClick={()=>{setSelected(game);setRounds(game.defaultRounds)}}><span className={`game-icon tone-${index%4}`}>{iconFor(game.icon)}</span><span className="game-meta"><strong>{game.title}</strong><small>{game.experience}</small></span><span>→</span></button>)}</div>:<div className="setup-card glass-card"><div className="visual-picker"><span>Rounds</span>{[5,6,8,10].map(value=><button key={value} className={rounds===value?"active":""} onClick={()=>setRounds(value)}><strong>{value}</strong><small>rounds</small></button>)}</div><div className="visual-picker themes"><span>Theme</span>{THEMES.map(([id,label])=><button key={id} className={theme===id?"active":""} onClick={()=>setTheme(id)}><i className={`theme-dot ${id}`}/><strong>{label}</strong></button>)}</div><div className="setup-actions"><button className="secondary" onClick={()=>setSelected(null)}>Back</button><button className="primary" onClick={()=>send({type:"configure_game",gameId:selected.id,rounds,theme})}>Ready this game →</button></div></div>}</section>; }

function RoomCodeBar({code}:{code:string}) { const copy=()=>void navigator.clipboard?.writeText(code); return <div className="room-code-bar"><span>Party room</span><strong>{code}</strong><button onClick={copy}>Copy code</button></div>; }

function Lobby({ snapshot, connected, playerUrl, send }: { snapshot: RoomSnapshot; connected: boolean; playerUrl: string; send: (event: object) => void }) {
  return <div className="lobby view-enter">
    <section className="invite-card glass-card">
      <div className="game-pill"><span>✦</span>{snapshot.gameTitle}</div><p className="eyebrow">Join with room code</p><div className="room-code">{snapshot.roomCode}</div>
      <button className="copy-code secondary" onClick={() => void navigator.clipboard?.writeText(snapshot.roomCode)}>Copy / Share code</button>
      <div className="qr-wrap"><QRCodeSVG value={playerUrl} size={210} level="M" bgColor="#ffffff" fgColor="#151329" aria-label="QR code to join this room" /></div>
      <p className="join-address">Go to <strong>{new URL(playerUrl).host}</strong></p><p className="muted">Scan the code or enter it manually. Everyone joins the same room.</p>
    </section>
    <section className="roster-card glass-card"><div className="section-heading"><div><p className="eyebrow">Live lobby</p><h2>Your party</h2></div><span className="count">{snapshot.players.length}/12</span></div>
      <div className="players">{snapshot.players.map((player, index) => <div className={`player-row ${player.id === snapshot.hostPlayerId ? "is-host" : ""}`} key={player.id}><span className={`avatar tone-${index % 4}`}>{player.nickname.slice(0, 1).toUpperCase()}</span><span><strong>{player.nickname}{player.id === snapshot.hostPlayerId && <em className="host-badge">Host</em>}</strong><small><i className={player.connected ? "online" : ""}/>{player.connected ? "Connected" : "Reconnecting"}</small></span>{player.id !== snapshot.hostPlayerId && <button className="ghost danger" disabled={!connected} onClick={() => send({ type: "kick_player", playerId: player.id })}>Remove</button>}</div>)}</div>
      <div className="lobby-footer"><div><strong>{snapshot.maxRounds} rounds</strong><small>Fresh, balanced questions selected for this room</small></div><button className="primary start" disabled={!connected || snapshot.players.length < 2} onClick={() => send({ type: "start_game" })}>{snapshot.players.length < 2 ? "Waiting for 1 friend" : "Start game"}<span>→</span></button></div>
    </section>
  </div>;
}

function EmptyState() { return <div className="empty-state"><div><span/><span/><span/></div><strong>The room is ready</strong><p>Players will appear here as soon as they join.</p></div>; }

function RoundStage({ snapshot, connected, send, confirm }: { snapshot: RoomSnapshot; connected: boolean; send: (event: object) => void; confirm: (value: Confirmation) => void }) {
  const progress = snapshot.totalEligiblePlayerCount ? snapshot.submittedAnswerCount / snapshot.totalEligiblePlayerCount * 100 : 0;
  if (snapshot.gameId === "truth-or-dare") {
    const active = snapshot.players.find(player => player.id === snapshot.activePlayerId);
    return <section className="stage truth-stage glass-card view-enter"><div className="stage-top"><span className="round-badge">Turn {snapshot.round} <i/> {snapshot.maxRounds}</span><span className="category-badge">No scores</span></div><p className="eyebrow">The spotlight chooses</p><div className="spotlight-avatar">{active?.nickname.slice(0,1).toUpperCase()}</div><h1>{active?.nickname ?? "Choosing a player"}</h1>{snapshot.selectedAnswer ? <div className="challenge-card"><small>{snapshot.selectedAnswer.startsWith("Truth") ? "Truth" : "Dare"}</small><strong>{snapshot.selectedAnswer.replace(/^(Truth|Dare):\s*/, "")}</strong></div> : <p className="muted">Waiting for {active?.nickname ?? "the selected player"} to choose Truth or Dare.</p>}<div className="answer-progress"><div><span>Room confirmations</span><strong>{snapshot.completedCount} <small>/ {snapshot.players.length}</small></strong></div><div className="progress-track"><i style={{ transform: `scaleX(${snapshot.players.length ? snapshot.completedCount / snapshot.players.length : 0})` }}/></div></div><button className="ghost" onClick={() => confirm({ title: "End the game?", message: "This will close the current challenge.", action: () => send({ type: "end_game" }) })}>End game</button></section>;
  }
  return <section className="stage glass-card view-enter"><div className="stage-top"><span className="round-badge">Round {snapshot.round} <i/> {snapshot.maxRounds}</span><span className="category-badge">{snapshot.currentCategory}</span></div><p className="eyebrow">{snapshot.gameTitle}</p><h1>{snapshot.currentQuestion}</h1>
    {snapshot.responseMode !== "player_vote" && <div className={`host-options ${snapshot.gameId === "this-or-that" ? "visual" : ""}`}>{snapshot.currentOptions.map((option, index) => <div key={option}><VisualOption id={snapshot.currentVisualOptions[index] || (snapshot.gameId === "this-or-that" ? "generated" : undefined)} label={option}/><span>{String.fromCharCode(65 + index)}</span><strong>{option}</strong></div>)}</div>}
    <div className="answer-progress"><div><span>Answers locked</span><strong>{snapshot.submittedAnswerCount} <small>/ {snapshot.totalEligiblePlayerCount}</small></strong></div><div className="progress-track"><i style={{ width: `${progress}%` }}/></div></div>
    <div className="stage-actions"><button className="primary action" disabled={!connected} onClick={() => send({ type: "reveal_results" })}>Reveal results <span>✦</span></button><button className="ghost" disabled={!connected} onClick={() => confirm({ title: "End the game?", message: "Players will move straight to the final leaderboard.", action: () => send({ type: "end_game" }) })}>End game</button></div>
  </section>;
}

function InteractiveRoundStage({ snapshot, connected, send, confirm }: { snapshot: RoomSnapshot; connected: boolean; send: (event: object) => void; confirm: (value: Confirmation) => void }) {
  const [selected, setSelected] = useState<string | null>(null);
  useEffect(() => setSelected(null), [snapshot.round]);
  const progress = snapshot.totalEligiblePlayerCount ? snapshot.submittedAnswerCount / snapshot.totalEligiblePlayerCount : 0;
  const everyoneAnswered = snapshot.submittedAnswerCount >= snapshot.totalEligiblePlayerCount;
  const endGame = () => confirm({ title: "End the game?", message: "Everyone will move to this game’s final screen.", action: () => send({ type: "end_game" }) });

  if (snapshot.gameId === "truth-or-dare") {
    return <HostTruthOrDare snapshot={snapshot} connected={connected} send={send} endGame={endGame}/>;
  }

  const choices = snapshot.responseMode === "player_vote"
    ? snapshot.players.map(player => ({ id:player.id, label:player.nickname, visual:"" }))
    : snapshot.currentOptions.map((option,index) => ({ id:String(index), label:option, visual:snapshot.currentVisualOptions[index] ?? "" }));
  const lockAnswer = () => {
    if (selected === null) return;
    if (snapshot.responseMode === "player_vote") send({type:"submit_vote",round:snapshot.round,selectedPlayerId:selected});
    else send({type:"submit_answer",round:snapshot.round,answer:selected});
  };
  return <section className="stage host-player-stage glass-card view-enter"><div className="stage-top"><span className="round-badge">Round {snapshot.round} <i/> {snapshot.maxRounds}</span><span className="category-badge">{snapshot.currentCategory}</span></div><p className="eyebrow">{snapshot.gameTitle} · You’re playing</p><h1>{snapshot.currentQuestion}</h1>
    {!snapshot.viewerHasAnswered && <><div className={`host-play-choices ${snapshot.gameId === "this-or-that" ? "visual" : ""}`} role="radiogroup" aria-label="Choose your answer">{choices.map((choice,index) => <button type="button" role="radio" aria-checked={selected === choice.id} className={selected === choice.id ? "selected" : ""} key={choice.id} onClick={() => setSelected(choice.id)}><VisualOption id={choice.visual || (snapshot.gameId === "this-or-that" ? "generated" : undefined)} label={choice.label}/><span>{snapshot.responseMode === "player_vote" ? choice.label.slice(0,1).toUpperCase() : String.fromCharCode(65+index)}</span><strong>{choice.label}</strong><i>✓</i></button>)}</div><button className="primary host-lock" disabled={!connected || selected === null} onClick={lockAnswer}>Lock answer <span>→</span></button></>}
    {snapshot.viewerHasAnswered && <div className="host-locked"><span>✓</span><div><strong>Your answer is locked</strong><small>{everyoneAnswered ? "Everyone is ready. Reveal when you are." : "Waiting for the rest of the room."}</small></div></div>}
    <div className="answer-progress"><div><span>Answers locked</span><strong>{snapshot.submittedAnswerCount} <small>/ {snapshot.totalEligiblePlayerCount}</small></strong></div><div className="progress-track"><i style={{ transform: `scaleX(${progress})` }}/></div></div>
    <div className="stage-actions"><button className="primary action" disabled={!connected || !everyoneAnswered} onClick={() => send({ type: "reveal_results" })}>{everyoneAnswered ? "Reveal results" : "Waiting for everyone"} <span>✦</span></button><button className="ghost" disabled={!connected} onClick={endGame}>End game</button></div>
  </section>;
}

function HostTruthOrDare({snapshot,connected,send,endGame}:{snapshot:RoomSnapshot;connected:boolean;send:(event:object)=>void;endGame:()=>void}) {
  const [choice,setChoice]=useState<"truth"|"dare"|null>(null); useEffect(()=>setChoice(null),[snapshot.round]);
  const phase=snapshot.truthOrDarePhase??"choosing"; const isSpotlight=snapshot.viewerPlayerId===snapshot.spotlightPlayerId;
  const reaction=(emoji:string)=>send({type:"send_reaction",round:snapshot.round,emoji,reactionId:globalThis.crypto?.randomUUID?.()??`${Date.now()}-${Math.random()}`});
  useEffect(()=>{if(phase!=="spotlight"||(!snapshot.spotlightCompleted&&!snapshot.spotlightSkipped))return;const timer=window.setTimeout(()=>send({type:"next_spotlight",round:snapshot.round}),3000);return()=>clearTimeout(timer)},[phase,snapshot.spotlightCompleted,snapshot.spotlightSkipped,snapshot.spotlightIndex,snapshot.round]);
  if(phase==="choosing"||phase==="preparing_reveal")return <section className="stage truth-stage glass-card view-enter"><div className="stage-top"><span className="round-badge">Round {snapshot.round} <i/> {snapshot.maxRounds}</span><span className="category-badge">Everyone chooses</span></div><p className="eyebrow">Your private choice</p><h1>Truth or Dare?</h1>{!snapshot.viewerHasAnswered&&<><div className="truth-choice-grid" role="radiogroup" aria-label="Choose Truth or Dare"><button role="radio" aria-checked={choice==="truth"} className={`truth-option ${choice==="truth"?"selected":""}`} onClick={()=>setChoice("truth")}><span>◌</span><strong>Truth</strong><small>Answer something honestly</small><i>✓</i></button><button role="radio" aria-checked={choice==="dare"} className={`dare-option ${choice==="dare"?"selected":""}`} onClick={()=>setChoice("dare")}><span>ϟ</span><strong>Dare</strong><small>Take on a playful challenge</small><i>✓</i></button></div><button className="primary host-lock" disabled={!connected||!choice} onClick={()=>choice&&send({type:"submit_answer",round:snapshot.round,answer:choice})}>Lock choice →</button></>}{snapshot.viewerHasAnswered&&<div className="host-locked"><span>✓</span><div><strong>{snapshot.viewerTruthOrDareChoice} locked!</strong><small>{phase==="preparing_reveal"?"Everyone is ready.":"Waiting for the others…"}</small></div></div>}<div className="answer-progress"><div><span>Choices locked</span><strong>{snapshot.submittedAnswerCount} <small>/ {snapshot.totalEligiblePlayerCount}</small></strong></div></div><div className="host-control-panel"><small>Host controls</small><button className="primary action" disabled={!connected||phase!=="preparing_reveal"} onClick={()=>send({type:"reveal_results"})}>{phase==="preparing_reveal"?"Start Spotlight Reveal ✦":"Waiting for everyone"}</button><button className="ghost" onClick={endGame}>End game</button></div></section>;
  if(phase==="spotlight")return <section className="stage truth-stage spotlight-shared glass-card view-enter"><div className="stage-top"><span className="round-badge">Round {snapshot.round}</span><span className="category-badge">Player {(snapshot.spotlightIndex??0)+1} of {snapshot.spotlightOrder?.length??0}</span></div><p className="eyebrow">{isSpotlight?"You’re in the spotlight!":`${snapshot.spotlightPlayerName} is in the spotlight!`}</p><div className="spotlight-avatar">{snapshot.spotlightPlayerName?.slice(0,1).toUpperCase()}</div><h1>{snapshot.spotlightPlayerName} chose <em>{snapshot.spotlightChoice}</em></h1><div className={`challenge-card ${snapshot.spotlightChoice}`}><small>{snapshot.spotlightChoice}</small><strong>{snapshot.spotlightPrompt}</strong></div><div className="reaction-tray">{["😂","😱","👏","🔥","❤️"].map(emoji=><button key={emoji} onClick={()=>reaction(emoji)}>{emoji}</button>)}</div><div className="reaction-burst">{snapshot.reactions?.slice(-5).map(item=><span key={item.id}>{item.emoji}</span>)}</div>{isSpotlight&&!snapshot.spotlightCompleted&&!snapshot.spotlightSkipped&&<div className="spotlight-actions"><button className="primary" onClick={()=>send({type:"mark_completed",round:snapshot.round})}>Completed ✓</button><button className="secondary" disabled={!snapshot.viewerRerollAvailable} onClick={()=>send({type:"reroll_spotlight",round:snapshot.round})}>Reroll</button><button className="ghost" onClick={()=>send({type:"skip_spotlight",round:snapshot.round})}>Skip safely</button></div>}{(snapshot.spotlightCompleted||snapshot.spotlightSkipped)&&<p className="spotlight-finished">{snapshot.spotlightSkipped?"Prompt skipped — no pressure.":"Moment complete! 🎉"}</p>}<div className="host-control-panel"><small>Host controls</small>{!snapshot.spotlightCompleted&&!snapshot.spotlightSkipped&&<button className="ghost" onClick={()=>window.confirm("Skip this inactive player’s turn?")&&send({type:"force_skip_spotlight",round:snapshot.round})}>Skip inactive player</button>}<button className="primary action" disabled={!snapshot.spotlightCompleted&&!snapshot.spotlightSkipped} onClick={()=>send({type:"next_spotlight",round:snapshot.round})}>Next player →</button><button className="ghost" onClick={endGame}>End game</button></div></section>;
  if(phase==="highlight_voting")return <section className="stage truth-stage glass-card view-enter"><p className="eyebrow">Celebrate the round</p><h1>Pick the highlights</h1><div className="highlight-ballot">{snapshot.highlightCategories?.map(category=><div key={category.id}><h2>{category.emoji} {category.label}</h2><div>{category.eligiblePlayerIds.filter(id=>id!==snapshot.viewerPlayerId).map(id=><button className={snapshot.viewerHighlightVotes?.[category.id]===id?"selected":""} disabled={Boolean(snapshot.viewerHighlightVotes?.[category.id])} key={id} onClick={()=>send({type:"submit_highlight_vote",round:snapshot.round,categoryId:category.id,selectedPlayerId:id})}>{snapshot.players.find(player=>player.id===id)?.nickname}</button>)}</div></div>)}</div><button className="ghost" disabled={snapshot.viewerSkippedHighlightVoting} onClick={()=>send({type:"skip_highlight_voting",round:snapshot.round})}>Skip my voting</button><div className="host-control-panel"><small>Host controls · {snapshot.highlightReadyCount??0}/{snapshot.totalEligiblePlayerCount} finished</small><button className="primary action" disabled={(snapshot.highlightReadyCount??0)<snapshot.totalEligiblePlayerCount} onClick={()=>send({type:"reveal_highlights",round:snapshot.round})}>Reveal highlights ✦</button></div></section>;
  return <section className="stage truth-stage highlight-reveal glass-card view-enter"><Confetti/><p className="eyebrow">Round {snapshot.round} highlights</p><h1>Favourite moments</h1><div className="highlight-winners">{snapshot.highlightResults?.map(result=><div key={result.categoryId}><span>{result.emoji}</span><strong>{result.label}</strong><p>{result.winnerPlayerIds.map(id=>snapshot.players.find(player=>player.id===id)?.nickname).join(" & ")}</p></div>)}</div><button className="primary action" onClick={()=>send({type:"next_round"})}>{snapshot.round>=snapshot.maxRounds?"Finish game":"Next round →"}</button></section>;
}

function ResultsStage({ snapshot, connected, send }: { snapshot: RoomSnapshot; connected: boolean; send: (event: object) => void }) {
  const max = Math.max(0, ...(snapshot.results ?? []).map((result) => result.votes));
  const total = (snapshot.results ?? []).reduce((sum, result) => sum + result.votes, 0);
  const [seconds, setSeconds] = useState(3);
  useEffect(() => { if (snapshot.experience === "competitive") return; setSeconds(3); const tick = window.setInterval(() => setSeconds(value => Math.max(0, value - 1)), 1000); const advance = window.setTimeout(() => send({ type: "next_round" }), 3000); return () => { clearInterval(tick); clearTimeout(advance); }; }, [snapshot.version, snapshot.experience]);
  if (snapshot.experience !== "competitive") {
    if (snapshot.gameId === "most-likely-to") {
      const winner = snapshot.results?.[0]; const activeIndex = Math.max(0, snapshot.players.findIndex(player => player.id === winner?.playerId)); const titles = ["Main Character", "Chaos Magnet", "Drama Royalty", "Snack Legend", "Always Online", "Lucky One"];
      return <section className="results-stage spotlight-result glass-card view-enter"><Confetti/><p className="eyebrow">The room has spoken</p><div className="spotlight-avatar">{winner?.label.slice(0,1).toUpperCase()}</div><h1>{winner?.label}</h1><div className="fun-title">♛ {titles[(snapshot.round + activeIndex) % titles.length]}</div><p className="muted"><strong>{winner?.votes ?? 0}</strong> votes this round · next turn in {seconds}s</p></section>;
    }
    if (snapshot.gameId === "never-have-i-ever") {
      const yes = snapshot.results?.find(result => result.label === "I have")?.votes ?? 0;
      return <section className="results-stage social-result glass-card view-enter"><p className="eyebrow">Honesty unlocked</p><h1><em>{yes}</em> out of <em>{total}</em> players have done this 😭</h1><div className="split-results">{snapshot.results?.map(result => <div key={result.id}><strong>{result.label}</strong><b>{total ? Math.round(result.votes / total * 100) : 0}%</b><span>{result.votes} responses</span><i style={{ transform: `scaleX(${total ? result.votes / total : 0})` }}/></div>)}</div><p className="auto-next">Next round in {seconds}s</p></section>;
    }
    const winner = snapshot.results?.[0];
    return <section className="results-stage voting-result glass-card view-enter"><p className="eyebrow">{snapshot.gameId === "this-or-that" ? "The winning side" : "The room has decided"}</p><h1>{snapshot.currentQuestion}</h1><div className="split-results">{snapshot.results?.map(result => <div className={result.id === winner?.id ? "popular" : ""} key={result.id}><strong>{result.label}</strong><b><AnimatedNumber value={total ? Math.round(result.votes / total * 100) : 0}/>%</b><span>{result.votes} {result.votes === 1 ? "vote" : "votes"}</span><i style={{ transform: `scaleX(${total ? result.votes / total : 0})` }}/></div>)}</div><p className="popular-choice">Most popular: <strong>{winner?.label}</strong> · {total} total votes</p><p className="auto-next">Next round in {seconds}s</p></section>;
  }
  return <section className="results-stage glass-card view-enter"><Confetti/><div className="stage-top"><span className="round-badge">Round {snapshot.round} results</span><span className="category-badge">{snapshot.currentCategory}</span></div><h1>{snapshot.currentQuestion}</h1>
    <div className="result-bars">{snapshot.results?.map((result, index) => <div className={`result-bar ${result.isCorrect ? "correct" : ""}`} style={{ "--delay": `${index * 90}ms` } as React.CSSProperties} key={result.id}><span className="result-rank">{result.isCorrect ? "✓" : result.playerId ? result.label.slice(0,1).toUpperCase() : index + 1}</span><strong>{result.label}</strong><div><i style={{ width: `${max ? result.votes / max * 100 : 0}%` }}/></div><b><AnimatedNumber value={result.votes}/></b><small>{result.votes === 1 ? "vote" : "votes"}</small></div>)}</div>
    {snapshot.roundPoints.length > 0 && <div className="round-points">{snapshot.roundPoints.map(item => <span key={item.playerId}><strong>{item.nickname}</strong> +{item.points}</span>)}</div>}
    <MiniLeaderboard entries={snapshot.leaderboard.slice(0, 3)} />
    <button className="primary action" disabled={!connected} onClick={() => send({ type: "next_round" })}>{snapshot.round >= snapshot.maxRounds ? "See final leaderboard" : "Next round"}<span>→</span></button>
  </section>;
}

function MiniLeaderboard({ entries }: { entries: RoomSnapshot["leaderboard"] }) {
  return <div className="mini-leaderboard"><span>Overall standings</span>{entries.map((entry) => <div key={entry.playerId}><b>#{entry.rank}</b><strong>{entry.nickname}</strong><em>{entry.score} pts</em></div>)}</div>;
}

function FinalLeaderboard({ snapshot, resetSession, replay, chooseAnother }: { snapshot: RoomSnapshot; resetSession: () => void; replay: () => void; chooseAnother: () => void }) {
  if(snapshot.gameId==="truth-or-dare")return <section className="final-screen casual-final glass-card view-enter"><Confetti/><div className="thanks-icon">🎉</div><p className="eyebrow">Truth or Dare complete</p><h1>Your party highlights</h1><div className="highlight-winners">{snapshot.historicalHighlights?.map((result,index)=><div key={`${result.categoryId}-${index}`}><span>{result.emoji}</span><strong>{result.label}</strong><p>{result.winnerPlayerIds.map(id=>snapshot.players.find(player=>player.id===id)?.nickname).join(" & ")}</p></div>)}</div><div className="reaction-totals">{snapshot.players.map(player=><span key={player.id}><strong>{player.nickname}</strong> {snapshot.reactionTotals?.[player.id]??0} reactions</span>)}</div><EndActions replay={replay} chooseAnother={chooseAnother} home={resetSession}/></section>;
  if (snapshot.experience === "voting") {
    const highlights = (snapshot.roundHistory ?? []).map(round => ({ question: round.question, winner: round.results[0] })).slice(0,3);
    return <section className="final-screen glass-card view-enter"><p className="eyebrow">{snapshot.gameTitle} complete</p><h1>Round highlights</h1><p className="muted">No rankings—just the choices that defined the room.</p><div className="highlight-list">{highlights.map((item,index) => <div key={index}><span>{index+1}</span><p>{item.question}<strong>{item.winner?.label} · {item.winner?.votes ?? 0} votes</strong></p></div>)}</div><EndActions replay={replay} chooseAnother={chooseAnother} home={resetSession}/></section>;
  }
  if (snapshot.experience === "casual") return <section className="final-screen casual-final glass-card view-enter"><Confetti/><div className="thanks-icon">🎉</div><p className="eyebrow">{snapshot.gameTitle}</p><h1>Thanks for playing!</h1><p className="muted">Hope you had fun, laughed loudly, and learned something unexpected.</p><EndActions replay={replay} chooseAnother={chooseAnother} home={resetSession}/></section>;
  return <section className="final-screen glass-card view-enter"><Confetti/><div className="winner-glow"/><div className="trophy-icon">🏆</div><p className="eyebrow">{snapshot.gameTitle} complete</p><h1>Final leaderboard</h1><p className="muted">A champion has been crowned.</p><div className="podium-list">{snapshot.leaderboard.slice(0,3).map((entry, index) => <div className={index === 0 ? "champion" : ""} style={{ "--delay": `${index * 60}ms` } as React.CSSProperties} key={entry.playerId}><span>{index === 0 ? "♛" : `#${entry.rank}`}</span><strong>{entry.nickname}</strong><b><AnimatedNumber value={entry.score}/></b><small>points</small></div>)}</div><EndActions replay={replay} chooseAnother={chooseAnother} home={resetSession}/></section>;
}
function EndActions({ replay, chooseAnother, home }: { replay: () => void; chooseAnother: () => void; home: () => void }) { return <div className="end-actions"><button className="primary" onClick={chooseAnother}>Play another game</button><button className="secondary" onClick={replay}>Play again</button><button className="ghost" onClick={home}>Return home</button></div>; }

function ConnectionBadge({ connected }: { connected: boolean }) { return <div className={`connection ${connected ? "online" : ""}`}><span/>{connected ? "Live" : "Reconnecting…"}</div>; }
function Toast({ message }: { message: string }) { return <div className="toast" role="alert"><span>!</span>{message}</div>; }
function Loading({ title, subtitle }: { title: string; subtitle: string }) { return <section className="loading glass-card"><div className="spinner"/><h2>{title}</h2><p>{subtitle}</p></section>; }
function ConfirmDialog({ confirmation, close }: { confirmation: NonNullable<Confirmation>; close: () => void }) { const leaving=confirmation.title.startsWith("Leave"); const endingRoom=confirmation.title.startsWith("End room"); return <div className="modal-backdrop" role="presentation" onMouseDown={(event)=>{if(event.target===event.currentTarget)close();}}><div className="modal" role="dialog" aria-modal="true" aria-labelledby="confirm-title"><button className="modal-close" onClick={close} aria-label="Close">×</button><span className="modal-icon">!</span><h2 id="confirm-title">{confirmation.title}</h2><p>{confirmation.message}</p><div><button className="secondary" onClick={close}>{leaving?"No, stay":"Cancel"}</button><button className={`primary ${leaving||endingRoom?"danger-action":""}`} onClick={() => { close(); confirmation.action(); }}>{leaving?"Yes, leave":endingRoom?"End room":"End game"}</button></div></div></div>; }
function AnimatedNumber({ value }: { value: number }) { const [shown, setShown] = useState(0); useEffect(() => { let frame = 0; const started = performance.now(); const tick = (now: number) => { setShown(Math.round(value * Math.min(1, (now - started) / 550))); if (now - started < 550) frame = requestAnimationFrame(tick); }; frame = requestAnimationFrame(tick); return () => cancelAnimationFrame(frame); }, [value]); return <>{shown}</>; }
function Confetti() { return <div className="confetti" aria-hidden="true">{Array.from({ length: 14 }, (_, index) => <i key={index} style={{ "--i": index } as React.CSSProperties}/>)}</div>; }
