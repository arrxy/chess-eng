import { useState, useEffect, useRef, useCallback } from 'react';
import { EMPTY_BOARD } from './chess.js';
import TweaksPanel from './components/TweaksPanel.jsx';
import Lobby from './components/Lobby.jsx';
import GameView from './components/GameView.jsx';
import MyGames from './components/MyGames.jsx';
import Replay from './components/Replay.jsx';

export default function App() {
  const [tweaks, setTweaksState] = useState({ accent: '#3b6ea5', theme: 'warm', sketch: true });
  const setTweak = (k, v) => setTweaksState(prev => ({ ...prev, [k]: v }));

  useEffect(() => {
    document.documentElement.style.setProperty('--accent', tweaks.accent);
    document.documentElement.setAttribute('data-theme', tweaks.theme);
    document.documentElement.setAttribute('data-sketch', tweaks.sketch ? 'on' : 'off');
  }, [tweaks.accent, tweaks.theme, tweaks.sketch]);

  const [gameCount, setGameCount] = useState(0);
  useEffect(() => {
    const poll = () => fetch('/stats').then(r => r.json()).then(d => setGameCount(d.games)).catch(() => {});
    poll();
    const id = setInterval(poll, 5000);
    return () => clearInterval(id);
  }, []);

  // phase: lobby | game | history | replay
  const [phase, setPhase] = useState('lobby');
  const [myColor, setMyColor]     = useState(null);
  const [gameId, setGameId]       = useState(null);
  const [turn, setTurn]           = useState(null);
  const [status, setStatus]       = useState('waiting');
  const [board, setBoard]         = useState(EMPTY_BOARD);
  const [selected, setSelected]   = useState(null);
  const [legalMoves, setLegalMoves] = useState([]);
  const [lastMove, setLastMove]   = useState(null);
  const [msg, setMsg]             = useState(null);
  const [gameStarted, setGameStarted] = useState(false);
  const [players, setPlayers]     = useState(null);
  const [captured, setCaptured]   = useState({ white: [], black: [] });

  // auth
  const [user, setUser]           = useState(null);
  const [clientId, setClientId]   = useState(null);
  const [replayGame, setReplayGame] = useState(null);

  useEffect(() => {
    fetch('/auth/config').then(r => r.json())
      .then(d => setClientId(d.google_client_id)).catch(() => {});
    fetch('/auth/me').then(r => r.json())
      .then(d => setUser(d.user)).catch(() => {});
  }, []);

  const onLogout = useCallback(() => {
    fetch('/auth/logout', { method: 'POST' }).then(() => setUser(null)).catch(() => {});
  }, []);

  const wsRef = useRef(null);
  // stable refs so event handlers don't capture stale closures
  const myColorRef     = useRef(null);
  const turnRef        = useRef(null);
  const boardRef       = useRef(EMPTY_BOARD);
  const legalMovesRef  = useRef([]);
  const selectedRef    = useRef(null);
  const gameStartedRef = useRef(false);

  myColorRef.current    = myColor;
  turnRef.current       = turn;
  boardRef.current      = board;
  legalMovesRef.current = legalMoves;
  selectedRef.current   = selected;
  gameStartedRef.current = gameStarted;

  const showMsg = useCallback((text, err = false) => {
    setMsg({ text, err });
    setTimeout(() => setMsg(null), 3500);
  }, []);

  const handle = useCallback((m) => {
    switch (m.type) {
      case 'joined':
        setMyColor(m.color);
        setGameId(m.game_id);
        setPhase('game');
        setStatus('waiting');
        setGameStarted(false);
        setBoard(EMPTY_BOARD);
        setSelected(null);
        setLegalMoves([]);
        setLastMove(null);
        setPlayers(null);
        setCaptured({ white: [], black: [] });
        break;

      case 'state':
        setGameStarted(true);
        setBoard(m.board);
        setTurn(m.turn);
        setStatus(m.status);
        setSelected(null);
        setLegalMoves([]);
        if (m.players) setPlayers(m.players);
        if (m.captured) setCaptured(m.captured);
        if (m.lastMove) setLastMove(m.lastMove);
        if (m.status === 'checkmate') {
          const winner = m.turn === 'white' ? 'Black' : 'White';
          showMsg(`${winner} wins by checkmate!`);
        } else if (m.status === 'stalemate') {
          showMsg("Stalemate — it's a draw!");
        }
        break;

      case 'possible_moves':
        setLegalMoves(m.moves);
        break;

      case 'error':
        showMsg(m.message, true);
        break;

      case 'opponent_disconnected':
        showMsg('Opponent disconnected.', true);
        break;
    }
  }, [showMsg]);

  const connect = useCallback((onOpen) => {
    const proto = location.protocol === 'https:' ? 'wss' : 'ws';
    const ws = new WebSocket(`${proto}://${location.host}/ws`);
    ws.onopen = onOpen;
    ws.onmessage = e => handle(JSON.parse(e.data));
    ws.onclose = () => showMsg('Disconnected.', true);
    ws.onerror = () => showMsg('Connection error.', true);
    wsRef.current = ws;
  }, [handle, showMsg]);

  const send = useCallback((obj) => {
    const ws = wsRef.current;
    if (ws && ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(obj));
  }, []);

  const onSquareClick = useCallback((row, col) => {
    if (!gameStartedRef.current || turnRef.current !== myColorRef.current) return;
    const legalSet = new Set(legalMovesRef.current.map(m => `${m.x},${m.y}`));

    if (selectedRef.current && legalSet.has(`${row},${col}`)) {
      send({ type: 'move',
             from: { x: selectedRef.current.row, y: selectedRef.current.col },
             to:   { x: row, y: col } });
      setSelected(null);
      setLegalMoves([]);
      return;
    }

    const piece = boardRef.current && boardRef.current[row][col];
    if (piece && piece.color === myColorRef.current) {
      setSelected({ row, col });
      setLegalMoves([]);
      send({ type: 'moves', x: row, y: col });
    } else {
      setSelected(null);
      setLegalMoves([]);
    }
  }, [send]);

  const onCreate = useCallback(() => connect(() => send({ type: 'create' })), [connect, send]);
  const onJoin   = useCallback((gid) => connect(() => send({ type: 'join', game_id: gid })), [connect, send]);
  const onNewGame = useCallback(() => {
    wsRef.current?.close();
    setPhase('lobby'); setMyColor(null); setGameId(null);
    setTurn(null); setStatus('waiting'); setBoard(EMPTY_BOARD);
    setSelected(null); setLegalMoves([]); setLastMove(null);
    setMsg(null); setGameStarted(false);
    setPlayers(null); setCaptured({ white: [], black: [] });
  }, []);
  const onCopy = useCallback(() => {
    navigator.clipboard.writeText(gameId).then(() => showMsg('Copied!'));
  }, [gameId, showMsg]);

  const connected = phase === 'game';

  return (
    <>
      <header className="topbar">
        <div className="brand">
          <span className="brand-name">parallel</span>
          <span className="brand-sub">— chess</span>
          {gameCount > 0 && (
            <span className="games-live">
              <span className="live-dot" />
              {gameCount} {gameCount === 1 ? 'game' : 'games'} live
            </span>
          )}
        </div>
        <span className="auth-box">
          {user && (
            <>
              {user.picture && <img className="avatar" src={user.picture} alt="" referrerPolicy="no-referrer" />}
              <span className="auth-name">{user.name}</span>
              <button className="linklike" onClick={onLogout}>sign out</button>
            </>
          )}
          <span className="livecount">
            <span className={`live-dot ${connected ? '' : 'dim'}`} />
            {connected ? 'connected' : 'offline'}
          </span>
        </span>
      </header>
      <hr className="divider" />

      {phase === 'lobby' &&
        <Lobby onCreate={onCreate} onJoin={onJoin} msg={msg}
          user={user} clientId={clientId} onUser={setUser}
          onAuthError={(e) => showMsg(e, true)}
          onMyGames={() => setPhase('history')} />}
      {phase === 'game' &&
        <GameView
          myColor={myColor} turn={turn} status={status} board={board}
          selected={selected} legalMoves={legalMoves} lastMove={lastMove}
          gameId={gameId} gameStarted={gameStarted} msg={msg}
          players={players} captured={captured} user={user}
          onSquareClick={onSquareClick} onNewGame={onNewGame} onCopy={onCopy} />}
      {phase === 'history' &&
        <MyGames onBack={() => setPhase('lobby')}
          onReplay={(g) => { setReplayGame(g); setPhase('replay'); }} />}
      {phase === 'replay' && replayGame &&
        <Replay game={replayGame} onBack={() => setPhase('history')} />}

      <TweaksPanel tweaks={tweaks} setTweak={setTweak} />
    </>
  );
}
