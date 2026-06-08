use crate::board::board::Board;
use crate::board::player::Player;
use crate::pieces::pieces::{Color, Position};

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

    pub fn make_move(&mut self, from: Position, to: Position) -> bool {
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
        let pseudo_moves = piece.possible_moves(from, &self.board);

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
        self.switch_turn();

        true
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
        piece.possible_moves(from, &self.board)
            .into_iter()
            .filter(|&to| {
                self.board.apply_move(from, to)
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
                        let has_legal = piece.possible_moves(from, &self.board)
                            .into_iter()
                            .any(|to| {
                                self.board.apply_move(from, to)
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