// data.jsx — board parsing helpers + sample live-game data. Exports to window.
const PIECE = {
  K: '\u2654', Q: '\u2655', R: '\u2656', B: '\u2657', N: '\u2658', P: '\u2659',
  k: '\u265A', q: '\u265B', r: '\u265C', b: '\u265D', n: '\u265E', p: '\u265F'
};

function parseFEN(fen) {
  const rows = fen.split(' ')[0].split('/');
  const sq = [];
  for (const row of rows) {
    for (const ch of row) {
      if (/\d/.test(ch)) { for (let i = 0; i < +ch; i++) sq.push(null); }
      else sq.push(ch);
    }
  }
  while (sq.length < 64) sq.push(null);
  return sq.slice(0, 64);
}
const isUpper = (c) => c === c.toUpperCase();

const FENS = {
  italian:   'r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R',
  sicilian:  'r1bqkbnr/pp1ppppp/2n5/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R',
  french:    'r1bqk2r/ppp2ppp/2n1pn2/3p4/1b1P4/2NBPN2/PPP2PPP/R1BQK2R',
  ruy:       'r1bqkbnr/1ppp1ppp/p1n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R',
  qgd:       'rnbqkb1r/ppp1pppp/5n2/3p4/2PP4/2N5/PP2PPPP/R1BQKBNR',
  kid:       'rnbqk2r/ppppppbp/5np1/8/2PP4/2N5/PP2PPPP/R1BQKBNR',
  endgame:   '8/5pk1/6p1/7p/7P/2R3P1/5PK1/8',
  caro:      'rnbqkb1r/pp2pppp/2p2n2/3p4/2PP4/2N5/PP2PPPP/R1BQKBNR',
  midR:      'r2q1rk1/pp2bppp/2n1pn2/3p4/3P4/2NBPN2/PP3PPP/R1BQ1RK1',
  scotch:    'r1bqkbnr/pppp1ppp/2n5/8/3pP3/5N2/PPP2PPP/RNBQKB1R',
  pirc:      'rnbqkb1r/ppp1pp1p/3p1np1/8/3PP3/2N5/PPP2PPP/R1BQKBNR',
  rook_end:  '5rk1/5ppp/8/8/8/5PP1/4R1KP/8'
};

function g(id, w, b, wt, bt, turn, fenKey, last, ev) {
  return { id, w, b, wt, bt, turn, fen: FENS[fenKey], last, ev };
}

// turn = side whose clock is running ('w' | 'b')
const GAMES = [
  g(1,  'pawnstar',     'knightowl',     '5:21', '4:48', 'b', 'italian',  'Nf6',   0.4),
  g(2,  'zugzwang',     'fianchetto',    '2:09', '3:31', 'w', 'sicilian', 'Nc6',  -0.2),
  g(3,  'castle_keep',  'enpassant',     '8:54', '7:12', 'b', 'french',   'Bb4',   0.1),
  g(4,  'gambit_grrl',  'blunderbuss',   '1:18', '0:47', 'w', 'ruy',      'a6',    0.7),
  g(5,  'tempo_tom',    'skewer_sam',    '6:40', '6:55', 'b', 'qgd',      'Nf6',  -0.5),
  g(6,  'luftmeister',  'rookie_mist',   '3:02', '2:24', 'w', 'kid',      'g6',    0.3),
  g(7,  'forkyou',      'stalemate_k',   '0:38', '1:55', 'b', 'endgame',  'Rxc3', -1.8),
  g(8,  'bishop_bash',  'queens_g',      '9:11', '9:44', 'w', 'caro',     'c6',    0.0),
  g(9,  'nimzo_nina',   'petrov_pete',   '4:27', '3:50', 'b', 'midR',     'O-O',   0.6),
  g(10, 'open_sesame',  'closed_carl',   '2:46', '2:58', 'w', 'scotch',   'exd4',  0.9),
  g(11, 'hippo_hank',   'dragon_dee',    '7:33', '6:09', 'b', 'pirc',     'd6',   -0.3),
  g(12, 'pin_it_pam',   'wing_walter',   '0:54', '1:12', 'w', 'rook_end', 'Re2',   2.4),
  g(13, 'cleo_castles', 'mate_in_won',   '5:05', '4:39', 'b', 'italian',  'Bc5',   0.2),
  g(14, 'tal_tales',    'capa_blanca',   '3:48', '3:33', 'w', 'midR',     'Be7',  -0.1)
];

Object.assign(window, { PIECE, parseFEN, isUpper, FENS, GAMES });
