use crate::{
    cache::DCache,
    pipeline::{data_cache::MemMode, ExitReason},
    regfile::RegFile,
};

use super::data_cache::DataCache;

pub struct WriteBack {
    pub stalled: bool,
}

impl WriteBack {
    #[inline(always)]
    pub fn cycle(
        &mut self,
        dc: &DataCache,
        dcache: &mut DCache,
        regs: &mut RegFile,
    ) -> Result<(), ExitReason> {
        // We don't need to check this, because blocked() will prevent cycle from being called in this case
        debug_assert!(self.stalled == false);

        // TODO: CP0 bypass interlock
        let mut value = dc.alu_out;

        if dc.mem_mode.is_some() {
            assert!(dc.mem_size != 0);
        }

        // Finish DCache access from last stage
        if let Some(mem_mode) = dc.mem_mode {
            let cache_attempt = dc.cache_attempt;
            let tlb_tag = dc.tlb_tag;

            if cache_attempt.is_hit(tlb_tag) {
                match mem_mode {
                    MemMode::Store => cache_attempt.write(dcache, value, dc.mem_mask),
                    MemMode::LoadZeroExtend(up, down) => {
                        value = (cache_attempt.read(dcache) << up) >> down;
                    }
                    MemMode::LoadSignExtend(up, down) => {
                        value = ((cache_attempt.read(dcache) << up) as i64 >> down) as u64;
                    }
                    MemMode::LoadMergeWord(align) => {
                        let new_value = cache_attempt.read(dcache) >> align;
                        dc.mem_mask.masked_insert(&mut value, new_value);

                        value = value as i32 as u64; // sign extend
                    }
                    MemMode::LoadMergeDouble(align) => {
                        let new_value = cache_attempt.read(dcache) >> align;
                        dc.mem_mask.masked_insert(&mut value, new_value);
                    }
                    MemMode::ConditionalStore => {
                        cache_attempt.write(dcache, value, dc.mem_mask);
                        value = 1;
                    }
                    MemMode::ConditionalStoreFail => value = 0,
                }
            } else {
                let mem_size = dc.mem_size as usize;
                self.stalled = true;
                let do_miss = |is_store| {
                    cache_attempt.do_miss(
                        &dcache,
                        tlb_tag,
                        mem_size as u8,
                        is_store,
                        value,
                        dc.mem_mask,
                    )
                };
                match mem_mode {
                    MemMode::LoadSignExtend(_, _)
                    | MemMode::LoadZeroExtend(_, _)
                    | MemMode::LoadMergeWord(_)
                    | MemMode::LoadMergeDouble(_) => {
                        return Err(ExitReason::Mem(do_miss(false)));
                    }
                    MemMode::Store => {
                        return Err(ExitReason::Mem(do_miss(true)));
                    }
                    MemMode::ConditionalStore => {
                        // According to @Lemmy, LL/SC act just like LW/SW
                        if tlb_tag.is_uncached() {
                            // PERF: To avoid an extra branch in the uncached store path, do the
                            //       register write now.

                            regs.write(dc.writeback_reg, 1);
                        }
                        return Err(ExitReason::Mem(do_miss(true)));
                    }
                    MemMode::ConditionalStoreFail => {
                        // HWTEST: Which suggests that SC fails aborts the memory operation or suppresses exceptions
                        regs.write(dc.writeback_reg, 0);
                    }
                };
            }
        }

        // Register file writeback
        regs.write(dc.writeback_reg, value);

        Ok(())
    }
}
