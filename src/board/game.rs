use crate::board::board::Board;
use crate::board::player::Player;
use crate::pieces::bishop::Bishop;
use crate::pieces::knight::Knight;
use crate::pieces::pieces::{Color, Piece, PieceType, Position};
use crate::pieces::queen::Queen;
use crate::pieces::rook::Rook;

#[derive(Debug, PartialEq)]
pub enum GameStatus {
    Ongoing,
    Check,
    Checkmate,
    Stalemate,
}

pub struct Game {
    board: Board,
    white: Player,
    black: Player,
    turn: Color,
}

impl Game {
    pub fn new() -> Self {
        Self {
            board: Board::new(),
            white: Player::new("white".parse().unwrap(), Color::White),
            black: Player::new("black".parse().unwrap(), Color::Black),
            turn: Color::White,
        }
    }

    /// Reconstruct a Game from a pre-built Board and turn (used when loading from Redis).
    pub fn from_parts(board: Board, turn: Color) -> Self {
        Self {
            board,
            white: Player::new("white".to_string(), Color::White),
            black: Player::new("black".to_string(), Color::Black),
            turn,
        }
    }

    pub fn board(&self) -> &Board {
        &self.board
    }

    pub fn board_mut(&mut self) -> &mut Board {
        &mut self.board
    }

    pub fn current_turn(&self) -> Color {
        self.turn
    }

    pub fn current_player(&self) -> &Player {
        match self.turn {
            Color::White => &self.white,
            Color::Black => &self.black,
        }
    }

    pub fn white_player(&self) -> &Player {
        &self.white
    }

    pub fn black_player(&self) -> &Player {
        &self.black
    }

    pub fn make_move(
        &mut self,
        from: Position,
        to: Position,
        promotion: Option<PieceType>,
    ) -> bool {
        if !Board::in_bounds(from) || !Board::in_bounds(to) {
            return false;
        }

        let piece = match self.board.get_piece(from) {
            Some(piece) => piece,
            None => return false,
        };

        if piece.color() != self.turn {
            return false;
        }

        let color = piece.color();
        let mut pseudo_moves = piece.possible_moves(from, &self.board);
        if matches!(piece.piece_type(), PieceType::King) {
            pseudo_moves.extend(self.board.castling_moves(color));
        }

        if !pseudo_moves.contains(&to) {
            return false;
        }

        let new_board = match self.board.apply_move(from, to) {
            Some(b) => b,
            None => return false,
        };

        if new_board.is_in_check(color) {
            return false;
        }

        self.board.move_piece(from, to);
        self.apply_promotion(to, promotion);
        self.switch_turn();

        true
    }

    /// Replace a pawn that reached the last rank. Defaults to a queen when no
    /// (valid) choice was supplied.
    fn apply_promotion(&mut self, to: Position, choice: Option<PieceType>) {
        let color = match self.board.get_piece(to) {
            Some(p) if matches!(p.piece_type(), PieceType::Pawn) => p.color(),
            _ => return,
        };
        let last_rank = match color {
            Color::White => 0,
            Color::Black => 7,
        };
        if to.x != last_rank {
            return;
        }
        let promoted: Box<dyn Piece> = match choice {
            Some(PieceType::Rook) => Box::new(Rook::new(color)),
            Some(PieceType::Bishop) => Box::new(Bishop::new(color)),
            Some(PieceType::Knight) => Box::new(Knight::new(color)),
            _ => Box::new(Queen::new(color)),
        };
        self.board.board[to.x as usize][to.y as usize] = Some(promoted);
    }

    pub fn possible_moves_from(&self, from: Position) -> Vec<Position> {
        if !Board::in_bounds(from) {
            return vec![];
        }

        let piece = match self.board.get_piece(from) {
            Some(piece) => piece,
            None => return vec![],
        };

        if piece.color() != self.turn {
            return vec![];
        }

        let color = piece.color();
        let mut moves = piece.possible_moves(from, &self.board);
        if matches!(piece.piece_type(), PieceType::King) {
            moves.extend(self.board.castling_moves(color));
        }
        moves
            .into_iter()
            .filter(|&to| {
                self.board
                    .apply_move(from, to)
                    .map_or(false, |b| !b.is_in_check(color))
            })
            .collect()
    }

    pub fn can_current_player_move(&self, from: Position, to: Position) -> bool {
        self.possible_moves_from(from).contains(&to)
    }

    pub fn is_in_check(&self, color: Color) -> bool {
        self.board.is_in_check(color)
    }

    pub fn is_checkmate(&self, color: Color) -> bool {
        self.board.is_in_check(color) && !self.has_any_legal_move(color)
    }

    pub fn is_stalemate(&self, color: Color) -> bool {
        !self.board.is_in_check(color) && !self.has_any_legal_move(color)
    }

    pub fn status(&self) -> GameStatus {
        let color = self.turn;
        if self.is_checkmate(color) {
            GameStatus::Checkmate
        } else if self.is_stalemate(color) {
            GameStatus::Stalemate
        } else if self.is_in_check(color) {
            GameStatus::Check
        } else {
            GameStatus::Ongoing
        }
    }

