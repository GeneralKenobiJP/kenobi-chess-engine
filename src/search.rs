use crate::board::Board;
use crate::evaluation::evaluate;
use crate::move_generator::MoveList;
use crate::perft::perft;

pub fn search(move_list: &mut MoveList, depth: u32) -> f32 {
    let mut value: f32 = 0.0;

    if depth == 0 {
        return evaluate(move_list.get_board());
    }

    move_list.generate_moves();

    let moves = move_list.get_moves().clone();

    for piece_move in moves
    {
        move_list.make_move(&piece_move);
        if !move_list.is_opponent_in_check() {
            value = value.max(-search(move_list, depth - 1))
        }
        move_list.unmake_move(&piece_move);
    }

    value
}

#[cfg(test)]
mod tests {
    use std::time::Instant;
    use crate::board::START_POSITION;
    use super::*;

    #[test]
    fn search_initial_position() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::new(&mut board);

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

        assert_eq!(0.0, search(&mut move_list, 0));
        assert_eq!(5.0, search(&mut move_list, 1));
        assert_eq!(0.0, search(&mut move_list, 2));
    }
}