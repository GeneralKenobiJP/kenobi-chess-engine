//! Perft = performance test, move path enumeration
//!
//! Current perft correctness:
//! Initial position: up to perft 6
//! Position 2 (kiwipete): up to perft 3; perft 4 failed (-481)
//! Position 3: up to perft 6
//! Position 4: up to perft 4; perft 5 failed; perft 5 used to work

use crate::board::Board;
use crate::move_generator::{MoveList, Move};
use crate::piece::Colour::{BLACK, WHITE};

/// Walks the move generation tree of strictly legal moves to count all the leaf nodes of a certain depth.
/// Used for debugging.
/// Outputs the number of nodes it generated, given a move list (containing a board with position) and a depth.
pub fn perft(move_list: &mut MoveList, depth: u32) -> u64 {
    let mut nodes = 0u64;

    if depth == 0 {
        return 1;
    }

    move_list.generate_moves();

    let moves = move_list.get_moves().clone();

    for piece_move in moves
    {
        move_list.make_move(&piece_move);
        if !move_list.is_opponent_in_check() { nodes += perft(move_list, depth - 1); }
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

    if depth == 0 {
        println!("Main bitboard: {}", move_list.get_board().main_bitboard);
        return 1;
    }

    move_list.generate_moves();

    let moves = move_list.get_moves().clone();

    for piece_move in moves
    {
        move_list.make_move(&piece_move);
        let descendants = if !move_list.is_opponent_in_check() {perft(move_list, depth - 1)} else { 0 };
        println!("{}: {}", piece_move.to_algebraic_notation(), descendants);
        nodes += descendants;
        move_list.unmake_move(&piece_move);
    }

    nodes
}

#[cfg(test)]
mod tests {
    use std::time::Instant;
    use crate::board::START_POSITION;
    use crate::piece::Piece;
    use crate::piece::Piece::{KING, KNIGHT, PAWN, QUEEN};
    use super::*;

    #[test]
    fn bench_perft() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);
        let mut move_list = MoveList::new(&mut board);

        let depth = 5;

        let start = Instant::now();
        let nodes = perft(&mut move_list, depth);
        let duration = start.elapsed();
        println!("perft {} returned {} nodes", depth, nodes);
        println!("bench_perft lasted for: {:?}", duration);
    }

    #[test]
    fn perft_test() {
        let mut board = Board::new();
        board.read_fen("rnbqkbnr/ppppppp1/7p/8/8/P7/1PPPPPPP/RNBQKBNR w KQkq - 0 2");
        let mut move_list = MoveList::new(&mut board);

        let depth = 1;

        let nodes = perft(&mut move_list, depth);
        assert_eq!(19, nodes);
    }

    #[test]
    fn perft_checkmate() {
        let mut board = Board::new();
        board.read_fen("rnbqkbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 0 1");
        let mut move_list = MoveList::new(&mut board);

        let depth = 1;

        let nodes = perft(&mut move_list, depth);
        println!("perft {} returned {} nodes", depth, nodes);
        assert_eq!(0, nodes);
    }

    #[test]
    fn perft_position_1() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);
        let mut move_list = MoveList::new(&mut board);

        let expected_nodes = vec![20, 400, 8902, 197281, 4865609];

        for depth in 1..6 {
            let nodes = perft_log(&mut move_list, depth);
            println!("perft {} returned {} nodes", depth, nodes);
            assert_eq!(expected_nodes[depth as usize - 1], nodes);
        }
    }

    #[test]
    fn perft_position_2() {
        let mut board = Board::new();
        board.read_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        let mut move_list = MoveList::new(&mut board);

        move_list.generate_moves();
        println!("{:?}", move_list.get_moves());

        let depth = 4;

        let nodes = perft_log(&mut move_list, depth);
        println!("perft {} returned {} nodes", depth, nodes);
    }

    #[test]
    fn perft_position_3() {
        let mut board = Board::new();
        board.read_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1");
        let mut move_list = MoveList::new(&mut board);

        let expected_nodes = vec![14, 191, 2812, 43238, 674624, 11030083];

        for depth in 1..7 {
            let nodes = perft_log(&mut move_list, depth);
            println!("perft {} returned {} nodes", depth, nodes);
            assert_eq!(expected_nodes[depth as usize - 1], nodes);
        }
    }

    #[test]
    fn perft_position_4() {
        let mut board = Board::new();
        board.read_fen("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1");
        let mut move_list = MoveList::new(&mut board);

        // move_list.make_move(&Move{origin: 2, target: 10, promotion: 0, piece: Piece::ROOK});
        // move_list.make_move(&Move{origin: 14, target: 7, promotion: 2, piece: Piece::PAWN});
        // move_list.make_move(&Move{origin: 4, target: 7, promotion: 2, piece: QUEEN});

        move_list.generate_moves();
        println!("{:?}", move_list.get_moves());

        let depth = 5;

        let nodes = perft_log(&mut move_list, depth);
        println!("perft {} returned {} nodes", depth, nodes);
    }
}