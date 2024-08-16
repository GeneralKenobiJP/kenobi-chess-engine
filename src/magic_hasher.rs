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
        let mut input = 0u64;
        for &byte in bytes {
            input <<= 8;
            input |= byte as u64;
        }
        self.write_u64(input);
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

    #[test]
    fn hashing() {
        let s = BuildMagicHasher::new();
        let mut magic_hasher = s.build_hasher();
        magic_hasher.write(&[0x00,0x80,0x80,0x80,0x80,0x80,0x80,0x7E]);
        assert_eq!(0x008080808080807Eu64.wrapping_mul(0x000101010101017E), magic_hasher.finish());
        magic_hasher.write_u64(0x008080808080807E);
        assert_eq!(0x008080808080807Eu64.wrapping_mul(0x000101010101017E), magic_hasher.finish());
    }
}