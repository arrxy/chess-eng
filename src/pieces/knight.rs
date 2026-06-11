use crate::board::board::Board;
use crate::pieces::pieces::{Color, DIRS_KNIGHT, Piece, PieceType, Position, step_moves};

pub struct Knight {
    color: Color,
}

impl Knight {
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl Piece for Knight {
    fn color(&self) -> Color {
        self.color
    }
    fn piece_type(&self) -> PieceType {
        PieceType::Knight
    }
    fn possible_moves(&self, from: Position, board: &Board) -> Vec<Position> {
        step_moves(&from, &DIRS_KNIGHT, board, self.color)
    }

    fn clone_box(&self) -> Box<dyn Piece> {
        Box::new(Self { color: self.color })
    }
}
