mod board;
mod pieces;

use crate::board::game::Game;
use crate::pieces::pieces::Position;

fn main() {
    let mut game = Game::new();

    let success = game.make_move(
        Position { x: 6, y: 4 },
        Position { x: 4, y: 4 },
    );

    println!("White pawn move success: {}", success);

    let success = game.make_move(
        Position { x: 1, y: 4 },
        Position { x: 3, y: 4 },
    );

    println!("Black pawn move success: {}", success);

    let success = game.make_move(
        Position { x: 7, y: 0 },
        Position { x: 5, y: 0 },
    );

    println!("White rook move success: {}", success);
}