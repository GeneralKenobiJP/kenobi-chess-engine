use crate::board::Board;
use crate::move_generator::{MoveList, Move};
use crate::piece::Colour::{BLACK, WHITE};

/// Walks the move generation tree of strictly legal moves to count all the leaf nodes of a certain depth.
/// Used for debugging.
/// Outputs the number of nodes it generated, given a move list (containing a board with position) and a depth.
pub fn perft(move_list: &mut MoveList, depth: u32) -> u64 {
    let mut nodes = 0u64;

    move_list.generate_moves();

    if depth == 1 {
        // println!("No. of leaves: {}", move_list.get_moves().len());
        // println!("Main bitboard: {}", move_list.get_board().main_bitboard);
        // println!("Own bitboard: {}", move_list.get_board().colour_bitboards[move_list.get_board().active_player as usize]);
        return move_list.get_moves().len() as u64;
    }

    let mut moves = move_list.get_moves().clone();

    for piece_move in moves
    {
        move_list.make_move(&piece_move);
        nodes += perft(move_list, depth - 1);
        move_list.unmake_move(&piece_move);
    }

    nodes
}

/// Walks the move generation tree of strictly legal moves to count all the leaf nodes of a certain depth.
/// Used for debugging.
/// Outputs the number of nodes it generated, given a move list (containing a board with position) and a depth.
/// Logs in the console number of nodes descending from each first move.
pub fn perft_log(move_list: &mut MoveList, depth: u32) -> u64 {
    let mut nodes = 0u64;

    move_list.generate_moves();

    if depth == 1 {
        // println!("No. of leaves: {}", move_list.get_moves().len());
        // println!("Main bitboard: {}", move_list.get_board().main_bitboard);
        // println!("Own bitboard: {}", move_list.get_board().colour_bitboards[move_list.get_board().active_player as usize]);
        return move_list.get_moves().len() as u64;
    }

    let mut moves = move_list.get_moves().clone();

    for piece_move in moves
    {
        move_list.make_move(&piece_move);
        let descendants = perft(move_list, depth - 1);
        println!("{}: {}", piece_move.to_algebraic_notation(), descendants);
        nodes += descendants;
        move_list.unmake_move(&piece_move);
    }

    nodes
}

// struct TreeStats {
//     nodes: u64,
//     captures: u64,
//     en_passants: u64,
//     castles: u64,
//     promotions: u64,
//     checks: u64,
//     checkmates: u64
// }
//
// pub fn perft_stats(move_list: &mut MoveList, depth: u32, tree_stats: &mut TreeStats) {
//     let mut nodes = 0u64;
//
//     move_list.generate_moves();
//
//     if depth == 1 {
//         // println!("No. of leaves: {}", move_list.get_moves().len());
//         // println!("Main bitboard: {}", move_list.get_board().main_bitboard);
//         // println!("Own bitboard: {}", move_list.get_board().colour_bitboards[move_list.get_board().active_player as usize]);
//
//     }
//
//     let mut moves = move_list.get_moves().clone();
//
//     for piece_move in moves
//     {
//         move_list.make_move(&piece_move);
//         nodes += perft(move_list, depth - 1);
//         move_list.unmake_move(&piece_move);
//     }
//
//     nodes
// }
//
// impl TreeStats {
//     pub fn new() -> Self {
//         TreeStats {
//             nodes: 0,
//             captures: 0,
//             en_passants: 0,
//             castles: 0,
//             promotions: 0,
//             checks: 0,
//             checkmates: 0
//         }
//     }
// }



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

        let depth = 2;

        let start = Instant::now();
        let nodes = perft(&mut move_list, depth);
        let duration = start.elapsed();
        println!("perft {} returned {} nodes", depth, nodes);
        println!("bench_perft lasted for: {:?}", duration);
    }

    #[test]
    fn perft_log_test() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);
        let mut move_list = MoveList::new(&mut board);

        let depth = 3;

        let nodes = perft_log(&mut move_list, depth);
        println!("perft {} returned {} nodes", depth, nodes);
    }

    #[test]
    fn perft_check() {
        let mut board = Board::new();
        board.read_fen("rnbqkbnr/pppppppp/8/8/8/P7/1PPPPPPP/RNBQKBNR b KQkq - 0 1");
        let mut move_list = MoveList::new(&mut board);

        let depth = 2;

        let nodes = perft_log(&mut move_list, depth);
        println!("perft {} returned {} nodes", depth, nodes);
    }
}