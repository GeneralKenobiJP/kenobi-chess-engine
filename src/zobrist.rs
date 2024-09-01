
const PIECES_POSITIONS: usize = 64*12;

pub struct ZobristTable {
    pub pieces: [[u64; 64]; 12],
    pub en_passant: [u64; 8], // we only need to know the file of en passant
    pub castling_rights: [u64; 16], // there are 2^4=16 possible combinations of castling rights
    pub active_player: u64 // we only need to indicate if black is the active player
}

pub const ZOBRIST_TABLE: ZobristTable = {
    let mut seed = 0xFFAA_B58C_5833_FE89u64;
    let mut table = [0u64; 792];

    for i in 0..PIECES_POSITIONS {
        table[i] = xorshift(seed);
    }

    for i in PIECES_POSITIONS..PIECES_POSITIONS + 8 {
        table[i] = xorshift(seed);
    }

    for i in PIECES_POSITIONS + 8..PIECES_POSITIONS + 8 + 16 {
        table[i] = xorshift(seed);
    }

    table[PIECES_POSITIONS + 8 + 16];

    unsafe { std::mem::transmute(table) }
};

fn xorshift(mut seed: u64) -> u64 {
    seed ^= seed << 13;
    seed ^= seed >> 7;
    seed ^= seed << 17;

    seed
}

