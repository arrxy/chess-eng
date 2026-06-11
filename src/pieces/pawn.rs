use crate::board::board::Board;
use crate::pieces::pieces::{Color, Piece, PieceType, Position};

pub struct Pawn {
    color: Color,
}

impl Pawn {
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl Piece for Pawn {
    fn color(&self) -> Color {
        self.color
    }

    fn piece_type(&self) -> PieceType {
        PieceType::Pawn
    }
    fn possible_moves(&self, from: Position, board: &Board) -> Vec<Position> {
        let mut possible_positions: Vec<Position> = vec![];

        let direction: i8 = match self.color {
            Color::White => -1,
            Color::Black => 1,
        };

        let start_row: u8 = match self.color {
            Color::White => 6,
            Color::Black => 1,
        };

        // 1-step forward
        let one_x = from.x as i8 + direction;

        if one_x >= 0 && one_x <= 7 {
            let one_step = Position {
                x: one_x as u8,
                y: from.y,
            };

            if board.board[one_step.x as usize][one_step.y as usize].is_none() {
                possible_positions.push(one_step);

                // 2-step forward from starting row
                let two_x = from.x as i8 + 2 * direction;

                if from.x == start_row && two_x >= 0 && two_x <= 7 {
                    let two_step = Position {
                        x: two_x as u8,
                        y: from.y,
                    };

                    if board.board[two_step.x as usize][two_step.y as usize].is_none() {
                        possible_positions.push(two_step);
                    }
                }
            }
        }

        // diagonal captures
        for dy in [-1, 1] {
            let nx = from.x as i8 + direction;
            let ny = from.y as i8 + dy;

            if nx < 0 || ny < 0 || nx > 7 || ny > 7 {
                continue;
            }

            let capture_pos = Position {
                x: nx as u8,
                y: ny as u8,
            };

            let target = &board.board[capture_pos.x as usize][capture_pos.y as usize];

            if let Some(piece) = target {
                if piece.color() != self.color {
                    possible_positions.push(capture_pos);
                }
            } else if board.en_passant_target == Some(capture_pos) {
                // the pawn being taken sits beside us, not on the target square
                let victim = &board.board[from.x as usize][capture_pos.y as usize];
                if victim.as_ref().map_or(false, |p| {
                    p.color() != self.color && matches!(p.piece_type(), PieceType::Pawn)
                }) {
                    possible_positions.push(capture_pos);
                }
            }
        }

        possible_positions
    }

    fn clone_box(&self) -> Box<dyn Piece> {
        Box::new(Self { color: self.color })
    }
}
