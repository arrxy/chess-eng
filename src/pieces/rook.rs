use crate::board::board::Board;
use crate::pieces::pieces::{Color, DIRS_4, Piece, PieceType, Position, sliding_moves};

pub struct Rook {
    color: Color,
}

impl Rook {
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl Piece for Rook {
    fn color(&self) -> Color {
        self.color
    }

    fn piece_type(&self) -> PieceType {
        PieceType::Rook
    }

    fn possible_moves(&self, from: Position, board: &Board) -> Vec<Position> {
        sliding_moves(&from, &DIRS_4, board, self.color)
    }

    fn clone_box(&self) -> Box<dyn Piece> {
        Box::new(Self { color: self.color })
    }
}
