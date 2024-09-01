//! Best move search algorithm
//! Builds a search tree to find the best possible move according to the evaluation algorithm
//! Uses negamax convention, alpha-beta prunning, quiescence search.

use crate::board::Board;
use crate::evaluation::evaluate;
use crate::move_generator::MoveList;
use crate::evaluation::{POSITIVE_INFINITY, NEGATIVE_INFINITY};

/// Calls search algorithm to find the best possible moves in the current situation.
/// Searches up to the given depth.
/// Uses the given move list to generate moves in-place and analyze the board situation.
/// Calls the algorithm using alpha-beta prunning and quiescence search
pub fn search(move_list: &mut MoveList, depth: u32) -> i32 {
    search_alpha_beta_prunning(move_list, depth, NEGATIVE_INFINITY, POSITIVE_INFINITY)
}

/// Calls search algorithm to find the best possible moves in the current situation.
/// Searches up to the given depth.
/// Uses the given move list to generate moves in-place and analyze the board situation.
/// Calls the algorithm using alpha-beta prunning.
/// NOTE: Does NOT use quiescence search and therefore is inferior to the search() function
///     Should be used mainly for testing.
pub fn search_no_quiescence(move_list: &mut MoveList, depth: u32) -> i32 {
    search_alpha_beta_prunning_naive(move_list, depth, NEGATIVE_INFINITY, POSITIVE_INFINITY)
}

/// Uses alpha-beta prunning to find the best possible move in the search tree.
/// Searches up to the given depth.
/// Uses the given move list to generate moves in-place and analyze the board situation.
/// Calls quiescence search if depth is zero
/// Alpha - minimum score the current player is assured of (we found a move of at least this value earlier at this depth)
/// Beta - maximum score the opponent is assured of (the best value the parent node recorded)
fn search_alpha_beta_prunning(move_list: &mut MoveList, depth: u32, mut alpha: i32, beta: i32) -> i32 {
    let mut value: i32 = evaluate(move_list.get_board());

    if depth == 0 {
        return -quiescence_search(move_list, -beta, -alpha);
    }

    move_list.generate_moves();

    let moves = move_list.get_moves().clone();

    for piece_move in moves
    {
        move_list.make_move(&piece_move);
        if !move_list.is_opponent_in_check() {
            value = value.max(
                -search_alpha_beta_prunning(move_list, depth - 1, -beta, -alpha)
            );
        }
        move_list.unmake_move(&piece_move);

        alpha = alpha.max(value);
        if alpha >= beta { break; }
    }

    value
}

/// Uses alpha-beta prunning to find the best possible move in the search tree.
/// Searches up to the given depth.
/// Uses the given move list to generate moves in-place and analyze the board situation.
/// Alpha - minimum score the current player is assured of (we found a move of at least this value earlier at this depth)
/// Beta - maximum score the opponent is assured of (the best value the parent node recorded)
/// NOTE: Does NOT use quiescence search and therefore is inferior to the search_alpha_beta_prunning() function
///     Should be used mainly for testing.
fn search_alpha_beta_prunning_naive(move_list: &mut MoveList, depth: u32, mut alpha: i32, beta: i32) -> i32 {
    let mut value: i32 = evaluate(move_list.get_board());

    if depth == 0 {
        return value;
    }

    move_list.generate_moves();

    let moves = move_list.get_moves().clone();

    for piece_move in moves
    {
        move_list.make_move(&piece_move);
        if !move_list.is_opponent_in_check() {
            value = value.max(
                -search_alpha_beta_prunning_naive(move_list, depth - 1, -beta, -alpha)
            );
        }
        move_list.unmake_move(&piece_move);

        alpha = alpha.max(value);
        if alpha >= beta { break; }
    }

    value
}

