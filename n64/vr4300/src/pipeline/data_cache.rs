use common::util::ByteMask8;

use crate::cache::{DataCacheAttempt, CacheTag, DCache};

use super::{ExitReason, execute::Execute};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemMode
{
    //Load,
    LoadSignExtend(u8, u8),
    LoadZeroExtend(u8, u8),
    LoadMergeWord(u8),
    LoadMergeDouble(u8),
    Store,
    ConditionalStore,
    ConditionalStoreFail,
}

pub struct DataCache {
    pub cache_attempt: DataCacheAttempt,
    pub tlb_tag: CacheTag,
    pub writeback_reg: u8,
    pub alu_out: u64,
    pub mem_mode: Option<MemMode>,
    pub mem_size: u8,
    pub mem_mask: ByteMask8,
}

impl DataCache {
    #[inline(always)]
    pub fn cycle(&mut self, ex: &Execute, dcache: &mut DCache, writeback_has_work: &mut bool) -> Result<(), ExitReason> {
        // Clear previous op
        self.mem_mode = None;
        self.writeback_reg = 0;

        if let Some(_) = ex.mem_mode {
            let addr = ex.addr;

            self.cache_attempt = dcache.open(addr);
            // TODO: Implement TLB lookups
            self.tlb_tag = CacheTag::new_uncached((addr as u32) & 0x1fff_ffff);
        }

        if ex.mem_mode.is_some() || ex.writeback_reg != 0 {
            // Forward from EX
            self.alu_out = ex.alu_out;
            self.writeback_reg = ex.writeback_reg;
            self.mem_mode = ex.mem_mode;
            self.mem_size = ex.mem_size;
            self.mem_mask = ex.mem_mask;
            *writeback_has_work = true;
        }

        Ok(())
    }
}
