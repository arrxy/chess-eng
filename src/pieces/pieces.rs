use crate::board::board::{Board};

#[derive(Copy, Clone, PartialEq)]
pub struct Position {
    pub x: u8,
    pub y: u8
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Color {
    White,
    Black
}

#[derive(Copy, Clone)]
pub enum PieceType {
    Empty,
    King,
    Queen,
    Rook,
    Bishop,
    Knight,
    Pawn
}
pub trait Piece {
    fn color(&self) -> Color;
    fn piece_type(&self) -> PieceType;
    fn can_move(&self, from: Position, to: Position) -> bool;
    fn possible_moves(&self, from: Position, board: &Board) -> Vec<Position>;
    fn clone_box(&self) -> Box<dyn Piece>;
}