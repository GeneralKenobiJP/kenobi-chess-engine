use std::collections::HashMap;
use crate::move_generator::{Move, MoveList};
use crate::transposition_table::{Transposition, TranspositionTable};

type OrderTable = HashMap<Move, i32>;

const HASH_MOVE_PRIORITY: [i32; 3] = [256, 128, 32];

impl<'a> MoveList<'a> {
    pub fn order_moves(&mut self, transposition_table: &TranspositionTable) {
        let mut order_table = self.construct_order_table();

        self.apply_hash_move(&mut order_table, transposition_table);

        self.moves.sort_by(|move_a, move_b| {
            let priority_a = order_table.get(move_a).unwrap();  // Default to 0 if not found
            let priority_b = order_table.get(move_b).unwrap();

            // Compare priorities
            priority_b.cmp(priority_a)  // Sort in descending order (higher priority first)
        });
    }

    fn construct_order_table(&self) -> OrderTable {
        let mut order_table = HashMap::<Move, i32>::with_capacity(self.moves.len());

        for piece_move in &self.moves {
            order_table.insert(*piece_move, 0);
        }

        order_table
    }

    fn apply_hash_move(&self, order_table: &mut OrderTable, transposition_table: &TranspositionTable) {
        match transposition_table.get_from_zobrist(self.get_board().zobrist) {
            None => { return; }
            Some(transposition) => {

                let mut index = 0;

                for entry in transposition.best_moves {
                    match entry {
                        None => { return; }
                        Some(piece_move) => {
                            let old_value = order_table.get(&piece_move).unwrap();
                            order_table.insert(piece_move, old_value + HASH_MOVE_PRIORITY[index]);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;
    use crate::board::{Board, START_POSITION};
    use crate::piece::Piece::PAWN;
    use crate::transposition_table::NodeType::EXACT;
    use super::*;

    #[test]
    fn test_construct_order_table() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let move_list = MoveList::from_board(&mut board);

        let time = Instant::now();
        move_list.construct_order_table();
        let duration = time.elapsed();
        println!("construct_order_table lasted for: {:?}", duration);
    }

    #[test]
    fn test_order_moves() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut move_list = MoveList::from_board(&mut board);
        move_list.generate_moves();
        let mut transposition_table = TranspositionTable::with_capacity(256);
        transposition_table.put_position(move_list.board, 5, 200,
                                         &[Option::from(Move::new(11, 27, 0, PAWN)), Option::from(Move::new(12, 28, 0, PAWN)), None],
                                         EXACT);

        let time = Instant::now();
        move_list.order_moves(&transposition_table);
        let duration = time.elapsed();
        println!("order_moves lasted for: {:?}", duration);
    }
}