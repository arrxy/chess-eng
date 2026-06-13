import { useState, useEffect } from 'react';
import GoogleButton from './GoogleButton.jsx';

export default function Lobby({ onCreate, onJoin, onRejoin, msg, user, clientId, onUser, onAuthError, onMyGames }) {
  const [gid, setGid] = useState('');
  const [active, setActive] = useState([]);

  useEffect(() => {
    if (!user) { setActive([]); return; }
    fetch('/api/active-games')
      .then(r => r.ok ? r.json() : { games: [] })
      .then(d => setActive((d.games || []).filter(g => g.game_id)))
      .catch(() => setActive([]));
  }, [user]);

  return (
    <div className="lobby-wrap">
      <div className="card lobby-card">
        {!user && clientId && (
          <>
            <GoogleButton clientId={clientId} onUser={onUser} onError={onAuthError} />
            <div className="lobby-or">— or play as a guest —</div>
          </>
        )}
        <div className="lobby-title">start a game</div>
        <button className="btn btn-primary" onClick={onCreate}>Create Game</button>
        <hr className="lobby-hr" />
        <div className="join-row">
          <input className="inp" placeholder="Game ID" maxLength={8}
            value={gid} onChange={e => setGid(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && gid.trim() && onJoin(gid.trim())} />
          <button className="btn btn-outline"
            onClick={() => gid.trim() && onJoin(gid.trim())}>Join</button>
        </div>
        {active.length > 0 && (
          <>
            <hr className="lobby-hr" />
            <div className="active-list">
              <div className="active-title">rejoin a game</div>
              {active.map(g => (
                <div key={g.game_id} className="active-row">
                  <div className="arow-main">
                    <span className="arow-opp">vs {g.opponent}</span>
                    <span className="arow-sub">as {g.your_color} · {g.moves} moves</span>
                  </div>
                  <button className="btn btn-outline"
                    onClick={() => onRejoin(g.game_id, g.your_color)}>Rejoin</button>
                </div>
              ))}
            </div>
          </>
        )}
        {user && (
          <>
            <hr className="lobby-hr" />
            <button className="linklike" onClick={onMyGames}>my games →</button>
          </>
        )}
        {msg && <div className={`msg ${msg.err ? 'err' : 'ok'}`}>{msg.text}</div>}
      </div>
    </div>
  );
}
