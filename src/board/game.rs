use crate::board::board::Board;
use crate::board::player::Player;
use crate::pieces::pieces::{Color, Position};

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

        let possible_moves = piece.possible_moves(from, &self.board);

        if !possible_moves.contains(&to) {
            return false;
        }

        let moved = self.board.move_piece(from, to);

        if !moved {
            return false;
        }

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

        piece.possible_moves(from, &self.board)
    }

    pub fn can_current_player_move(&self, from: Position, to: Position) -> bool {
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

        piece.possible_moves(from, &self.board).contains(&to)
    }

    fn switch_turn(&mut self) {
        self.turn = match self.turn {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };
    }
}