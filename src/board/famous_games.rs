//! Replays 300+ real games (World Championship matches 1886–2023 plus classic
//! checkmate miniatures) through the engine and cross-checks every move
//! against a fixture generated and validated with python-chess.
//!
//! What this validates:
//! - SAN resolution uses *our* move generation, so a missing legal move shows
//!   up as "no candidate found" and an extra illegal move as an ambiguity.
//! - Capture markers are checked against the board, including en passant.
//! - Check/checkmate markers are compared with `Game::status()` after every move.
//! - The final position must match python-chess's board exactly (catches
//!   silently-wrong mechanics like a misplaced castling rook).

use crate::board::board::Board;
use crate::board::game::{Game, GameStatus};
use crate::pieces::pieces::{Color, PieceType, Position};

#[derive(Debug)]
struct San {
    piece: PieceType,
    to: Position,
    file_hint: Option<u8>,
    rank_hint: Option<u8>,
    promotion: Option<PieceType>,
    is_capture: bool,
    gives_check: bool,
    gives_mate: bool,
    castle_kingside: bool,
    castle_queenside: bool,
}

fn piece_from_letter(c: u8) -> Option<PieceType> {
    match c {
        b'K' => Some(PieceType::King),
        b'Q' => Some(PieceType::Queen),
        b'R' => Some(PieceType::Rook),
        b'B' => Some(PieceType::Bishop),
        b'N' => Some(PieceType::Knight),
        _ => None,
    }
}

/// `e4` -> y=4 (file), x = 7 - 3 = 4 (rank 4 from white's side).
fn square(file: u8, rank: u8) -> Position {
    Position {
        x: 7 - (rank - b'1'),
        y: file - b'a',
    }
}

fn parse_san(raw: &str) -> San {
    let gives_mate = raw.contains('#');
    let gives_check = !gives_mate && raw.contains('+');
    let body: String = raw
        .chars()
        .filter(|c| !matches!(c, '+' | '#' | '!' | '?'))
        .collect();

    let mut san = San {
        piece: PieceType::Pawn,
        to: Position { x: 0, y: 0 },
        file_hint: None,
        rank_hint: None,
        promotion: None,
        is_capture: false,
        gives_check,
        gives_mate,
        castle_kingside: false,
        castle_queenside: false,
    };

    if body == "O-O-O" {
        san.piece = PieceType::King;
        san.castle_queenside = true;
        return san;
    }
    if body == "O-O" {
        san.piece = PieceType::King;
        san.castle_kingside = true;
        return san;
    }

    let mut chars: &[u8] = body.as_bytes();

    if let Some(p) = piece_from_letter(chars[0]) {
        san.piece = p;
        chars = &chars[1..];
    }

    // promotion suffix: "=Q"
    if chars.len() >= 2 && chars[chars.len() - 2] == b'=' {
        san.promotion = piece_from_letter(chars[chars.len() - 1]);
        assert!(san.promotion.is_some(), "bad promotion in {raw}");
        chars = &chars[..chars.len() - 2];
    }

    // last two chars are the target square
    assert!(chars.len() >= 2, "bad san {raw}");
    san.to = square(chars[chars.len() - 2], chars[chars.len() - 1]);
    chars = &chars[..chars.len() - 2];

    // what's left is disambiguation and/or the capture marker
    for &c in chars {
        match c {
            b'x' => san.is_capture = true,
            b'a'..=b'h' => san.file_hint = Some(c - b'a'),
            b'1'..=b'8' => san.rank_hint = Some(7 - (c - b'1')),
            _ => panic!("unexpected char {} in san {raw}", c as char),
        }
    }
    san
}

/// Find the one origin square from which the side to move can legally play
/// this SAN move, using the engine's own move generation.
fn resolve(game: &Game, san: &San, ctx: &str) -> Position {
    if san.castle_kingside || san.castle_queenside {
        let row = match game.current_turn() {
            Color::White => 7,
            Color::Black => 0,
        };
        return Position { x: row, y: 4 };
    }
    let mut candidates = vec![];
    for x in 0..8u8 {
        for y in 0..8u8 {
            if san.file_hint.is_some_and(|f| f != y) || san.rank_hint.is_some_and(|r| r != x) {
                continue;
            }
            let from = Position { x, y };
            let piece = match game.board().get_piece(from) {
                Some(p) => p,
                None => continue,
            };
            if piece.color() != game.current_turn() || piece.piece_type() != san.piece {
                continue;
            }
            if game.possible_moves_from(from).contains(&san.to) {
                candidates.push(from);
            }
        }
    }
    assert_eq!(
        candidates.len(),
        1,
        "{ctx}: expected exactly one piece able to play this move, found {} \
         (0 = engine rejects a legal move, 2+ = engine allows an illegal one)",
        candidates.len()
    );
    candidates[0]
}

fn castle_target(game: &Game, san: &San) -> Position {
    let row = match game.current_turn() {
        Color::White => 7,
        Color::Black => 0,
    };
    let y = if san.castle_kingside { 6 } else { 2 };
    Position { x: row, y }
}

