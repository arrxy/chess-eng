import { useState, useEffect } from 'react';

export function resultInfo(g) {
  if (g.result === 'draw') return { label: 'draw', cls: 'draw' };
  if (g.result === 'abandoned') return { label: 'abandoned', cls: 'abandoned' };
  const won = (g.result === 'white_won') === (g.your_color === 'white');
  return won ? { label: 'won', cls: 'win' } : { label: 'lost', cls: 'loss' };
}

export default function MyGames({ onBack, onReplay }) {
  const [games, setGames] = useState(null);
  const [err, setErr] = useState(null);
  useEffect(() => {
    fetch('/api/games')
      .then(r => { if (!r.ok) throw new Error(); return r.json(); })
      .then(d => setGames(d.games))
      .catch(() => setErr('Could not load your games.'));
  }, []);

  return (
    <div className="hist-wrap">
      <div className="card hist-card">
        <div className="hist-title">
          <span>my games</span>
          <button className="linklike" onClick={onBack}>← back</button>
        </div>
        {err && <div className="msg err">{err}</div>}
        {!err && games === null && <div className="msg">Loading…</div>}
        {games && games.length === 0 && <div className="msg">No games yet — finish a game while signed in.</div>}
        {games && games.map(g => {
          const opp = g.your_color === 'white'
            ? (g.black_name || 'Anonymous') : (g.white_name || 'Anonymous');
          const res = resultInfo(g);
          return (
            <div key={g.id} className="hist-row" onClick={() => onReplay(g)}>
              <span className="hist-opp">
                vs {opp} <span className="hist-date">as {g.your_color} · {g.moves.length} moves</span>
              </span>
              <span className="hist-date">{new Date(g.created_at).toLocaleDateString()}</span>
              <span className={`hist-result ${res.cls}`}>{res.label}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
