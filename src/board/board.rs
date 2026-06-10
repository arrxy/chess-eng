use crate::pieces::bishop::Bishop;
use crate::pieces::king::King;
use crate::pieces::knight::Knight;
use crate::pieces::pawn::Pawn;
use crate::pieces::pieces::{Color, Piece, PieceType, Position};
use crate::pieces::queen::Queen;
use crate::pieces::rook::Rook;

#[derive(Clone, Copy)]
pub struct CastlingRights {
    pub white_kingside: bool,
    pub white_queenside: bool,
    pub black_kingside: bool,
    pub black_queenside: bool,
}

impl CastlingRights {
    fn new() -> Self {
        Self {
            white_kingside: true,
            white_queenside: true,
            black_kingside: true,
            black_queenside: true,
        }
    }
}

pub struct Board {
    pub board: Vec<Vec<Option<Box<dyn Piece>>>>,
    /// Square a pawn can capture onto via en passant this turn (set after a double push).
    pub en_passant_target: Option<Position>,
    pub castling: CastlingRights,
}

impl Clone for Board {
    fn clone(&self) -> Self {
        let board = self.board.iter()
            .map(|row| row.iter().map(|cell| cell.as_ref().map(|p| p.clone_box())).collect())
            .collect();
        Self {
            board,
            en_passant_target: self.en_passant_target,
            castling: self.castling,
        }
    }
}

impl Board {
    pub fn new() -> Self {
        let mut board: Vec<Vec<Option<Box<dyn Piece>>>> = (0..8)
            .map(|_| (0..8).map(|_| None).collect())
            .collect();

        board[0][0] = Some(Box::new(Rook::new(Color::Black)));
        board[0][1] = Some(Box::new(Knight::new(Color::Black)));
        board[0][2] = Some(Box::new(Bishop::new(Color::Black)));
        board[0][3] = Some(Box::new(Queen::new(Color::Black)));
        board[0][4] = Some(Box::new(King::new(Color::Black)));
        board[0][5] = Some(Box::new(Bishop::new(Color::Black)));
        board[0][6] = Some(Box::new(Knight::new(Color::Black)));
        board[0][7] = Some(Box::new(Rook::new(Color::Black)));

        for col in 0..8 {
            board[1][col] = Some(Box::new(Pawn::new(Color::Black)));
            board[6][col] = Some(Box::new(Pawn::new(Color::White)));
        }

        board[7][0] = Some(Box::new(Rook::new(Color::White)));
        board[7][1] = Some(Box::new(Knight::new(Color::White)));
        board[7][2] = Some(Box::new(Bishop::new(Color::White)));
        board[7][3] = Some(Box::new(Queen::new(Color::White)));
        board[7][4] = Some(Box::new(King::new(Color::White)));
        board[7][5] = Some(Box::new(Bishop::new(Color::White)));
        board[7][6] = Some(Box::new(Knight::new(Color::White)));
        board[7][7] = Some(Box::new(Rook::new(Color::White)));

        Self {
            board,
            en_passant_target: None,
            castling: CastlingRights::new(),
        }
    }

    pub fn in_bounds(pos: Position) -> bool {
        pos.x < 8 && pos.y < 8
    }

    pub fn get_piece(&self, pos: Position) -> Option<&dyn Piece> {
        if !Self::in_bounds(pos) {
            return None;
        }

        self.board[pos.x as usize][pos.y as usize].as_deref()
    }

    pub fn is_empty(&self, pos: Position) -> bool {
        self.get_piece(pos).is_none()
    }

    pub fn has_enemy_piece(&self, pos: Position, color: Color) -> bool {
        match self.get_piece(pos) {
            Some(piece) => piece.color() != color,
            None => false,
        }
    }

    pub fn has_own_piece(&self, pos: Position, color: Color) -> bool {
        match self.get_piece(pos) {
            Some(piece) => piece.color() == color,
            None => false,
        }
    }

    pub fn move_piece(&mut self, from: Position, to: Position) -> bool {
        if !Self::in_bounds(from) || !Self::in_bounds(to) {
            return false;
        }

        if self.board[from.x as usize][from.y as usize].is_none() {
            return false;
        }

        let moved_type = self.board[from.x as usize][from.y as usize]
            .as_ref()
            .map(|p| p.piece_type())
            .unwrap();

        // en passant: a pawn capture landing on an empty square takes the
        // pawn that just double-pushed past it
        if matches!(moved_type, PieceType::Pawn)
            && from.y != to.y
            && self.board[to.x as usize][to.y as usize].is_none()
        {
            self.board[from.x as usize][to.y as usize] = None;
        }

        // castling: the king moves two files, the rook crosses over it
        if matches!(moved_type, PieceType::King) && (to.y as i8 - from.y as i8).abs() == 2 {
            let (rook_from, rook_to) = if to.y > from.y { (7usize, 5usize) } else { (0usize, 3usize) };
            let rook = self.board[from.x as usize][rook_from].take();
            self.board[from.x as usize][rook_to] = rook;
        }

        let piece = self.board[from.x as usize][from.y as usize].take();

        self.board[to.x as usize][to.y as usize] = piece;

        self.en_passant_target = if matches!(moved_type, PieceType::Pawn)
            && (to.x as i8 - from.x as i8).abs() == 2
        {
            Some(Position { x: (from.x + to.x) / 2, y: from.y })
        } else {
            None
        };

        self.update_castling_rights(from, to);

        true
    }

