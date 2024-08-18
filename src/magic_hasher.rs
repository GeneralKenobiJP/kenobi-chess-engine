use std::hash::{Hash, Hasher};

const MAGIC_NUMBERS_ROOK: [u64; 64] = [
    0x008080808080807E, 0x004040404040403E, 0x002020202020205E, 0x001010101010106E, 0x0008080808080876, 0x000404040404047A, 0x000202020202027C, 0x000101010101017E,
    0x0080808080807E00, 0x0040404040403E00, 0x0020202020205E00, 0x0010101010106E00, 0x0008080808087600, 0x0004040404047A00, 0x0002020202027C00, 0x0001010101017E00,
    0x00808080807E8000, 0x00404040403E4000, 0x00202020205E2000, 0x00101010106E1000, 0x0008080808760800, 0x00040404047A0400, 0x00020202027C0200, 0x00010101017E0100,
    0x008080807E808000, 0x004040403E404000, 0x002020205E202000, 0x001010106E101000, 0x0008080876080800, 0x000404047A040400, 0x000202027C020200, 0x000101017E010100,
    0x0080807E80808000, 0x0040403E40404000, 0x0020205E20202000, 0x0010106E10101000, 0x0008087608080800, 0x0004047A04040400, 0x0002027C02020200, 0x0001017E01010100,
    0x00807E8080808000, 0x00403E4040404000, 0x00205E2020202000, 0x00106E1010101000, 0x0008760808080800, 0x00047A0404040400, 0x00027C0202020200, 0x00017E0101010100,
    0x007E808080808000, 0x003E404040404000, 0x005E202020202000, 0x006E101010101000, 0x0076080808080800, 0x007A040404040400, 0x007C020202020200, 0x007E010101010100,
    0x7E80808080808000, 0x3E40404040404000, 0x5E20202020202000, 0x6E10101010101000, 0x7608080808080800, 0x7A04040404040400, 0x7C02020202020200, 0x7E01010101010100,
];
const MAGIC_NUMBERS_SHIFT_ROOK: [u8; 64] = [
    52, 53, 53, 53, 53, 53, 53, 52,
    53, 54, 54, 54, 54, 54, 54, 53,
    53, 54, 54, 54, 54, 54, 54, 53,
    53, 54, 54, 54, 54, 54, 54, 53,
    53, 54, 54, 54, 54, 54, 54, 53,
    53, 54, 54, 54, 54, 54, 54, 53,
    53, 54, 54, 54, 54, 54, 54, 53,
    53, 53, 53, 53, 53, 54, 54, 53
];

pub fn magic_hash_rook(key: u64, origin: u8) -> usize {
    ((key.wrapping_mul(MAGIC_NUMBERS_ROOK[origin as usize]))
        >> MAGIC_NUMBERS_SHIFT_ROOK[origin as usize]) as usize
}

// #[derive(Eq, PartialEq, Clone)]
// pub struct MagicKey {
//     raw_key: u64,
//     origin: u8
// }
//
// impl Hash for MagicKey {
//     fn hash<H: Hasher>(&self, state: &mut H) {
//         let mut input: [u8; 9] = [0;9];
//         input[..8].copy_from_slice(&self.raw_key.to_le_bytes());
//         input[8] = self.origin;
//         input.hash(state);
//     }
// }
//
// pub struct MagicHasher {
//     state: u64,
// }
//
// pub struct BuildMagicHasher;
//
// impl BuildMagicHasher {
//     fn new() -> Self {
//         BuildMagicHasher
//     }
// }
//
// impl std::hash::Hasher for MagicHasher {
//     fn finish(&self) -> u64 {
//         self.state
//     }
//
//     fn write(&mut self, bytes: &[u8]) {
//         let mut input = 0u64;
//         for i in 0..8 {
//             input <<= 8;
//             input |= bytes[i] as u64;
//         }
//         self.state = input.wrapping_mul(MAGIC_NUMBERS_ROOK[bytes[8] as usize]);
//         self.state >>= MAGIC_NUMBERS_SHIFT_ROOK[bytes[8] as usize];
//         // self.write_u64(input);
//     }
//
//     fn write_u64(&mut self, i: u64) {
//         self.state = i.wrapping_mul(0x000101010101017E);
//     }
// }
//
// impl std::hash::BuildHasher for BuildMagicHasher {
//     type Hasher = MagicHasher;
//     fn build_hasher(&self) -> MagicHasher {
//         MagicHasher { state: 0 }
//     }
// }

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::hash::{BuildHasher, Hasher, RandomState};
    use crate::board::{Board, START_POSITION};
    use super::*;

    #[test]
    fn hashing() {
        assert_eq!(0x008080808080807Eu64.wrapping_mul(0x000101010101017E) >> 52, magic_hash_rook(0x008080808080807Eu64, 7) as u64);
        assert_eq!(0x000101010101017Eu64.wrapping_mul(0x008080808080807E) >> 52, magic_hash_rook(0x000101010101017Eu64, 0) as u64);
        // let s = BuildMagicHasher::new();
        // let mut magic_hasher = s.build_hasher();
        // magic_hasher.write(&[0x00,0x80,0x80,0x80,0x80,0x80,0x80,0x7E,7]);
        // assert_eq!(0x008080808080807Eu64.wrapping_mul(0x000101010101017E) >> 52, magic_hasher.finish());
        // magic_hasher.write_u64(0x008080808080807E);
        // assert_eq!(0x008080808080807Eu64.wrapping_mul(0x000101010101017E), magic_hasher.finish());

        // let x = MAGIC_NUMBERS_ROOK;
        // let mut y = [0u64;64];
        // let mut index = 0;
        // for i in 0..8 {
        //     for j in (0..8).rev() { y[index] = x[j+8*i]; index+=1; }
        // }
        // println!{"{:#018X?}", y};

        // let x = MAGIC_NUMBERS_SHIFT_ROOK;
        // let mut y = [0u8;64];
        // let mut index = 0;
        // for i in 0..8 {
        //     for j in (0..8).rev() { y[index] = x[j+8*i]; index+=1; }
        // }
        // println!{"{:?}", y};
    }

    #[test]
    fn hash_map() {
        // let mut hash_map = HashMap::<MagicKey,u64,BuildMagicHasher>::with_hasher(BuildMagicHasher);
        // let key = 0x008080808080807Eu64;
        // let magic_key = MagicKey { raw_key: key, origin: 0 };
        // hash_map.insert(magic_key.clone(), 69);
        // assert_eq!(hash_map.len(), 1);
        // assert_eq!(*hash_map.get(&magic_key).unwrap(), 69);
    }
}