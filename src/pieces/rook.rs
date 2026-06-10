use crate::board::board::Board;
use crate::pieces::pieces::{Color, Piece, PieceType, Position};

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

    fn can_move(&self, from: Position, to: Position) -> bool {
        if to.x >= 8 || to.y >= 8 {
            return false;
        }

        let same_row = from.x == to.x && from.y != to.y;
        let same_col = from.y == to.y && from.x != to.x;

        same_row || same_col
    }

    fn clone_box(&self) -> Box<dyn Piece> {
        Box::new(Self { color: self.color })
    }

    fn possible_moves(&self, from: Position, board: &Board) -> Vec<Position> {
        let dirs: [[i8; 2]; 4] = [[0, 1], [0, -1], [1, 0], [-1, 0]];

        let mut possible_positions: Vec<Position> = vec![];

        for [dx, dy] in dirs {
            let mut nx = from.x as i8 + dx;
            let mut ny = from.y as i8 + dy;

            while nx >= 0 && ny >= 0 && nx <= 7 && ny <= 7 {
                let target = &board.board[nx as usize][ny as usize];

                if let Some(piece) = target {
                    if piece.color() != self.color {
                        possible_positions.push(Position {
                            x: nx as u8,
                            y: ny as u8,
                        });
                    }
                    break;
                }

                possible_positions.push(Position {
                    x: nx as u8,
                    y: ny as u8,
                });

                nx += dx;
                ny += dy;
            }
        }

        possible_positions
    }
}
