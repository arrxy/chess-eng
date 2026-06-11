use crate::board::board::Board;
use crate::pieces::pieces::{Color, DIRS_8, Piece, PieceType, Position, step_moves};

pub struct King {
    color: Color,
}

impl King {
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl Piece for King {
    fn color(&self) -> Color {
        self.color
    }
    fn piece_type(&self) -> PieceType {
        PieceType::King
    }
    fn possible_moves(&self, from: Position, board: &Board) -> Vec<Position> {
        step_moves(&from, &DIRS_8, board, self.color)
    }

    fn clone_box(&self) -> Box<dyn Piece> {
        Box::new(Self { color: self.color })
    }
}
