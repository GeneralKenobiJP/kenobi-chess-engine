use std::collections::HashMap;
use crate::move_generator::{Move, MoveList};
use crate::transposition_table::TranspositionTable;

impl<'a> MoveList<'a> {
    pub fn order_moves(&mut self, transposition_table: &TranspositionTable) {
        let mut order_table = self.construct_order_table();

        self.moves.sort_by(|move_a, move_b| {
            let priority_a = order_table.get(move_a).unwrap();  // Default to 0 if not found
            let priority_b = order_table.get(move_b).unwrap();

            // Compare priorities
            priority_b.cmp(priority_a)  // Sort in descending order (higher priority first)
        });
    }

    fn construct_order_table(&self) -> HashMap<Move, i32> {
        let mut order_table = HashMap::<Move, i32>::with_capacity(self.moves.len());

        for piece_move in &self.moves {
            order_table.insert(*piece_move, 0);
        }

        order_table
    }
}

// fn hash_move(order_table: &mut Vec<Ordering>,move_list: &MoveList, transposition_table: &TranspositionTable) {
//
// }

#[cfg(test)]
mod tests {
    use std::time::Instant;
    use crate::board::{Board, START_POSITION};
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

        let time = Instant::now();
        move_list.order_moves(&TranspositionTable::with_capacity(1));
        let duration = time.elapsed();
        println!("order_moves lasted for: {:?}", duration);
    }
}