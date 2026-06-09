// board.jsx — MiniBoard + EvalBar + small bits. Exports to window.
const { PIECE, parseFEN, isUpper } = window;

function MiniBoard({ fen, flip, coords }) {
  let cells = parseFEN(fen).map((p, i) => ({ p, i }));
  if (flip) cells = cells.slice().reverse();
  return (
    <div className="board">
      {cells.map(({ p }, idx) => {
        const r = Math.floor(idx / 8), c = idx % 8;
        const light = (r + c) % 2 === 0;
        return (
          <div key={idx} className={'sq ' + (light ? 'l' : 'd')}>
            {p && <span className={'pc ' + (isUpper(p) ? 'w' : 'b')}>{PIECE[p]}</span>}
          </div>
        );
      })}
    </div>
  );
}

// vertical eval bar, white advantage fills from the bottom
function EvalBar({ ev }) {
  const clamped = Math.max(-5, Math.min(5, ev));
  const whitePct = 50 + clamped * 9; // 0..100
  return (
    <div className="evalbar" title={fmtEval(ev)}>
      <div className="evalbar-white" style={{ height: whitePct + '%' }}></div>
    </div>
  );
}

function fmtEval(ev) {
  if (ev === 0) return '0.0';
  return (ev > 0 ? '+' : '') + ev.toFixed(1);
}

// last move chip: piece glyph (when a piece letter leads the SAN) + the move
function lastMoveGlyph(san, moverIsWhite) {
  const m = { K: 'K', Q: 'Q', R: 'R', B: 'B', N: 'N' };
  if (san && m[san[0]]) {
    const letter = moverIsWhite ? san[0] : san[0].toLowerCase();
    return PIECE[letter];
  }
  return null;
}

Object.assign(window, { MiniBoard, EvalBar, fmtEval, lastMoveGlyph });
