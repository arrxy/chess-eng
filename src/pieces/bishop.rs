use crate::board::board::Board;
use crate::pieces::pieces::{Color, DIRS_DIAG, Piece, PieceType, Position, sliding_moves};

pub struct Bishop {
    color: Color,
}

impl Bishop {
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl Piece for Bishop {
    fn color(&self) -> Color {
        self.color
    }

    fn piece_type(&self) -> PieceType {
        PieceType::Bishop
    }
    fn possible_moves(&self, from: Position, board: &Board) -> Vec<Position> {
        sliding_moves(&from, &DIRS_DIAG, board, self.color)
    }

    fn clone_box(&self) -> Box<dyn Piece> {
        Box::new(Self { color: self.color })
    }
}