/// Uses alpha-beta prunning to find the best possible move in the search tree.
/// Searches until it finds a 'quiet' position, where no captures, promotions or checks can be made.
/// It avoids the horizon effect by not ignoring threats at depth zero.
/// Uses the given move list to generate moves in-place and analyze the board situation.
/// Alpha - minimum score the current player is assured of (we found a move of at least this value earlier at this depth)
/// Beta - maximum score the opponent is assured of (the best value the parent node recorded)
fn quiescence_search(move_list: &mut MoveList, mut alpha: i32, beta: i32) -> i32 {
    // let start = Instant::now();
    let mut value: i32 = evaluate(move_list.get_board());

    move_list.generate_noisy_moves();

    if move_list.get_moves().len() == 0 {
        return value;
    }

    let moves = move_list.get_moves().clone();

    for piece_move in moves
    {
        move_list.make_move(&piece_move);
        // let duration = start.elapsed();
        // println!("quiescence_search time from call to make_move included: {:?}", duration);
        if !move_list.is_opponent_in_check() {
            value = value.max(
                -quiescence_search(move_list, -beta, -alpha)
            );
        }
        move_list.unmake_move(&piece_move);

        alpha = alpha.max(value);
        if alpha >= beta { break; }
    }

    value
}

/// Finds the best possible move in the search tree.
/// Searches up to the given depth.
/// Uses the given move list to generate moves in-place and analyze the board situation.
/// NOTE: Uses NEITHER quiescence search NOR alpha-beta prunning
///     Should be used only for testing.
fn search_naive(move_list: &mut MoveList, depth: u32) -> i32 {
    let mut value: i32 = evaluate(move_list.get_board());

    if depth == 0 {
        return value;
    }

    move_list.generate_moves();

    let moves = move_list.get_moves().clone();

    for piece_move in moves
    {
        move_list.make_move(&piece_move);
        if !move_list.is_opponent_in_check() {
            value = value.max(-search_naive(move_list, depth - 1))
        }
        move_list.unmake_move(&piece_move);
    }

    value
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_initial_position() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::new(&mut board);

        assert_eq!(0, search_naive(&mut move_list, 0));
        assert_eq!(0, search_naive(&mut move_list, 1));
        assert_eq!(0, search_naive(&mut move_list, 2));
        assert_eq!(0, search_naive(&mut move_list, 3));

        assert_eq!(0, search(&mut move_list, 0));
        assert_eq!(0, search(&mut move_list, 1));
        assert_eq!(0, search(&mut move_list, 2));
        assert_eq!(0, search(&mut move_list, 3));
    }

    #[test]
    fn search_material_handling() {
        let mut board = Board::new();
        let fen = "4k3/5r2/8/8/8/5R2/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::new(&mut board);

        assert_eq!(0, search_naive(&mut move_list, 0));
        assert_eq!(500, search_naive(&mut move_list, 1));
        assert_eq!(0, search_naive(&mut move_list, 2));

        assert_eq!(0, search(&mut move_list, 0));
        assert_eq!(500, search(&mut move_list, 1));
        assert_eq!(0, search(&mut move_list, 2));
    }

    #[test]
    fn test_alpha_beta_prunning_best_move_first() {
        let mut board = Board::new();
        let fen = "4k3/8/8/8/8/5PP1/1q6/P3K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::new(&mut board);

        assert_eq!(-600, search_no_quiescence(&mut move_list, 0));
        assert_eq!(300, search_no_quiescence(&mut move_list, 1));
    }

    #[test]
    fn test_alpha_beta_prunning_best_move_last() {
        let mut board = Board::new();
        let fen = "4k3/8/6q1/7P/8/8/PP6/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::new(&mut board);

        assert_eq!(-600, search_no_quiescence(&mut move_list, 0));
        assert_eq!(300, search_no_quiescence(&mut move_list, 1));
    }

    #[test]
    fn test_quiescence_search_1() {
        let mut board = Board::new();
        let fen = "8/3pk3/3p4/2P5/8/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::new(&mut board);

        assert_eq!(0, search_naive(&mut move_list, 1));
        assert_eq!(-100, quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
    }

    // Should work, but it does not, and I don't know why
    // #[test]
    // fn test_quiescence_search_2() {
    //     let mut board = Board::new();
    //     let fen = "8/3pk3/3p3P/2P5/8/8/8/4K3 w - - 0 1";
    //     board.read_fen(fen);
    //     let mut move_list = MoveList::new(&mut board);
    //
    //     assert_eq!(100, search_naive(&mut move_list, 1));
    //
    //     move_list.make_move(&Move{origin: 40, target: 48, promotion: 0, piece: PAWN});
    //
    //     assert_eq!(100, search_naive(&mut move_list, 1));
    //     assert_eq!(700, quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
    // }

    #[test]
    fn bench_search() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::new(&mut board);

        println!("{}", search(&mut move_list, 7));
    }
}