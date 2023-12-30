use crate::{cache::{CacheTag, ICache}, microtlb::ITlb};

use super::register_file::RegisterFile;




pub struct InstructionCache {
    pub cache_data: u32,
    pub cache_tag: CacheTag,
    pub expected_tag: CacheTag,
    pub stalled: bool,
}

impl Default for InstructionCache {
    fn default() -> Self {
        Self {
            cache_data: 0,
            cache_tag: CacheTag::empty(),
            // Start with the first instruction fetch already started.
            // Otherwise the RF stage will incorrectly start a ITLB miss on the first cycle
            expected_tag: CacheTag::new_uncached(super::RESET_PC as u32 & 0x1fff_ffff),
            stalled: false,
        }
    }
}

impl InstructionCache {
    #[inline(always)]
    pub fn cycle(&mut self, icache: &mut ICache, itlb: &mut ITlb, rf: &RegisterFile) {
        (self.cache_data, self.cache_tag) = icache.fetch(rf.next_pc);
        self.expected_tag = itlb.translate(rf.next_pc);
    }
}