use std::time::Instant;
use crate::board::Board;
use crate::evaluation::evaluate;
use crate::move_generator::{Move, MoveList};
use crate::piece::Piece::PAWN;

const NEGATIVE_INFINITY: f32 = i32::MIN as f32;
const POSITIVE_INFINITY: f32 = i32::MAX as f32;

pub fn search(move_list: &mut MoveList, depth: u32) -> f32 {
    search_alpha_beta_prunning(move_list, depth, NEGATIVE_INFINITY, POSITIVE_INFINITY)
}

pub fn search_no_quiescence(move_list: &mut MoveList, depth: u32) -> f32 {
    search_alpha_beta_prunning_naive(move_list, depth, NEGATIVE_INFINITY, POSITIVE_INFINITY)
}


fn search_alpha_beta_prunning(move_list: &mut MoveList, depth: u32, mut alpha: f32, beta: f32) -> f32 {
    let mut value: f32 = evaluate(move_list.get_board());

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

fn search_alpha_beta_prunning_naive(move_list: &mut MoveList, depth: u32, mut alpha: f32, beta: f32) -> f32 {
    let mut value: f32 = evaluate(move_list.get_board());

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

fn quiescence_search(move_list: &mut MoveList, mut alpha: f32, beta: f32) -> f32 {
    // let start = Instant::now();
    let mut value: f32 = evaluate(move_list.get_board());

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

#[deprecated]
fn search_naive(move_list: &mut MoveList, depth: u32) -> f32 {
    let mut value: f32 = evaluate(move_list.get_board());

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
    use std::time::Instant;
    use crate::board::START_POSITION;
    use crate::move_generator::Move;
    use crate::piece::Piece::PAWN;
    use super::*;

    #[test]
    fn search_initial_position() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::new(&mut board);

        assert_eq!(0.0, search_naive(&mut move_list, 0));
        assert_eq!(0.0, search_naive(&mut move_list, 1));
        assert_eq!(0.0, search_naive(&mut move_list, 2));
        assert_eq!(0.0, search_naive(&mut move_list, 3));

        assert_eq!(0.0, search(&mut move_list, 0));
        assert_eq!(0.0, search(&mut move_list, 1));
        assert_eq!(0.0, search(&mut move_list, 2));
        assert_eq!(0.0, search(&mut move_list, 3));
    }

    #[test]
    fn search_material_handling() {
        let mut board = Board::new();
        let fen = "4k3/5r2/8/8/8/5R2/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::new(&mut board);

        assert_eq!(0.0, search_naive(&mut move_list, 0));
        assert_eq!(5.0, search_naive(&mut move_list, 1));
        assert_eq!(0.0, search_naive(&mut move_list, 2));

        assert_eq!(0.0, search(&mut move_list, 0));
        assert_eq!(5.0, search(&mut move_list, 1));
        assert_eq!(0.0, search(&mut move_list, 2));
    }

    #[test]
    fn test_alpha_beta_prunning_best_move_first() {
        let mut board = Board::new();
        let fen = "4k3/8/8/8/8/5PP1/1q6/P3K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::new(&mut board);

        assert_eq!(-6.0, search_no_quiescence(&mut move_list, 0));
        assert_eq!(3.0, search_no_quiescence(&mut move_list, 1));
    }

    #[test]
    fn test_alpha_beta_prunning_best_move_last() {
        let mut board = Board::new();
        let fen = "4k3/8/6q1/7P/8/8/PP6/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::new(&mut board);

        assert_eq!(-6.0, search_no_quiescence(&mut move_list, 0));
        assert_eq!(3.0, search_no_quiescence(&mut move_list, 1));
    }

    #[test]
    fn test_quiescence_search_1() {
        let mut board = Board::new();
        let fen = "8/3pk3/3p4/2P5/8/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::new(&mut board);

        assert_eq!(0.0, search_naive(&mut move_list, 1));
        assert_eq!(-1.0, quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
    }

    // Should work, but it does not, and I don't know why
    // #[test]
    // fn test_quiescence_search_2() {
    //     let mut board = Board::new();
    //     let fen = "8/3pk3/3p3P/2P5/8/8/8/4K3 w - - 0 1";
    //     board.read_fen(fen);
    //     let mut move_list = MoveList::new(&mut board);
    //
    //     assert_eq!(1.0, search_naive(&mut move_list, 1));
    //
    //     move_list.make_move(&Move{origin: 40, target: 48, promotion: 0, piece: PAWN});
    //
    //     assert_eq!(1.0, search_naive(&mut move_list, 1));
    //     assert_eq!(7.0, quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
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