    fn has_any_legal_move(&self, color: Color) -> bool {
        for row in 0..8u8 {
            for col in 0..8u8 {
                let from = Position { x: row, y: col };
                if let Some(piece) = self.board.get_piece(from) {
                    if piece.color() == color {
                        let has_legal =
                            piece
                                .possible_moves(from, &self.board)
                                .into_iter()
                                .any(|to| {
                                    self.board
                                        .apply_move(from, to)
                                        .map_or(false, |b| !b.is_in_check(color))
                                });
                        if has_legal {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn switch_turn(&mut self) {
        self.turn = match self.turn {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pieces::king::King;
    use crate::pieces::pawn::Pawn;

    fn mv(game: &mut Game, from: (u8, u8), to: (u8, u8)) -> bool {
        game.make_move(
            Position {
                x: from.0,
                y: from.1,
            },
            Position { x: to.0, y: to.1 },
            None,
        )
    }

    /// e4 / Bc4 / Nf3 with quiet black replies, leaving white ready to castle.
    fn open_white_kingside(g: &mut Game) {
        assert!(mv(g, (6, 4), (4, 4)));
        assert!(mv(g, (1, 0), (2, 0)));
        assert!(mv(g, (7, 5), (4, 2)));
        assert!(mv(g, (1, 1), (2, 1)));
        assert!(mv(g, (7, 6), (5, 5)));
        assert!(mv(g, (1, 2), (2, 2)));
    }

    #[test]
    fn kingside_castling_moves_king_and_rook() {
        let mut g = Game::new();
        open_white_kingside(&mut g);
        assert!(mv(&mut g, (7, 4), (7, 6)));
        let king = g.board().get_piece(Position { x: 7, y: 6 }).unwrap();
        assert!(matches!(king.piece_type(), PieceType::King));
        let rook = g.board().get_piece(Position { x: 7, y: 5 }).unwrap();
        assert!(matches!(rook.piece_type(), PieceType::Rook));
        assert!(g.board().get_piece(Position { x: 7, y: 4 }).is_none());
        assert!(g.board().get_piece(Position { x: 7, y: 7 }).is_none());
    }

    #[test]
    fn castling_right_lost_after_king_moves() {
        let mut g = Game::new();
        open_white_kingside(&mut g);
        assert!(mv(&mut g, (7, 4), (7, 5)));
        assert!(mv(&mut g, (1, 3), (2, 3)));
        assert!(mv(&mut g, (7, 5), (7, 4)));
        assert!(mv(&mut g, (1, 4), (2, 4)));
        assert!(!mv(&mut g, (7, 4), (7, 6)));
    }

    #[test]
    fn en_passant_captures_the_passed_pawn() {
        let mut g = Game::new();
        assert!(mv(&mut g, (6, 4), (4, 4)));
        assert!(mv(&mut g, (1, 0), (2, 0)));
        assert!(mv(&mut g, (4, 4), (3, 4)));
        assert!(mv(&mut g, (1, 3), (3, 3))); // d5 double push past white's e5 pawn
        assert!(mv(&mut g, (3, 4), (2, 3))); // exd6 e.p.
        let pawn = g.board().get_piece(Position { x: 2, y: 3 }).unwrap();
        assert!(matches!(pawn.piece_type(), PieceType::Pawn));
        assert_eq!(pawn.color(), Color::White);
        assert!(g.board().get_piece(Position { x: 3, y: 3 }).is_none());
    }

    #[test]
    fn en_passant_expires_after_one_move() {
        let mut g = Game::new();
        assert!(mv(&mut g, (6, 4), (4, 4)));
        assert!(mv(&mut g, (1, 0), (2, 0)));
        assert!(mv(&mut g, (4, 4), (3, 4)));
        assert!(mv(&mut g, (1, 3), (3, 3))); // d5 double push
        assert!(mv(&mut g, (6, 0), (5, 0))); // white plays elsewhere
        assert!(mv(&mut g, (2, 0), (3, 0)));
        assert!(!mv(&mut g, (3, 4), (2, 3))); // e.p. no longer available
    }

    #[test]
    fn pawn_promotes_to_chosen_piece() {
        let mut g = Game::new();
        for row in g.board_mut().board.iter_mut() {
            for cell in row.iter_mut() {
                *cell = None;
            }
        }
        g.board_mut().board[1][0] = Some(Box::new(Pawn::new(Color::White)));
        g.board_mut().board[7][4] = Some(Box::new(King::new(Color::White)));
        g.board_mut().board[0][7] = Some(Box::new(King::new(Color::Black)));
        assert!(g.make_move(
            Position { x: 1, y: 0 },
            Position { x: 0, y: 0 },
            Some(PieceType::Knight),
        ));
        let p = g.board().get_piece(Position { x: 0, y: 0 }).unwrap();
        assert!(matches!(p.piece_type(), PieceType::Knight));
        assert_eq!(p.color(), Color::White);
    }

    #[test]
    fn pawn_promotes_to_queen_by_default() {
        let mut g = Game::new();
        for row in g.board_mut().board.iter_mut() {
            for cell in row.iter_mut() {
                *cell = None;
            }
        }
        g.board_mut().board[1][0] = Some(Box::new(Pawn::new(Color::White)));
        g.board_mut().board[7][4] = Some(Box::new(King::new(Color::White)));
        g.board_mut().board[2][7] = Some(Box::new(King::new(Color::Black)));
        assert!(mv(&mut g, (1, 0), (0, 0)));
        let p = g.board().get_piece(Position { x: 0, y: 0 }).unwrap();
        assert!(matches!(p.piece_type(), PieceType::Queen));
    }
}
