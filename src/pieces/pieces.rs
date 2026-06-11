use crate::board::board::Board;
use serde::{Deserialize, Serialize};

pub const DIRS_8: [[i8; 2]; 8] = [
    [0, 1],
    [0, -1],
    [1, 0],
    [-1, 0],
    [-1, -1],
    [1, 1],
    [1, -1],
    [-1, 1],
];

pub const DIRS_4: [[i8; 2]; 4] = [[0, 1], [0, -1], [1, 0], [-1, 0]];

pub const DIRS_DIAG: [[i8; 2]; 4] = [[1, 1], [1, -1], [-1, 1], [-1, -1]];

pub const DIRS_KNIGHT: [[i8; 2]; 8] = [
    [-2, -1],
    [-2, 1],
    [2, -1],
    [2, 1],
    [1, -2],
    [1, 2],
    [-1, -2],
    [-1, 2],
];

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Position {
    pub x: u8,
    pub y: u8,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    White,
    Black,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PieceType {
    Empty,
    King,
    Queen,
    Rook,
    Bishop,
    Knight,
    Pawn,
}
pub trait Piece: Send {
    fn color(&self) -> Color;
    fn piece_type(&self) -> PieceType;
    fn possible_moves(&self, from: Position, board: &Board) -> Vec<Position>;
    fn clone_box(&self) -> Box<dyn Piece>;
}

pub fn sliding_moves(
    from: &Position,
    dirs: &[[i8; 2]],
    board: &Board,
    color: Color,
) -> Vec<Position> {
    let mut possible_positions: Vec<Position> = vec![];
    for [dx, dy] in dirs {
        let mut nx = from.x as i8 + dx;
        let mut ny = from.y as i8 + dy;

        while nx >= 0 && ny >= 0 && nx <= 7 && ny <= 7 {
            let target = &board.board[nx as usize][ny as usize];
            if let Some(piece) = target {
                if piece.color() != color {
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

pub fn step_moves(from: &Position, dirs: &[[i8; 2]], board: &Board, color: Color) -> Vec<Position> {
    let mut possible_positions: Vec<Position> = vec![];
    for [dx, dy] in dirs {
        let nx = from.x as i8 + dx;
        let ny = from.y as i8 + dy;
        if nx < 0 || ny < 0 || nx > 7 || ny > 7 {
            continue;
        }
        let target = &board.board[nx as usize][ny as usize];
        if let Some(piece) = target {
            if piece.color() == color {
                continue;
            }
        }
        possible_positions.push(Position {
            x: nx as u8,
            y: ny as u8,
        })
    }
    possible_positions
}
