use core::simd;
use std::simd::cmp::SimdPartialEq;

use crate::BusRequest;


#[derive(Clone, Copy)]
pub struct ICache {
    sets: [ICacheSet; 128],
    cache: Option<(u8, u8)>,
    lookup_address: u32,
}

#[derive(Clone, Copy)]
pub struct ICacheSet {
    valid: u8,
    plru: u8,
    tags: [u32; 8],
    instructions: [[u32; 8]; 8],
}

static PLRU_TABLE: [(u8, u8); 8] = [
    (0xFF, 0xD0),
    (0xEF, 0xC0),
    (0xBF, 0x88),
    (0xB7, 0x80),
    (0x7F, 0x24),
    (0x7B, 0x20),
    (0x5F, 0x02),
    (0x5D, 0x00),
];

impl ICacheSet {
    fn hit(&mut self, way: usize, address: u32) -> FetchResult {
        if (self.valid & (1 << way)) != 0 {
            // Cache hit
            let offset = (address & 0x1F) as usize >> 2;
            let end = (offset + 4).min(8); // Fetch up to 4 instructions, but not beyond the block
            let instructions = &self.instructions[way][offset..end];

            // Update PLRU state
            let (plru_and, plru_or) = PLRU_TABLE[way];
            self.plru = (self.plru & plru_and) | plru_or;

            FetchResult::Hit(instructions)
        } else {
            FetchResult::Miss
        }
    }
}

pub enum FetchResult<'a> {
    Hit(&'a [u32]),
    Miss,
}

impl ICache {
    pub fn start_fetch(&mut self, address: u32) {
        self.lookup_address = address;
    }

    pub fn fetch(&mut self) -> FetchResult {
        let expected_tag = self.lookup_address & 0xFFFF_FC00; // Upper 22 bits
        let index = (self.lookup_address >> 5) as usize & 0x7F; // 128 sets
        let set = &mut self.sets[index];

        for (way, &tag) in set.tags.iter().enumerate() {
            if tag == expected_tag {
                return set.hit(way, self.lookup_address);
            }
        }

        FetchResult::Miss
    }

    pub fn fetch_simd(&mut self) -> FetchResult {
        let expected_tag = self.lookup_address & 0xFFFF_FC00; // Upper 22 bits
        let index = (self.lookup_address >> 5) as usize & 0x7F; // 128 sets
        let set = &mut self.sets[index];

        let tags = simd::u32x8::from_array(set.tags);
        let expected_tag_vec = simd::u32x8::splat(expected_tag);

        if let Some(way) = tags.simd_eq(expected_tag_vec).first_set() {
            return set.hit(way, self.lookup_address);
        }

        FetchResult::Miss
    }


}