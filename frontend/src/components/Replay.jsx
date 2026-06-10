import { useState, useEffect } from 'react';
import Board from './Board.jsx';
import { resultInfo } from './MyGames.jsx';
import { boardAfter } from '../chess.js';

export default function Replay({ game, onBack }) {
  const total = game.moves.length;
  const [idx, setIdx] = useState(0);
  const board = boardAfter(game.moves, idx);
  const lastMove = idx > 0
    ? { from: game.moves[idx - 1].from, to: game.moves[idx - 1].to }
    : null;
  const res = resultInfo(game);

  useEffect(() => {
    const onKey = (e) => {
      if (e.key === 'ArrowRight') setIdx(i => Math.min(total, i + 1));
      if (e.key === 'ArrowLeft') setIdx(i => Math.max(0, i - 1));
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [total]);

  return (
    <div className="game-wrap">
      <div className="card player-row">
        <span className="pname">
          {game.white_name || 'Anonymous'} vs {game.black_name || 'Anonymous'}
        </span>
        <span className={`hist-result ${res.cls}`}>{res.label}</span>
      </div>
      <div className="board-area">
        <Board board={board} myColor={game.your_color} turn={null} status="replay"
          selected={null} legalMoves={[]} lastMove={lastMove} onSquareClick={() => {}} />
      </div>
      <div className="replay-controls">
        <button className="replay-btn" disabled={idx === 0} onClick={() => setIdx(0)}>⏮</button>
        <button className="replay-btn" disabled={idx === 0}
          onClick={() => setIdx(i => Math.max(0, i - 1))}>◀</button>
        <span className="replay-count">{idx} / {total}</span>
        <button className="replay-btn" disabled={idx === total}
          onClick={() => setIdx(i => Math.min(total, i + 1))}>▶</button>
        <button className="replay-btn" disabled={idx === total}
          onClick={() => setIdx(total)}>⏭</button>
      </div>
      <button className="linklike" onClick={onBack}>← back to my games</button>
    </div>
  );
}
