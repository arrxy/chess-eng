use crate::board::board::Board;
use crate::pieces::pieces::{Color, Piece, PieceType, Position};

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

    fn can_move(&self, from: Position, to: Position) -> bool {
        if to.x > 7 || to.y > 7 {
            return false;
        }
        let dx = from.x.abs_diff(to.x);
        let dy = from.y.abs_diff(to.y);
        dx == dy && dx != 0
    }

    fn clone_box(&self) -> Box<dyn Piece> {
        Box::new(Self { color: self.color })
    }

    fn possible_moves(&self, from: Position, board: &Board) -> Vec<Position> {
        let dirs: [[i8; 2]; 4] = [
            [1, 1],
            [1, -1],
            [-1, 1],
            [-1, -1],
        ];

        let mut possible_positions: Vec<Position> = Vec::new();

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