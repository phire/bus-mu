
pub mod instruction_cache;
pub mod register_file;
pub mod execute;
pub mod data_cache;
pub mod writeback;

use common::util::ByteMask8;

use crate::{
    DCache, cache::{CacheTag, DataCacheAttempt, ICache}, microtlb::ITlb, regfile::RegFile
};

use self::{instruction_cache::InstructionCache, register_file::RegisterFile, execute::{Execute, ExMode}, data_cache::{DataCache, MemMode}, writeback::WriteBack};


pub struct Pipeline {
    ic: InstructionCache,
    rf: RegisterFile,
    ex: Execute,
    dc: DataCache,
    wb: WriteBack,
    pub(crate) regs: RegFile,
}


pub enum MemoryReq
{
    ICacheFill(u32),
    DCacheFill(u32),
    DCacheReplace(u32, u32, [u64; 2]),
    UncachedInstructionRead(u32),
    UncachedDataReadWord(u32),
    UncachedDataReadDouble(u32),
    UncachedDataWriteWord(u32, u64, ByteMask8), // one transfer
    UncachedDataWriteDouble(u32, u64, ByteMask8), // two transfers
}

pub enum MemoryResponce
{
    ICacheFill([u64; 4]),
    DCacheFill([u64; 2]),
    UncachedInstructionRead(u64),
    UncachedDataRead(u64),
    UncachedDataWrite,
}

pub enum ExitReason
{
    Blocked, // All stages are stalled until a memory request is completed
    Stalled,
    //Stall(u8),
    Mem(MemoryReq),
}


impl Pipeline {

    pub fn pc(&self) -> u64 {
        self.rf.next_pc
    }

    /// The pipeline is blocked if (given the current state) there is no possible
    pub fn blocked(&self) -> bool {
        if self.wb.stalled {
            // Easy case, WB is stalled, so the entire pipeline is blocked
            return true;
        }
        let wb_has_work = self.dc.mem_mode.is_some() || self.dc.writeback_reg != 0;
        let dc_has_work = self.ex.mem_mode.is_some() || self.ex.writeback_reg != 0;
        let ex_has_work = match self.rf.ex_mode { ExMode::Nop => false, _ => true };

        // Otherwise we are blocked if ic is stalled and nothing else has work
        return self.ic.stalled && !(wb_has_work || dc_has_work || ex_has_work);
    }

    pub fn cycle(
        &mut self,
        icache: &mut ICache,
        dcache: &mut DCache,
        itlb: &mut ITlb,
    ) -> Result<(), ExitReason> {
        // We evaluate the pipeline in reverse order.
        // So each stage can use the previous stage's output before it's overwritten
        // This also allows us to stall the pipeline by returning early.

        // Stage 5: WriteBack
        self.wb.cycle(&self.dc, dcache, &mut self.regs)?;
        let mut writeback_has_work = false;

        // Stage 4: DataCache
        self.dc.cycle(&self.ex, dcache, &mut writeback_has_work)?;

        // Stage 3: Execute
        self.ex.cycle(&self.rf)?;

        // Stage 2: Register File read
        // Fixme: ic shouldn't need to be borrowed mutably
        self.rf.cycle(&mut self.ic, &self.ex, &mut self.regs, writeback_has_work)?;

        // Stage 1: Instruction Cache
        self.ic.cycle(icache, itlb, &self.rf);

        Ok(())
    }


    #[inline(always)]
    pub fn memory_responce(&mut self, info: MemoryResponce, transfers: usize, icache: &mut ICache,
        dcache: &mut DCache) {
        match info {
            MemoryResponce::ICacheFill(data) => {
                assert!(transfers == 8, "Bus state machine stalled");

                // Reconstruct line/offset from program counter
                let line = (self.pc() as usize >> 5) & 0x1ff;
                let new_tag = self.ic.expected_tag;

                icache.finish_fill(line, new_tag, data);

                self.ic.cache_data = icache.fetch(self.pc()).0;
                self.ic.cache_tag = new_tag;
                self.ic.stalled = false;
            }
            MemoryResponce::UncachedInstructionRead(data) => {
                let shift = 8 * (!self.pc() & 0x4);
                self.ic.cache_data = (data >> shift) as u32;
                self.ic.cache_tag = self.ic.expected_tag;
                self.ic.stalled = false;
                self.rf.ex_mode = ExMode::Nop;
            }
            MemoryResponce::DCacheFill(data) => {
                assert!(transfers == 4, "Bus state machine stalled");
                // TODO: critical word first timings
                self.dc.cache_attempt.finish_fill(dcache, self.dc.tlb_tag, data);
                self.wb.stalled = false;
            }
            MemoryResponce::UncachedDataRead(value) => {
                if self.dc.mem_size == 8 {
                    assert!(transfers == 2, "Bus state machine stalled");
                }
                let value = match self.dc.mem_mode {
                    Some(MemMode::LoadSignExtend(up, down)) => {
                        (value.wrapping_shl(up as u32) as i64 >> down) as u64
                    }
                    Some(MemMode::LoadZeroExtend(up, down)) => {
                        value.wrapping_shl(up as u32) >> down
                    }
                    Some(MemMode::LoadMergeWord(align)) => {
                        let mut temp = self.dc.alu_out;
                        self.dc.mem_mask.masked_insert(&mut temp, value >> align);

                        temp as i32 as u64 // sign extend
                    }
                    Some(MemMode::LoadMergeDouble(align)) => {
                        assert!(transfers == 2, "Bus state machine stalled");
                        let mut temp = self.dc.alu_out;
                        self.dc.mem_mask.masked_insert(&mut temp, value >> align);

                        temp
                    }
                    _ => unreachable!()
                };

                self.regs.write(self.dc.writeback_reg, value);
                self.dc.writeback_reg = 0;
                self.dc.mem_mode = None;
                self.wb.stalled = false;
            }
            MemoryResponce::UncachedDataWrite => {
                assert!(transfers == self.dc.mem_size as usize / 4 , "Bus state machine stalled");
                self.dc.mem_mode = None;
                self.wb.stalled = false;
            }
        }
    }

}

pub const RESET_PC : u64 = 0xffff_ffff_bfc0_0000;

pub fn create() -> Pipeline {
    Pipeline{
        ic: Default::default(),
        rf: Default::default(),
        ex: Default::default(),
        dc: DataCache{
            cache_attempt: DataCacheAttempt::empty(),
            tlb_tag: CacheTag::empty(),
            writeback_reg: 0,
            alu_out: 0,
            mem_mode: None,
            mem_size: 0,
            mem_mask: Default::default(),
        },
        wb: WriteBack{
            stalled: false,
        },
        regs: RegFile::new(),
    }
}
