use crate::board::Board;
use crate::move_generator::{MoveList, Move};

// pub fn perft(board: &mut Board, depth: u32) {
//     let mut move_list = MoveList::new(board);
//
//     if depth == 0 {
//         return;
//     }
//
//     move_list.generate_moves();
//
//     let mut moves = move_list.get_moves().clone();
//
//     for piece_move in moves
//     {
//         move_list.make_move(&piece_move);
//         perft(&mut move_list.get_board().clone(), depth - 1);
//         move_list.unmake_move(&piece_move);
//     }
// }
//
pub fn perft(move_list: &mut MoveList, depth: u32) {

    if depth == 0 {
        return;
    }

    move_list.generate_moves();

    let mut moves = move_list.get_moves().clone();

    for piece_move in moves
    {
        move_list.make_move(&piece_move);
        perft(move_list, depth - 1);
        move_list.unmake_move(&piece_move);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;
    use crate::board::START_POSITION;
    use super::*;

    #[test]
    fn bench_perft() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);
        let mut move_list = MoveList::new(&mut board);

        let start = Instant::now();
        perft(&mut move_list, 5);
        let duration = start.elapsed();
        println!("bench_perft lasted for: {:?}", duration);
    }
}