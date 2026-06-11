use crate::board::board::Board;
use crate::pieces::pieces::{Color, DIRS_8, Piece, PieceType, Position, sliding_moves};

pub struct Queen {
    color: Color,
}

impl Queen {
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl Piece for Queen {
    fn color(&self) -> Color {
        self.color
    }

    fn piece_type(&self) -> PieceType {
        PieceType::Queen
    }

    fn possible_moves(&self, from: Position, board: &Board) -> Vec<Position> {
        sliding_moves(&from, &DIRS_8, board, self.color)
    }

    fn clone_box(&self) -> Box<dyn Piece> {
        Box::new(Self { color: self.color })
    }
}