/// FEN board field for the current position, e.g. "rnbqkbnr/pppppppp/8/...".
fn placement(board: &Board) -> String {
    let mut out = String::new();
    for x in 0..8usize {
        if x > 0 {
            out.push('/');
        }
        let mut empties = 0;
        for y in 0..8usize {
            match &board.board[x][y] {
                None => empties += 1,
                Some(p) => {
                    if empties > 0 {
                        out.push_str(&empties.to_string());
                        empties = 0;
                    }
                    let c = match p.piece_type() {
                        PieceType::King => 'k',
                        PieceType::Queen => 'q',
                        PieceType::Rook => 'r',
                        PieceType::Bishop => 'b',
                        PieceType::Knight => 'n',
                        PieceType::Pawn => 'p',
                        PieceType::Empty => unreachable!(),
                    };
                    out.push(if p.color() == Color::White {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    });
                }
            }
        }
        if empties > 0 {
            out.push_str(&empties.to_string());
        }
    }
    out
}

#[derive(Default)]
struct Stats {
    games: usize,
    moves: usize,
    castles: usize,
    en_passants: usize,
    promotions: usize,
    checks: usize,
    checkmates: usize,
    stalemates: usize,
}

fn replay_game(
    name: &str,
    result: &str,
    final_status: &str,
    final_pos: &str,
    moves: &str,
    stats: &mut Stats,
) {
    let mut game = Game::new();
    let sans: Vec<&str> = moves.split_whitespace().collect();
    let mut last_status = GameStatus::Ongoing;

    for (i, raw) in sans.iter().enumerate() {
        let ctx = format!("[{name}] move {} ({raw})", i + 1);
        let san = parse_san(raw);
        let mover = game.current_turn();
        let from = resolve(&game, &san, &ctx);
        let to = if san.castle_kingside || san.castle_queenside {
            castle_target(&game, &san)
        } else {
            san.to
        };

        // validate the capture marker against the board before moving
        let target_piece = game.board().get_piece(to).map(|p| p.piece_type());
        let is_ep = san.piece == PieceType::Pawn && from.y != to.y && target_piece.is_none();
        if san.is_capture {
            assert!(
                target_piece.is_some() || is_ep,
                "{ctx}: SAN says capture but target square is empty and it's not en passant"
            );
        } else {
            assert!(
                target_piece.is_none(),
                "{ctx}: SAN says quiet move but target is occupied"
            );
        }

        assert!(
            game.make_move(from, to, san.promotion),
            "{ctx}: make_move rejected the move"
        );

        // verify the check/mate marker against the engine
        last_status = game.status();
        if san.gives_mate {
            assert_eq!(last_status, GameStatus::Checkmate, "{ctx}: SAN says mate");
        } else if san.gives_check {
            assert_eq!(last_status, GameStatus::Check, "{ctx}: SAN says check");
        } else {
            assert!(
                matches!(last_status, GameStatus::Ongoing | GameStatus::Stalemate),
                "{ctx}: engine reports {last_status:?} but SAN has no check marker"
            );
        }

        stats.moves += 1;
        if san.castle_kingside || san.castle_queenside {
            stats.castles += 1;
        }
        if is_ep {
            stats.en_passants += 1;
        }
        if san.promotion.is_some() {
            stats.promotions += 1;
        }
        if san.gives_check {
            stats.checks += 1;
        }
        if san.gives_mate {
            stats.checkmates += 1;
            // the loser is the side to move; the result must agree
            let expected = match mover {
                Color::White => "1-0",
                Color::Black => "0-1",
            };
            assert_eq!(
                result, expected,
                "{ctx}: checkmate winner disagrees with game result"
            );
        }
    }

    let status_str = match last_status {
        GameStatus::Ongoing => "ongoing",
        GameStatus::Check => "check",
        GameStatus::Checkmate => "checkmate",
        GameStatus::Stalemate => "stalemate",
    };
    assert_eq!(status_str, final_status, "[{name}] final status mismatch");
    if last_status == GameStatus::Stalemate {
        stats.stalemates += 1;
    }

    assert_eq!(
        placement(game.board()),
        final_pos,
        "[{name}] final position differs from python-chess"
    );
    stats.games += 1;
}

#[test]
fn replays_famous_games() {
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/famous_games.tsv"
    );
    let data = std::fs::read_to_string(fixture).expect("missing tests/fixtures/famous_games.tsv");

    let mut stats = Stats::default();
    for line in data.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(cols.len(), 5, "bad fixture line: {line}");
        replay_game(cols[0], cols[1], cols[2], cols[3], cols[4], &mut stats);
    }

    println!(
        "replayed {} games / {} moves: {} castles, {} en passants, {} promotions, \
         {} checks, {} checkmates, {} stalemates",
        stats.games,
        stats.moves,
        stats.castles,
        stats.en_passants,
        stats.promotions,
        stats.checks,
        stats.checkmates,
        stats.stalemates
    );

    // the corpus must actually exercise every special mechanic
    assert!(
        stats.games >= 100,
        "expected at least 100 games, got {}",
        stats.games
    );
    assert!(stats.castles >= 100, "corpus should contain many castles");
    assert!(
        stats.en_passants >= 5,
        "corpus should contain en passant captures"
    );
    assert!(stats.promotions >= 5, "corpus should contain promotions");
    assert!(stats.checkmates >= 3, "corpus should contain checkmates");
    assert!(stats.stalemates >= 1, "corpus should contain a stalemate");
}
