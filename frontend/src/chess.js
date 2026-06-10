// Shared chess constants and pure helpers.

// U+FE0E forces text presentation — without it iOS renders ♟ as an emoji,
// ignoring the CSS color that distinguishes white pieces from black.
export const GLYPH = {
  white: { king: '♚︎', queen: '♛︎', rook: '♜︎', bishop: '♝︎', knight: '♞︎', pawn: '♟︎' },
  black: { king: '♚︎', queen: '♛︎', rook: '♜︎', bishop: '♝︎', knight: '♞︎', pawn: '♟︎' },
};

export const PIECE_VALUE = { queen: 9, rook: 5, bishop: 3, knight: 3, pawn: 1, king: 0 };

export function capScore(pieces) {
  return (pieces || []).reduce((s, t) => s + (PIECE_VALUE[t] || 0), 0);
}

// Mirrors Board::new() on the server: row 0 = black back rank, row 7 = white.
export function startBoard() {
  const back = ['rook', 'knight', 'bishop', 'queen', 'king', 'bishop', 'knight', 'rook'];
  const b = Array(8).fill(null).map(() => Array(8).fill(null));
  back.forEach((t, c) => {
    b[0][c] = { type: t, color: 'black' };
    b[7][c] = { type: t, color: 'white' };
  });
  for (let c = 0; c < 8; c++) {
    b[1][c] = { type: 'pawn', color: 'black' };
    b[6][c] = { type: 'pawn', color: 'white' };
  }
  return b;
}

// The engine has no castling/en passant/promotion yet, so replaying a move is
// just "piece slides from -> to" with any capture implied by the target square.
export function boardAfter(moves, n) {
  const b = startBoard();
  for (let i = 0; i < n; i++) {
    const m = moves[i];
    b[m.to.x][m.to.y] = b[m.from.x][m.from.y];
    b[m.from.x][m.from.y] = null;
  }
  return b;
}

export function materialAdv(board) {
  let b = 0, w = 0;
  for (let r = 0; r < 8; r++)
    for (let c = 0; c < 8; c++) {
      const p = board[r][c];
      if (!p) continue;
      if (p.color === 'black') b += PIECE_VALUE[p.type] || 0;
      else w += PIECE_VALUE[p.type] || 0;
    }
  const total = b + w || 1;
  return { bPct: (b / total) * 100, wPct: (w / total) * 100 };
}

export const EMPTY_BOARD = Array(8).fill(null).map(() => Array(8).fill(null));
