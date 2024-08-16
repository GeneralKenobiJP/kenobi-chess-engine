pub struct MagicHasher {
    state: u64,
}

pub struct BuildMagicHasher;

impl BuildMagicHasher {
    pub(crate) fn new() -> Self {
        BuildMagicHasher
    }
}

impl std::hash::Hasher for MagicHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state = self.state.rotate_left(8).wrapping_add(u64::from(byte).wrapping_mul(0x000101010101017E));
            // self.state = self.state.checked_shl(8).unwrap_or_default() + u64::from(byte).wrapping_mul(0x000101010101017E);
        }
            // for chunk in bytes.chunks(8) {
            //     let mut buf = [0u8; 8];
            //     buf[..chunk.len()].copy_from_slice(chunk);
            //     let value = u64::from_le_bytes(buf);
            //     self.write_u64(value);
            // }
        // let mut num = 0u64;
        // for &byte in bytes {
        //     num <<= 8;
        //     num |= byte as u64;
        // }
        // println!("{}", num);
        // self.write_u64(num);
    }

    fn write_u64(&mut self, i: u64) {
        self.state = i.wrapping_mul(0x000101010101017E);
    }
}

impl std::hash::BuildHasher for BuildMagicHasher {
    type Hasher = MagicHasher;
    fn build_hasher(&self) -> MagicHasher {
        MagicHasher { state: 0 }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::hash::{BuildHasher, Hasher, RandomState};
    use crate::board::{Board, START_POSITION};
    use super::*;

    // #[test]
    // fn test() {
    //     let s = BuildMagicHasher::new();
    //     let mut magic_hasher = s.build_hasher();
    //     magic_hasher.write(&[0u8, 0u8,1,32]);
    //     assert_eq!(288, magic_hasher.finish());
    // }

    // #[test]
    // fn test2() {
    //     let s = BuildMagicHasher::new();
    //     let mut magic_hasher = s.build_hasher();
    //     magic_hasher.write(&[0,0,2,4,8,16,32,0]);
    //     println!("{}", magic_hasher.finish() >> (64-5));
    // }

    #[test]
    fn test3() {
        let s = BuildMagicHasher::new();
        let mut magic_hasher = s.build_hasher();
        magic_hasher.write(&[0x00,0x80,0x80,0x80,0x80,0x80,0x80,0x7E]);
        println!("{}", magic_hasher.finish());
    }

    #[test]
    fn test4() {
        let s = BuildMagicHasher::new();
        let mut magic_hasher = s.build_hasher();
        magic_hasher.write_u64(0x008080808080807E);
        println!("{}", magic_hasher.finish());
    }
}