    /// Moving the king or a rook — or capturing a rook on its home square —
    /// forfeits the matching castling rights.
    fn update_castling_rights(&mut self, from: Position, to: Position) {
        for sq in [from, to] {
            match (sq.x, sq.y) {
                (7, 4) => {
                    self.castling.white_kingside = false;
                    self.castling.white_queenside = false;
                }
                (7, 0) => self.castling.white_queenside = false,
                (7, 7) => self.castling.white_kingside = false,
                (0, 4) => {
                    self.castling.black_kingside = false;
                    self.castling.black_queenside = false;
                }
                (0, 0) => self.castling.black_queenside = false,
                (0, 7) => self.castling.black_kingside = false,
                _ => {}
            }
        }
    }

    /// King destinations for legal castling. Kept out of `King::possible_moves`
    /// because the attack checks here would recurse through `is_attacked`.
    pub fn castling_moves(&self, color: Color) -> Vec<Position> {
        let row: u8 = match color {
            Color::White => 7,
            Color::Black => 0,
        };
        let (kingside, queenside) = match color {
            Color::White => (self.castling.white_kingside, self.castling.white_queenside),
            Color::Black => (self.castling.black_kingside, self.castling.black_queenside),
        };
        if !kingside && !queenside {
            return vec![];
        }
        let opponent = match color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };
        let king_home = Position { x: row, y: 4 };
        let king_at_home = self.get_piece(king_home).map_or(false, |p| {
            p.color() == color && matches!(p.piece_type(), PieceType::King)
        });
        if !king_at_home || self.is_attacked(king_home, opponent) {
            return vec![];
        }

        let mut moves = vec![];
        if kingside
            && self.is_empty(Position { x: row, y: 5 })
            && self.is_empty(Position { x: row, y: 6 })
            && !self.is_attacked(Position { x: row, y: 5 }, opponent)
            && !self.is_attacked(Position { x: row, y: 6 }, opponent)
        {
            moves.push(Position { x: row, y: 6 });
        }
        if queenside
            && self.is_empty(Position { x: row, y: 1 })
            && self.is_empty(Position { x: row, y: 2 })
            && self.is_empty(Position { x: row, y: 3 })
            && !self.is_attacked(Position { x: row, y: 3 }, opponent)
            && !self.is_attacked(Position { x: row, y: 2 }, opponent)
        {
            moves.push(Position { x: row, y: 2 });
        }
        moves
    }

    pub fn apply_move(&self, from: Position, to: Position) -> Option<Board> {
        if !Self::in_bounds(from) || !Self::in_bounds(to) {
            return None;
        }
        if self.board[from.x as usize][from.y as usize].is_none() {
            return None;
        }
        let mut new_board = self.clone();
        new_board.move_piece(from, to);
        Some(new_board)
    }

    pub fn find_king(&self, color: Color) -> Option<Position> {
        for row in 0..8u8 {
            for col in 0..8u8 {
                let pos = Position { x: row, y: col };
                if let Some(piece) = self.get_piece(pos) {
                    if piece.color() == color && matches!(piece.piece_type(), PieceType::King) {
                        return Some(pos);
                    }
                }
            }
        }
        None
    }

    pub fn is_attacked(&self, pos: Position, by_color: Color) -> bool {
        for row in 0..8u8 {
            for col in 0..8u8 {
                let from = Position { x: row, y: col };
                if let Some(piece) = self.get_piece(from) {
                    if piece.color() != by_color {
                        continue;
                    }
                    // pawns attack diagonals even when the square is empty,
                    // which possible_moves doesn't report
                    if matches!(piece.piece_type(), PieceType::Pawn) {
                        let dir: i8 = match by_color {
                            Color::White => -1,
                            Color::Black => 1,
                        };
                        if pos.x as i8 == from.x as i8 + dir
                            && (pos.y as i8 - from.y as i8).abs() == 1
                        {
                            return true;
                        }
                    } else if piece.possible_moves(from, self).contains(&pos) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn is_in_check(&self, color: Color) -> bool {
        let king_pos = match self.find_king(color) {
            Some(pos) => pos,
            None => return false,
        };
        let opponent = match color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };
        self.is_attacked(king_pos, opponent)
    }
}