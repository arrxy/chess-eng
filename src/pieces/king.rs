use crate::board::board::Board;
use crate::pieces::pieces::{ Color, Piece, PieceType, Position };

pub struct King {
    color: Color
}

impl King {
    pub fn new(color: Color) -> Self {
        Self {
            color
        }
    }
}

impl Piece for King {
    fn color(&self) -> Color {
        self.color
    }
    fn piece_type(&self) -> PieceType {
        PieceType::King
    }
    fn can_move(&self, from: Position, to: Position) -> bool {
        if to.x > 7 || to.y > 7 {
            return false
        }
        true
    }

    fn possible_moves(&self, from: Position, board: &Board) -> Vec<Position> {
        let dirs: [[i8;2];8] = [
            [0, 1], [0, -1], [1, 0], [-1, 0], [-1,-1], [1,1], [1,-1], [-1,1]
        ];
        let mut possible_positions: Vec<Position> = vec![];
        for [dx, dy] in dirs {
            let nx = from.x as i8 + dx;
            let ny = from.y as i8 + dy;
            if nx < 0 || ny < 0 || nx > 7 || ny > 7 {
                continue
            }
            let target = &board.board[nx as usize][ny as usize];
            if let Some(piece) = target {
                if piece.color() == self.color {
                    continue;
                }
            }
            possible_positions.push(Position {
                x: nx as u8,
                y: ny as u8
            })
        }
        possible_positions
    }
}