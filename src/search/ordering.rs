use std::collections::HashMap;

use crate::move_generator::{Move, MoveList};
use crate::search::Engine;
use crate::transposition_table::NodeType::EXACT;

type OrderTable = HashMap<Move, i32>;

const HASH_MOVE_PRIORITY: [i32; 3] = [256, 128, 32];
const PV_NODE_PRIORITY: i32 = 64;

impl Engine {
    pub fn order_moves(&self, move_list: &mut MoveList) {
        let mut order_table = self.construct_order_table(move_list);

        self.apply_hash_move(&mut order_table, move_list);

        move_list.order_moves(&order_table);
    }

    fn construct_order_table(&self, move_list: &MoveList) -> OrderTable {
        let mut order_table = HashMap::<Move, i32>::with_capacity(move_list.get_moves().len());

        for piece_move in move_list.get_moves() {
            order_table.insert(*piece_move, 0);
        }

        order_table
    }

    fn apply_hash_move(&self, order_table: &mut OrderTable, move_list: &MoveList) {
        match self.transposition_table.get_from_zobrist(move_list.get_board().zobrist) {
            None => { return; }
            Some(transposition) => {

                let mut index = 0;

                // PV-Node
                let mut pv_node = 0;
                if transposition.node_type == EXACT {
                    pv_node = PV_NODE_PRIORITY;
                }

                for entry in transposition.best_moves {
                    match entry {
                        None => { return; }
                        Some(piece_move) => {
                            let old_value = order_table.get(&piece_move).unwrap();
                            order_table.insert(piece_move, old_value + HASH_MOVE_PRIORITY[index] + pv_node);
                            pv_node = 0;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // #[test]
    // fn test_construct_order_table() {
    //     let mut board = Board::new();
    //     board.read_fen(START_POSITION);
    //
    //     let move_list = MoveList::from_board(&mut board);
    //
    //     let time = Instant::now();
    //     move_list.construct_order_table();
    //     let duration = time.elapsed();
    //     println!("construct_order_table lasted for: {:?}", duration);
    // }
    //
    // #[test]
    // fn test_order_moves() {
    //     let mut board = Board::new();
    //     board.read_fen(START_POSITION);
    //
    //     let mut move_list = MoveList::from_board(&mut board);
    //     move_list.generate_moves();
    //     let mut transposition_table = TranspositionTable::with_capacity(256);
    //     transposition_table.put_position(move_list.board, 5, 200,
    //                                      &[Option::from(Move::new(11, 27, 0, PAWN)), Option::from(Move::new(12, 28, 0, PAWN)), None],
    //                                      EXACT);
    //
    //     let time = Instant::now();
    //     move_list.order_moves(&transposition_table);
    //     let duration = time.elapsed();
    //     println!("order_moves lasted for: {:?}", duration);
    // }
}