import { useRef } from 'react';
import { GLYPH } from '../chess.js';

export default function Board({ board, myColor, turn, status, selected, legalMoves, lastMove, onSquareClick }) {
  const flip = myColor === 'black';
  const legalSet = new Set((legalMoves || []).map(m => `${m.x},${m.y}`));

  // Compute animation info synchronously during render when lastMove changes
  const prevLastMoveRef = useRef(null);
  const animRef = useRef(null);
  if (lastMove !== prevLastMoveRef.current) {
    prevLastMoveRef.current = lastMove;
    if (lastMove) {
      const { from, to } = lastMove;
      const fvr = flip ? 7 - from.x : from.x;
      const fvc = flip ? 7 - from.y : from.y;
      const tvr = flip ? 7 - to.x   : to.x;
      const tvc = flip ? 7 - to.y   : to.y;
      animRef.current = { key: `${to.x},${to.y}`, dx: fvc - tvc, dy: fvr - tvr };
    }
  }
  const anim = animRef.current;

  let kingCheckSq = null;
  if (status === 'check' || status === 'checkmate') {
    for (let r = 0; r < 8; r++) {
      for (let c = 0; c < 8; c++) {
        const p = board[r][c];
        if (p && p.type === 'king' && p.color === turn) kingCheckSq = `${r},${c}`;
      }
    }
  }

  const order = flip ? [7, 6, 5, 4, 3, 2, 1, 0] : [0, 1, 2, 3, 4, 5, 6, 7];
  const cells = [];
  for (const row of order) {
    for (const col of order) {
      const key = `${row},${col}`;
      const piece = board[row][col];
      const isLight = (row + col) % 2 === 0;

      let cls = 'sq ' + (isLight ? 'l' : 'd');
      if (selected && selected.row === row && selected.col === col) {
        cls += ' selected';
      } else if (kingCheckSq === key) {
        cls += ' in-check';
      } else if (lastMove &&
          ((lastMove.from.x === row && lastMove.from.y === col) ||
           (lastMove.to.x   === row && lastMove.to.y   === col))) {
        cls += ' last-move';
      }
      if (legalSet.has(key)) cls += piece ? ' legal-capture' : ' legal-dot';

      const isAnim = anim && anim.key === key;
      cells.push(
        <div key={key} className={cls}
             style={isAnim ? { zIndex: 100 } : undefined}
             onClick={() => onSquareClick(row, col)}>
          {piece && (
            <span className={`pc ${piece.color === 'white' ? 'w' : 'b'}${isAnim ? ' sliding' : ''}`}
                  style={isAnim ? { '--pc-from': `translate(${anim.dx * 12.5}cqw, ${anim.dy * 12.5}cqw)` } : undefined}>
              {GLYPH[piece.color][piece.type]}
            </span>
          )}
        </div>
      );
    }
  }

  const inCheck = status === 'check' || status === 'checkmate';
  return (
    <div className={`board-frame ${inCheck ? 'in-check-ring' : ''}`}>
      <div className="board">{cells}</div>
    </div>
  );
}
