use std::ffi::{c_char, CString};
use std::path::Path;
use crate::board::Board;
use crate::move_generator::NO_PASSANT;
use crate::piece::Colour::{BLACK, WHITE};
use crate::piece::Piece::{BISHOP, KING, KNIGHT, PAWN, QUEEN, ROOK};

const TB_LOSS: u32 = 0;
const TB_BLESSED_LOSS: u32 = 1;
const TB_DRAW: u32 = 2;
const TB_CURSED_WIN: u32 = 3;
const TB_WIN: u32 = 4;

const TB_RESULT_FAILED: u32 = 0xFFFF_FFFF;

unsafe extern "C" {
    fn fathom_init(path: *const c_char) -> i32;
    fn fathom_free();
    fn fathom_largest() -> u32;

    fn fathom_probe_wdl(
        white: u64,
        black: u64,
        kings: u64,
        queens: u64,
        rooks: u64,
        bishops: u64,
        knights: u64,
        pawns: u64,
        rule50: u32,
        castling: u32,
        ep: u32,
        white_to_move: u8,
    ) -> u32;
}

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct TablebaseStats {
    pub(crate) probes: u64,
    pub(crate) hits: u64,
    pub(crate) wins: u64,
    pub(crate) draws: u64,
    pub(crate) losses: u64,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Wdl {
    Loss,
    BlessedLoss,
    Draw,
    CursedWin,
    Win,
}

impl Wdl {
    fn from_raw(value: u32) -> Option<Self> {
        match value {
            TB_LOSS => Some(Self::Loss),
            TB_BLESSED_LOSS => Some(Self::BlessedLoss),
            TB_DRAW => Some(Self::Draw),
            TB_CURSED_WIN => Some(Self::CursedWin),
            TB_WIN => Some(Self::Win),
            TB_RESULT_FAILED => None,
            _ => None,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct TbPosition {
    /// All white pieces.
    pub white: u64,

    /// All black pieces.
    pub black: u64,

    /// Kings of both colors.
    pub kings: u64,

    /// Queens of both colors.
    pub queens: u64,

    /// Rooks of both colors.
    pub rooks: u64,

    /// Bishops of both colors.
    pub bishops: u64,

    /// Knights of both colors.
    pub knights: u64,

    /// Pawns of both colors.
    pub pawns: u64,

    /// FEN halfmove clock.
    pub rule50: u32,

    /// Fathom castling-right flags.
    pub castling: u32,

    /// En-passant target square, or zero for none.
    pub ep: u32,

    pub white_to_move: bool,
}

impl TbPosition {
    pub fn from_board(board: &Board) -> Self {
        let en_passant = if board.en_passant_possibility != NO_PASSANT {
            board.en_passant_possibility
        } else {
            0
        };
        TbPosition {
            white: board.colour_bitboards[WHITE as usize],
            black: board.colour_bitboards[BLACK as usize],
            kings: board.piece_bitboards[KING as usize] | board.piece_bitboards[6 + KING as usize],
            queens: board.piece_bitboards[QUEEN as usize] | board.piece_bitboards[6 + QUEEN as usize],
            rooks: board.piece_bitboards[ROOK as usize] | board.piece_bitboards[6 + ROOK as usize],
            bishops: board.piece_bitboards[BISHOP as usize] | board.piece_bitboards[6 + BISHOP as usize],
            knights: board.piece_bitboards[KNIGHT as usize] | board.piece_bitboards[6 + KNIGHT as usize],
            pawns: board.piece_bitboards[PAWN as usize] | board.piece_bitboards[6 + PAWN as usize],
            rule50: board.half_moves as u32,
            castling: board.castling_rights as u32,
            ep: en_passant as u32,
            white_to_move: board.active_player == WHITE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syzygy {
    max_pieces: u32,
}

impl Syzygy {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();

        let path = path
            .to_str()
            .ok_or_else(|| {
                "Syzygy path is not valid UTF-8".to_string()
            })?;

        let path = CString::new(path)
            .map_err(|_| {
                "Syzygy path contains an interior NUL byte".to_string()
            })?;

        let initialized = unsafe {
            fathom_init(path.as_ptr())
        };

        if initialized == 0 {
            return Err(
                "Fathom failed to initialize".to_string()
            );
        }

        let max_pieces = unsafe {
            fathom_largest()
        };

        /*
         * Important:
         *
         * Fathom's tb_init() may report success even when
         * it finds no usable tablebase files.
         */
        if max_pieces == 0 {
            unsafe {
                fathom_free();
            }

            return Err(
                "Fathom initialized, but no .rtbw tablebases were found"
                    .to_string(),
            );
        }

        Ok(Self { max_pieces })
    }

    pub fn max_pieces(&self) -> u32 {
        self.max_pieces
    }

    pub fn probe_wdl(&self, position: &TbPosition) -> Option<Wdl> {
        let occupied = position.white | position.black;
        let piece_count = occupied.count_ones();

        if piece_count > self.max_pieces {
            return None;
        }

        /*
         * Fathom's lightweight WDL search API explicitly refuses
         * castling-right positions and non-zero rule50 positions.
         */
        if position.castling != 0 {
            return None;
        }

        if position.rule50 != 0 {
            return None;
        }

        let result = unsafe {
            fathom_probe_wdl(
                position.white,
                position.black,
                position.kings,
                position.queens,
                position.rooks,
                position.bishops,
                position.knights,
                position.pawns,
                position.rule50,
                position.castling,
                position.ep,
                u8::from(position.white_to_move),
            )
        };

        Wdl::from_raw(result)
    }
}

impl Drop for Syzygy {
    fn drop(&mut self) {
        unsafe {
            fathom_free();
        }
    }
}

#[cfg(test)]
pub(crate) static FATHOM_TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_wdl_from_raw() {
        assert_eq!(Some(Wdl::Loss), Wdl::from_raw(TB_LOSS));
        assert_eq!(Some(Wdl::BlessedLoss), Wdl::from_raw(TB_BLESSED_LOSS));
        assert_eq!(Some(Wdl::Draw), Wdl::from_raw(TB_DRAW));
        assert_eq!(Some(Wdl::CursedWin), Wdl::from_raw(TB_CURSED_WIN));
        assert_eq!(Some(Wdl::Win), Wdl::from_raw(TB_WIN));
        assert_eq!(None, Wdl::from_raw(TB_RESULT_FAILED));
        assert_eq!(None, Wdl::from_raw(2137));
    }

    #[test]
    fn check_tb_position_from_board() {
        let board = Board::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 17 1");
        let position = TbPosition::from_board(&board);

        let white_queen = 1u64 << 4;
        let white_king = 1u64 << 3;
        let black_king = 1u64 << 59;

        assert_eq!(white_queen | white_king, position.white);
        assert_eq!(black_king, position.black);
        assert_eq!(white_king | black_king, position.kings);
        assert_eq!(white_queen, position.queens);
        assert_eq!(0, position.rooks);
        assert_eq!(0, position.bishops);
        assert_eq!(0, position.knights);
        assert_eq!(0, position.pawns);
        assert_eq!(17, position.rule50);
        assert_eq!(0, position.castling);
        assert_eq!(0, position.ep);
        assert!(position.white_to_move);
    }

    #[test]
    fn check_syzygy_open_rejects_interior_nul() {
        assert_eq!(
            Err("Syzygy path contains an interior NUL byte".to_string()),
            Syzygy::open("invalid\0path")
        );
    }

    #[test]
    #[ignore = "requires SYZYGY_PATH pointing at a directory containing KQvK.rtbw"]
    fn check_syzygy_probe_wdl_kqk() {
        let _guard = FATHOM_TEST_MUTEX.lock().unwrap();
        let path = std::env::var("SYZYGY_PATH")
            .expect("SYZYGY_PATH must point at the Syzygy tablebase directory");

        let syzygy = Syzygy::open(path).expect("failed to initialize Syzygy tablebases");
        assert!(syzygy.max_pieces() >= 3);

        let white_to_move =
            TbPosition::from_board(&Board::from_fen("8/8/8/8/8/8/4Q3/4K2k w - - 0 1"));
        let black_to_move =
            TbPosition::from_board(&Board::from_fen("8/8/8/8/8/8/4Q3/4K2k b - - 0 1"));
        let nonzero_rule50 =
            TbPosition::from_board(&Board::from_fen("8/8/8/8/8/8/4Q3/4K2k w - - 1 1"));

        assert_eq!(Some(Wdl::Win), syzygy.probe_wdl(&white_to_move));
        assert_eq!(Some(Wdl::Loss), syzygy.probe_wdl(&black_to_move));
        assert_eq!(None, syzygy.probe_wdl(&nonzero_rule50));
    }
}
