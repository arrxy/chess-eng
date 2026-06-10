import Board from './Board.jsx';
import CapturedRow from './CapturedRow.jsx';
import { GLYPH, capScore, materialAdv } from '../chess.js';

const PROMOTION_CHOICES = ['queen', 'rook', 'bishop', 'knight'];

export default function GameView({ myColor, turn, status, board, selected, legalMoves, lastMove,
                                   gameId, gameStarted, msg, players, captured, user, promo,
                                   onSquareClick, onNewGame, onCopy, onPromote }) {
  const opponentColor = myColor === 'white' ? 'black' : 'white';
  const myTurn = turn === myColor;
  const gameOver = status === 'checkmate' || status === 'stalemate';

  const oppName = (players && players[opponentColor]) || opponentColor;
  const myName = (players && players[myColor]) || (user ? user.name : myColor);
  const capScores = {
    white: capScore(captured && captured.white),
    black: capScore(captured && captured.black),
  };
  const advFor = (color) =>
    Math.max(0, capScores[color] - capScores[color === 'white' ? 'black' : 'white']);

  const statusLabel = {
    waiting:   'Waiting for opponent…',
    ongoing:   myTurn ? 'Your turn' : `${turn}'s turn`,
    check:     myTurn ? '⚠ You are in check' : `⚠ ${turn} is in check`,
    checkmate: turn === 'white' ? 'Black wins — Checkmate' : 'White wins — Checkmate',
    stalemate: 'Stalemate — Draw',
  }[status] || '';

  return (
    <div className="game-wrap">
      {/* opponent */}
      <div className={`card player-row ${!myTurn && gameStarted ? 'active' : ''}`}>
        <span className="turn-dot" />
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 3 }}>
          <span className="pname">{oppName} <span className="ptag">({opponentColor})</span></span>
          <CapturedRow pieces={captured && captured[opponentColor]}
                       victimColor={myColor} adv={advFor(opponentColor)} />
        </div>
        <span className="ptag">{opponentColor === 'white' ? '♔' : '♚'}</span>
      </div>

      {/* share strip — white waiting for opponent */}
      {gameId && status === 'waiting' && myColor === 'white' && (
        <div className="card share-strip">
          <span>Share ID:</span>
          <code className="share-code">{gameId}</code>
          <button className="copy-btn" title="Copy" onClick={onCopy}>⊕</button>
        </div>
      )}

      {/* board */}
      <div className="board-area">
        <div className="board-with-bar">
          <div className="adv-bar">
            <div className="adv-b" style={{ height: `${materialAdv(board).bPct}%` }} />
            <div className="adv-w" style={{ height: `${materialAdv(board).wPct}%` }} />
          </div>
          <div className="board-wrap">
            <Board board={board} myColor={myColor} turn={turn} status={status}
              selected={selected} legalMoves={legalMoves} lastMove={lastMove}
              onSquareClick={onSquareClick} />
            {promo && (
              <div className="promo-overlay">
                <div className="card promo-card">
                  <span className="promo-title">Promote to</span>
                  <div className="promo-choices">
                    {PROMOTION_CHOICES.map(t => (
                      <button key={t} className="promo-btn" title={t}
                              onClick={() => onPromote(t)}>
                        <span className={`pc ${myColor === 'white' ? 'w' : 'b'}`}>
                          {GLYPH[myColor][t]}
                        </span>
                      </button>
                    ))}
                  </div>
                  <button className="linklike" onClick={() => onPromote(null)}>cancel</button>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* you */}
      <div className={`card player-row ${myTurn && gameStarted ? 'active' : ''}`}>
        <span className="turn-dot" />
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 3 }}>
          <span className="pname">{myName} <span className="ptag">({myColor} — you)</span></span>
          <CapturedRow pieces={captured && captured[myColor]}
                       victimColor={opponentColor} adv={advFor(myColor)} />
        </div>
        <span className="ptag">{myColor === 'white' ? '♔' : '♚'}</span>
      </div>

      <div className={`status-badge ${status}`}>{statusLabel}</div>

      {msg && <div className={`msg ${msg.err ? 'err' : 'ok'}`}>{msg.text}</div>}

      {gameOver && (
        <button className="btn btn-primary" style={{ maxWidth: 200 }} onClick={onNewGame}>
          New Game
        </button>
      )}
    </div>
  );
}
