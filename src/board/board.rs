use crate::pieces::bishop::Bishop;
use crate::pieces::king::King;
use crate::pieces::knight::Knight;
use crate::pieces::pawn::Pawn;
use crate::pieces::pieces::{Color, Piece, PieceType, Position};
use crate::pieces::queen::Queen;
use crate::pieces::rook::Rook;

pub struct Board {
    pub board: Vec<Vec<Option<Box<dyn Piece>>>>,
}

impl Clone for Board {
    fn clone(&self) -> Self {
        let board = self.board.iter()
            .map(|row| row.iter().map(|cell| cell.as_ref().map(|p| p.clone_box())).collect())
            .collect();
        Self { board }
    }
}

impl Board {
    pub fn new() -> Self {
        let mut board: Vec<Vec<Option<Box<dyn Piece>>>> = (0..8)
            .map(|_| (0..8).map(|_| None).collect())
            .collect();

        board[0][0] = Some(Box::new(Rook::new(Color::Black)));
        board[0][1] = Some(Box::new(Knight::new(Color::Black)));
        board[0][2] = Some(Box::new(Bishop::new(Color::Black)));
        board[0][3] = Some(Box::new(Queen::new(Color::Black)));
        board[0][4] = Some(Box::new(King::new(Color::Black)));
        board[0][5] = Some(Box::new(Bishop::new(Color::Black)));
        board[0][6] = Some(Box::new(Knight::new(Color::Black)));
        board[0][7] = Some(Box::new(Rook::new(Color::Black)));

        for col in 0..8 {
            board[1][col] = Some(Box::new(Pawn::new(Color::Black)));
            board[6][col] = Some(Box::new(Pawn::new(Color::White)));
        }

        board[7][0] = Some(Box::new(Rook::new(Color::White)));
        board[7][1] = Some(Box::new(Knight::new(Color::White)));
        board[7][2] = Some(Box::new(Bishop::new(Color::White)));
        board[7][3] = Some(Box::new(Queen::new(Color::White)));
        board[7][4] = Some(Box::new(King::new(Color::White)));
        board[7][5] = Some(Box::new(Bishop::new(Color::White)));
        board[7][6] = Some(Box::new(Knight::new(Color::White)));
        board[7][7] = Some(Box::new(Rook::new(Color::White)));

        Self { board }
    }

    pub fn in_bounds(pos: Position) -> bool {
        pos.x < 8 && pos.y < 8
    }

    pub fn get_piece(&self, pos: Position) -> Option<&dyn Piece> {
        if !Self::in_bounds(pos) {
            return None;
        }

        self.board[pos.x as usize][pos.y as usize].as_deref()
    }

    pub fn is_empty(&self, pos: Position) -> bool {
        self.get_piece(pos).is_none()
    }

    pub fn has_enemy_piece(&self, pos: Position, color: Color) -> bool {
        match self.get_piece(pos) {
            Some(piece) => piece.color() != color,
            None => false,
        }
    }

    pub fn has_own_piece(&self, pos: Position, color: Color) -> bool {
        match self.get_piece(pos) {
            Some(piece) => piece.color() == color,
            None => false,
        }
    }

    pub fn move_piece(&mut self, from: Position, to: Position) -> bool {
        if !Self::in_bounds(from) || !Self::in_bounds(to) {
            return false;
        }

        if self.board[from.x as usize][from.y as usize].is_none() {
            return false;
        }

        let piece = self.board[from.x as usize][from.y as usize].take();

        self.board[to.x as usize][to.y as usize] = piece;

        true
    }

    pub fn apply_move(&self, from: Position, to: Position) -> Option<Board> {
        if !Self::in_bounds(from) || !Self::in_bounds(to) {
            return None;
        }
        if self.board[from.x as usize][from.y as usize].is_none() {
            return None;
        }
        let mut new_board = self.clone();
        new_board.move_piece(from, to);
        Some(new_board)
    }

    pub fn find_king(&self, color: Color) -> Option<Position> {
        for row in 0..8u8 {
            for col in 0..8u8 {
                let pos = Position { x: row, y: col };
                if let Some(piece) = self.get_piece(pos) {
                    if piece.color() == color && matches!(piece.piece_type(), PieceType::King) {
                        return Some(pos);
                    }
                }
            }
        }
        None
    }

    pub fn is_attacked(&self, pos: Position, by_color: Color) -> bool {
        for row in 0..8u8 {
            for col in 0..8u8 {
                let from = Position { x: row, y: col };
                if let Some(piece) = self.get_piece(from) {
                    if piece.color() == by_color && piece.possible_moves(from, self).contains(&pos) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn is_in_check(&self, color: Color) -> bool {
        let king_pos = match self.find_king(color) {
            Some(pos) => pos,
            None => return false,
        };
        let opponent = match color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };
        self.is_attacked(king_pos, opponent)
    }
}