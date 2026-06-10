import { useState } from 'react';
import GoogleButton from './GoogleButton.jsx';

export default function Lobby({ onCreate, onJoin, msg, user, clientId, onUser, onAuthError, onMyGames }) {
  const [gid, setGid] = useState('');
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
