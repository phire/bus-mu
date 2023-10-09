
use common::util::ByteMask8;

use super::{
    instructions::{
        InstructionInfo,
        IType,
        RfMode,
        JType,
        RType,
        ExMode,
        CmpMode
    }, DCache, cache::{CacheTag, DataCacheAttempt, ICache}, microtlb::ITlb, regfile::RegFile
};

struct InstructionCache {
    cache_data: u32,
    cache_tag: CacheTag,
    expected_tag: CacheTag,
    stalled: bool,
}

struct RegisterFile {
    next_pc: u64,
    alu_a: u64,
    alu_b: u64,
    temp: u64, // Either result of jump calculation, or value to store
    writeback_reg: u8,
    ex_mode: ExMode,
}

struct Execute {
    next_pc: u64,
    alu_out: u64,
    addr: u64,
    skip_next: bool, // Used to skip the op about to be executed in RF stage
    mem_size: u8,
    mem_mode: Option<MemMode>,
    mem_mask: ByteMask8,
    trap: bool,
    writeback_reg: u8,

    // internal storage
    hilo: [u64; 2],
    ll_bit: bool,
    ll_addr: u64,

    subinstruction_cycle: u32,
}
struct DataCache {
    cache_attempt: DataCacheAttempt,
    tlb_tag: CacheTag,
    writeback_reg: u8,
    alu_out: u64,
    mem_mode: Option<MemMode>,
    mem_size: u8,
    mem_mask: ByteMask8,
}
struct WriteBack {
    stalled: bool,
}

pub struct Pipeline {
    ic: InstructionCache,
    rf: RegisterFile,
    ex: Execute,
    dc: DataCache,
    wb: WriteBack,
    pub(crate) regs: RegFile,
}

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
    Ok,
    Blocked, // All stages are stalled until a memory request is completed
    //Stall(u8),
    Mem(MemoryReq),
}


impl Pipeline {
    fn compare(cmp: CmpMode, a: i64, b: i64) -> bool {
        match cmp {
            CmpMode::Eq => a == b,
            CmpMode::Ne => a != b,
            CmpMode::Lt => a < b,
            CmpMode::Gt => a > b,
            CmpMode::Le => a <= b,
            CmpMode::Ge => a >= b,
        }
    }

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
    ) -> ExitReason {
        // We evaluate the pipeline in reverse order.
        // So each stage can use the previous stage's output before it's overwritten
        // This also allows us to stall the pipeline by returning early.

        // ==================
        // Stage 5: WriteBack
        // ==================
        {
            // We don't need to check this, because blocked() will prevent cycle from being called in this case
            debug_assert!(self.wb.stalled == false);

            // TODO: CP0 bypass interlock
            let mut value = self.dc.alu_out;

            if self.dc.mem_mode.is_some() {
                assert!(self.dc.mem_size != 0);
            }

            // Finish DCache access from last stage
            if let Some(mem_mode) = self.dc.mem_mode {
                let cache_attempt = self.dc.cache_attempt;
                let tlb_tag = self.dc.tlb_tag;

                if cache_attempt.is_hit(tlb_tag) {
                    match mem_mode {
                        MemMode::Store => cache_attempt.write(dcache, value, self.dc.mem_mask),
                        MemMode::LoadZeroExtend(up, down) => {
                            value = (cache_attempt.read(dcache) << up) >> down;
                        }
                        MemMode::LoadSignExtend(up, down) => {
                            value = ((cache_attempt.read(dcache) << up) as i64 >> down) as u64;
                        }
                        MemMode::LoadMergeWord(align) => {
                            let new_value = cache_attempt.read(dcache) >> align;
                            self.dc.mem_mask.masked_insert(&mut value, new_value);

                            value = value as i32 as u64; // sign extend
                        }
                        MemMode::LoadMergeDouble(align) => {
                            let new_value = cache_attempt.read(dcache) >> align;
                            self.dc.mem_mask.masked_insert(&mut value, new_value);
                        }
                        MemMode::ConditionalStore => {
                            cache_attempt.write(dcache, value, self.dc.mem_mask);
                            value = 1;
                        }
                        MemMode::ConditionalStoreFail => value = 0,
                    }
                } else {
                    let mem_size = self.dc.mem_size as usize;
                    self.wb.stalled = true;
                    let do_miss = |is_store| {
                        cache_attempt.do_miss(&dcache, tlb_tag, mem_size as u8, is_store, value, self.dc.mem_mask)
                    };
                    match mem_mode {
                        MemMode::LoadSignExtend(_, _) | MemMode::LoadZeroExtend(_, _) |
                        MemMode::LoadMergeWord(_) |  MemMode::LoadMergeDouble(_) => {
                            return ExitReason::Mem(do_miss(false));
                        }
                        MemMode::Store => {
                            return ExitReason::Mem(do_miss(true));
                        }
                        MemMode::ConditionalStore => {
                            // According to @Lemmy, LL/SC act just like LW/SW
                            if tlb_tag.is_uncached() {
                                // PERF: To avoid an extra branch in the uncached store path, do the
                                //       register write now.

                                self.regs.write(self.dc.writeback_reg, 1);
                            }
                            return ExitReason::Mem(do_miss(true));
                        } MemMode::ConditionalStoreFail => {
                            // HWTEST: Which suggests that SC fails aborts the memory operation or suppresses exceptions
                            self.regs.write(self.dc.writeback_reg, 0);
                        }
                    };
                }
            }

            // Register file writeback
            self.regs.write(self.dc.writeback_reg, value);
        }

        let mut writeback_has_work = false;

        // ==================
        // Stage 4: DataCache
        // ==================
        {
            // Clear previous op
            self.dc.mem_mode = None;
            self.dc.writeback_reg = 0;

            if let Some(_) = self.ex.mem_mode {
                let addr = self.ex.addr;

                self.dc.cache_attempt = dcache.open(addr);
                // TODO: Implement TLB lookups
                self.dc.tlb_tag = CacheTag::new_uncached((addr as u32) & 0x1fff_ffff);
            }

            if self.ex.mem_mode.is_some() || self.ex.writeback_reg != 0 {
                // Forward from EX
                self.dc.alu_out = self.ex.alu_out;
                self.dc.writeback_reg = self.ex.writeback_reg;
                self.dc.mem_mode = self.ex.mem_mode;
                self.dc.mem_size = self.ex.mem_size;
                self.dc.mem_mask = self.ex.mem_mask;
                writeback_has_work = true;
            }
        }

        // ================
        // Stage 3: Execute
        // ================
        {
            self.ex.mem_mode = None;
            self.ex.writeback_reg = 0;

            // PERF: we might be able to move this skip logic into the jump table
            if self.ex.skip_next {
                match self.rf.ex_mode {
                    ExMode::Nop => {
                        // FIXME: This is going to break when there is a nop instruction in the branch delay slot
                    }
                    _ => {
                        // For some reason... branch likely instructions invalidate the branch-delay
                        // slot's instruction if they aren't taken... Which is backwards
                        //println!("Skipping instruction {:?}", self.rf.ex_mode);
                        self.ex.skip_next = false;
                        self.ex.next_pc = self.rf.next_pc;
                    }
                }
            } else {
                Self::run_ex_phase1(&self.rf, &mut self.ex);

                if self.ex.subinstruction_cycle != 0 {
                    // The pipeline is stalled, executing a multi-cycle instruction
                    return ExitReason::Ok;
                }
            }
        }

        // ======================
        // Stage 2: Register File
        // ======================
        {
            // First we check the result of the Instruction Cache stage
            // ICache always returns an instruction, but it might be the wrong one
            // The only way to know is to check the output of ITLB matches the tag ICache returned

            // PERF: How to we tell the compiler this first case is the most likely?
            if self.ic.cache_tag == self.ic.expected_tag {
                debug_assert!(self.ic.stalled == false);
                debug_assert!(self.ic.expected_tag.is_valid());

                self.regs.bypass(
                    self.ex.writeback_reg,
                    match self.ex.mem_mode {
                        Some(_) => Some(self.ex.alu_out),
                        None => None
                    });

                // ICache hit. We can continue with the rest of this stage
                Self::run_regfile(self.ic.cache_data, &mut self.rf, &mut self.regs);

                if self.regs.hazard_detected() {
                    // regfile detected a hazard (register value is dependent on memory load)
                    // The output of this stage is invalid, but we will retry next cycle
                    self.rf.ex_mode = ExMode::Nop;
                    return ExitReason::Ok;
                }

                self.rf.next_pc = self.ex.next_pc + 4;

            } else if !self.ic.stalled && self.ic.cache_tag != self.ic.expected_tag {
                // PERF: Can we tell the compiler this block is more likely than the next?

                debug_assert!(self.ic.expected_tag.is_valid());
                self.ic.stalled = true;
                self.rf.ex_mode = ExMode::Nop;

                let req = if self.ic.expected_tag.is_uncached() {
                    let lower_bits = (self.rf.next_pc as u32) & 0xfff;
                    // Do an uncached instruction fetch
                    MemoryReq::UncachedInstructionRead(self.ic.expected_tag.tag() | lower_bits)
                } else {
                    let cache_line = (self.rf.next_pc as u32) & 0x0000_3fe0;
                    let physical_address = self.ic.expected_tag.tag() | cache_line;

                    MemoryReq::ICacheFill(physical_address)
                };

                return ExitReason::Mem(req);
            } else if self.ic.stalled {
                // We should be able to get away with a simplified blocked check here
                if !writeback_has_work {
                    debug_assert!(self.blocked());
                    return ExitReason::Blocked;

                } else {
                    debug_assert!(!self.blocked());
                    return ExitReason::Ok;
                }
            } else {
                debug_assert!(!self.ic.expected_tag.is_valid());

                // ITLB missed. We need to query the Joint-TLB for a result
                todo!("JTLB lookup");
                //return ExitReason::Ok;
            }
        }

        // ==========================
        // Stage 1: Instruction Cache
        // ==========================
        (self.ic.cache_data, self.ic.cache_tag) = icache.fetch(self.rf.next_pc);
        self.ic.expected_tag = itlb.translate(self.rf.next_pc);

        return ExitReason::Ok;
    }

    fn run_regfile(instruction_word: u32, rf: &mut RegisterFile, regfile: &mut RegFile) {
        let (inst, inst_info) = super::instructions::decode(instruction_word);
        let j: JType = inst.into();
        let i: IType = inst.into();
        let r: RType = inst.into();

        if let InstructionInfo::Op(_, _, _, rf_mode, ex_mode) = *inst_info {
            rf.ex_mode = ex_mode;
            // The register read always happens
            // Especially the bypass logic, which might trigger a load dependency even if
            // the instruction doesn't use the value.
            let rs_val = regfile.read(i.rs());
            let rt_val = regfile.read(i.rt());

            //println!("RF: {:?}", rf_mode);
            match rf_mode {
                // PERF: This could be simplified down to just a few flags
                //       But would that be faster than the jump table this compiles to?
                RfMode::JumpImm => {
                    let upper_bits = rf.next_pc & 0xffff_ffff_f000_0000;
                    rf.temp = (j.target() as u64) << 2 | upper_bits;
                    rf.writeback_reg = 0;
                }
                RfMode::JumpImmLink => {
                    let upper_bits = rf.next_pc & 0xffff_ffff_f000_0000;
                    rf.temp = (j.target() as u64) << 2 | upper_bits;
                    rf.writeback_reg = 31;
                }
                RfMode::JumpReg => {
                    rf.temp = rs_val;
                    rf.writeback_reg = 0;
                }
                RfMode::JumpRegLink => {
                    rf.temp = rs_val;
                    rf.writeback_reg = r.rd();
                }
                RfMode::BranchImm1 => {
                    rf.alu_a = rs_val;
                    rf.alu_b = 0;
                    let offset = (i.imm() as i16 as u64) << 2;
                    rf.temp = rf.next_pc + offset;
                    rf.writeback_reg = 0;
                }
                RfMode::BranchImm2 => {
                    rf.alu_a = rs_val;
                    rf.alu_b = rt_val;
                    let offset = (i.imm() as i16 as u64) << 2;
                    rf.temp = rf.next_pc.wrapping_add(offset);
                    rf.writeback_reg = 0;
                }
                RfMode::BranchLinkImm => {
                    rf.alu_a = rs_val;
                    rf.alu_b = 0;
                    let offset = (i.imm() as i16 as u64) << 2;
                    rf.temp = rf.next_pc.wrapping_add(offset);
                    rf.writeback_reg = 31;
                }
                RfMode::ImmSigned => {
                    rf.alu_a = rs_val;
                    rf.alu_b = i.imm() as i16 as u64;
                    rf.writeback_reg = i.rt();
                }
                RfMode::ImmUnsigned => {
                    rf.alu_a = rs_val;
                    rf.alu_b = i.imm() as u64;
                    rf.writeback_reg = i.rt();
                }
                RfMode::Mem => {
                    rf.alu_a = rs_val;
                    rf.alu_b = rt_val;
                    rf.temp = i.imm() as i16 as u64;
                    rf.writeback_reg = i.rt();
                }
                RfMode::RegReg => {
                    rf.alu_a = rs_val;
                    rf.alu_b = rt_val;
                    rf.writeback_reg = r.rd();
                }
                RfMode::RegRegNoWrite => {
                    rf.alu_a = rs_val;
                    rf.alu_b = rt_val;
                    rf.writeback_reg = 0;
                }
                RfMode::SmallImm => {
                    rf.alu_a = r.rs() as u64;
                    rf.alu_b = rt_val;
                    rf.writeback_reg = r.rd();
                }
                RfMode::SmallImmOffset32 => {
                    rf.alu_a = r.rs() as u64 + 32;
                    rf.alu_b = rt_val;
                    rf.writeback_reg = r.rd();
                }
                RfMode::SmallImmNoWrite => {
                    rf.alu_a = r.rs() as u64;
                    rf.alu_b = rt_val;
                    rf.writeback_reg = 0;
                }
                RfMode::RfUnimplemented => {
                    println!("Unimplemented Rfmode");
                }
            }
        } else {
            match inst_info {
                InstructionInfo::Reserved =>
                    todo!("Exception on reserved instruction {:08x}", instruction_word),
                InstructionInfo::Unimplemented(name, _) =>
                    todo!("Unimplemented unstruction {} ({:08x})", name, instruction_word),
                _ => unreachable!(),
            }
        }
    }

    fn run_ex_phase1(rf: &RegisterFile, ex: &mut Execute) {
        let old_pc = ex.next_pc;
        ex.next_pc = rf.next_pc;
        ex.trap = false;
        ex.mem_size = 0;
        ex.writeback_reg = rf.writeback_reg;

        //println!("EX: {:?}", rf.ex_mode);

        match rf.ex_mode {
            ExMode::Nop => {
                ex.writeback_reg = 0;
                ex.next_pc = old_pc; // Don't let Nop clobber pending branches
            }
            ExMode::Jump => {
                // This looks sus...
                // Why do relative jumps not need a subtract?
                ex.next_pc = rf.temp - 4;
                ex.alu_out = rf.next_pc + 4;
            }
            ExMode::Branch(cmp) => {
                // PERF: Check the compiler will automatically duplicate this case?
                //       Or should we be doing that optimization manually?
                if Self::compare(cmp, rf.alu_a as i64, rf.alu_b as i64) {
                    ex.next_pc = rf.temp;
                    ex.alu_out = rf.next_pc + 4;
                } else {
                    // Cancel write to link register
                    ex.writeback_reg = 0;
                }
            }
            ExMode::BranchLikely(cmp) => {
                if Self::compare(cmp, rf.alu_a as i64, rf.alu_b as i64) {
                    ex.next_pc = rf.temp;
                    ex.alu_out = rf.next_pc + 4;
                } else {
                    // branch likely instructions skip execution of the branch delay slot
                    // when the branch IS NOT TAKEN. Which is stupid.
                    ex.skip_next = true;
                    ex.writeback_reg = 0;
                }
            }
            ExMode::Add32 => {
                if let Some(alu_out) = (rf.alu_a as i32).checked_add(rf.alu_b as i32) {
                    ex.alu_out = alu_out as u64; // sign extend
                } else {
                    ex.trap = true;
                }
            }
            ExMode::AddU32 => {
                let out = rf.alu_a.wrapping_add(rf.alu_b) as u32;
                ex.alu_out = out as i32 as u64; // sign extend
            }
            ExMode::Add64 => {
                if let Some(alu_out) = (rf.alu_a as i64).checked_add(rf.alu_b as i64) {
                    ex.alu_out = alu_out as u64;
                } else {
                    ex.trap = true;
                }
            }
            ExMode::AddU64 => {
                ex.alu_out = rf.alu_a.wrapping_add(rf.alu_b);
            }
            ExMode::Sub32 => {
                if let Some(alu_out) = (rf.alu_b as i32).checked_sub(rf.alu_a as i32) {
                    ex.alu_out = alu_out as u64; // sign extend
                } else {
                    ex.trap = true;
                }
            }
            ExMode::SubU32 => {
                let out = (rf.alu_b as u32).wrapping_sub(rf.alu_a as u32);
                ex.alu_out = out as i32 as u64; // sign extend
            }
            ExMode::Sub64 => {
                if let Some(alu_out) = (rf.alu_b as i64).checked_sub(rf.alu_a as i64) {
                    ex.alu_out = alu_out as u64;
                } else {
                    ex.trap = true;
                }
            }
            ExMode::SubU64 => {
                ex.alu_out = rf.alu_b.wrapping_sub(rf.alu_a);
            }
            ExMode::SetLess => {
                ex.alu_out = if (rf.alu_a as i64) < (rf.alu_b as i64) { 1 } else { 0 };
            }
            ExMode::SetLessU => {
                ex.alu_out = if rf.alu_a < rf.alu_b { 1 } else { 0 };
            }
            ExMode::And => {
                ex.alu_out = rf.alu_a & rf.alu_b;
            }
            ExMode::Or => {
                ex.alu_out = rf.alu_a | rf.alu_b;
            }
            ExMode::Xor => {
                ex.alu_out = rf.alu_a ^ rf.alu_b;
            }
            ExMode::Nor => {
                ex.alu_out = !(rf.alu_a | rf.alu_b);
            }
            ExMode::InsertUpper => {
                let out = (rf.alu_b as u32) << 16;
                ex.alu_out = out as i32 as u64; // sign extend
            }
            ExMode::ShiftLeft32 => {
                let out = (rf.alu_a as u32).wrapping_shl(rf.alu_b as u32);
                ex.alu_out = out as i32 as u64; // sign extend
            }
            ExMode::ShiftRight32 => {
                let out = (rf.alu_a as u32).wrapping_shr(rf.alu_b as u32);
                ex.alu_out = out as i32 as u64; // sign extend
            }
            ExMode::ShiftRightArith32 => {
                let out = (rf.alu_a as i32).wrapping_shr(rf.alu_b as u32);
                ex.alu_out = out as u64; // sign extend
            }
            ExMode::ShiftLeft64 => {
                ex.alu_out = rf.alu_a.wrapping_shl(rf.alu_b as u32);
            }
            ExMode::ShiftRight64 => {
                ex.alu_out = rf.alu_a.wrapping_shr(rf.alu_b as u32);
            }
            ExMode::ShiftRightArith64 => {
                ex.alu_out = (rf.alu_a as i64).wrapping_shr(rf.alu_b as u32) as u64;
            }
            ExMode::Mul32 => {
                let out = (rf.alu_a as i32 as i64).wrapping_mul(rf.alu_b as i32 as i64);
                let hi = (out >> 32) as i32 as u64; // sign extend
                let lo = out as i32 as u64; // sign extend
                ex.hilo = [hi, lo];
            }
            ExMode::MulU32 => {
                let out = (rf.alu_a as u32 as u64).wrapping_mul(rf.alu_b as u32 as u64);
                let hi = (out >> 32) as i32 as u64; // sign extend
                let lo = out as i32 as u64; // sign extend
                ex.hilo = [hi, lo];
            }
            ExMode::Mul64 => {
                let a = rf.alu_a as i64 as i128;
                let b = rf.alu_b as i64 as i128;
                let out: u128 = a.wrapping_mul(b) as u128;
                let hi = (out as u128 >> 64) as u64;
                let lo = out as u64;
                ex.hilo = [hi, lo];
            }
            ExMode::MulU64 => {
                let a = rf.alu_a as u128;
                let b = rf.alu_b as u128;
                let out: u128 = a.wrapping_mul(b);
                let hi = (out >> 64) as u64;
                let lo = out as u64;
                ex.hilo = [hi, lo];
            }
            ExMode::Div32 => {
                if rf.alu_b as i32 == 0 {
                    // Manual says this is undefined. Ares implements it as:
                    let lo = if (rf.alu_a as i32) < 0 { u64::MAX } else { 1 };
                    let hi = rf.alu_a as i32 as u64;
                    ex.hilo = [hi, lo];
                } else {
                    let div = (rf.alu_a as i32).wrapping_div(rf.alu_b as i32);
                    let rem = (rf.alu_a as i32).wrapping_rem(rf.alu_b as i32);
                    let hi = rem as u64;
                    let lo = div as u64;
                    ex.hilo = [hi, lo];
                }
            }
            ExMode::DivU32 => {
                if rf.alu_b as u32 == 0 {
                    // Ares:
                    let lo = u64::MAX;
                    let hi = rf.alu_a as i32 as u64;
                    ex.hilo = [hi, lo];
                } else {
                    let div = (rf.alu_a as u32).wrapping_div(rf.alu_b as u32);
                    let rem = (rf.alu_a as u32).wrapping_rem(rf.alu_b as u32);
                    let hi = rem as u64;
                    let lo = div as u64;
                    ex.hilo = [hi, lo];
                }
            }
            ExMode::Div64 => {
                if rf.alu_b == 0 {
                    // Ares:
                    let lo = if (rf.alu_a as i64) < 0 { u64::MAX } else { 1 };
                    let hi = rf.alu_a;
                    ex.hilo = [hi, lo];
                } else {
                    let div = (rf.alu_a as i64).wrapping_div(rf.alu_b as i64);
                    let rem = (rf.alu_a as i64).wrapping_rem(rf.alu_b as i64);
                    let hi = rem as u64;
                    let lo = div as u64;
                    ex.hilo = [hi, lo];
                }
            }
            ExMode::DivU64 => {
                if rf.alu_b == 0 {
                    // Ares:
                    let lo = u64::MAX;
                    let hi = rf.alu_a;
                    ex.hilo = [hi, lo];
                } else {
                    let div = rf.alu_a.wrapping_div(rf.alu_b);
                    let rem = rf.alu_a.wrapping_rem(rf.alu_b);
                    let hi = rem as u64;
                    let lo = div as u64;
                    ex.hilo = [hi, lo];
                }
            }
            ExMode::Load(size) => {
                ex.addr = rf.alu_a.wrapping_add(rf.temp);
                let align = (ex.addr & 0x7) as u8;
                if align & (size - 1) == 0 {
                    ex.mem_mode = Some(MemMode::LoadSignExtend(8 * align, 8 * (8 - size)));
                    ex.mem_size = size;
                } else {
                    todo!("Misalignment exception, addr={:x}, size={}", ex.addr, size);
                }
            }
            ExMode::LoadUnsigned(size) => {
                ex.addr = rf.alu_a.wrapping_add(rf.temp);
                let align = (ex.addr & 0x7) as u8;
                if align & (size - 1) == 0 {
                    ex.mem_mode = Some(MemMode::LoadZeroExtend(8 * align, 8 * (8 - size)));
                    ex.mem_size = size;
                } else {
                    todo!("Misalignment exception");
                }
            }
            ExMode::LoadLeft(size) => {
                ex.addr = rf.alu_a.wrapping_add(rf.temp);
                let align = (ex.addr & 0x7) as u8;
                if size == 4 {
                    // LoadMergeWord actually applies the mask after word-aligning, so we are only
                    // using the lower half of this mask
                    ex.mem_mask = ByteMask8::new(4 - (align & 0x3), align & 0x3);
                    ex.mem_mode = Some(MemMode::LoadMergeWord(align));
                } else {
                    ex.mem_mask = ByteMask8::new(8 - align, align);
                    ex.mem_mode = Some(MemMode::LoadMergeDouble(align));
                }
                ex.mem_size = size;
            }
            ExMode::LoadRight(size) => {
                ex.addr = rf.alu_a.wrapping_add(rf.temp);
                ex.alu_out = rf.alu_b;
                let align = (ex.addr & 0x7) as u8;
                if size == 4 {
                    // LoadMergeWord actually applies the mask after word-aligning, so we are only
                    // using the lower half of this mask
                    ex.mem_mask = ByteMask8::new(4 - (4 - (align & 0x3)), 0u32);
                    ex.mem_mode = Some(MemMode::LoadMergeWord(align));
                } else {
                    ex.mem_mask = ByteMask8::new(8 - (8 - align), 0u32);
                    ex.mem_mode = Some(MemMode::LoadMergeDouble(align));
                }
                ex.mem_size = size;
            }
            ExMode::Store(size) => {
                ex.addr = rf.alu_a.wrapping_add(rf.temp);
                ex.writeback_reg = 0;
                let align = (ex.addr & 0x7) as u8;
                if align & (size - 1) == 0 {
                    ex.mem_mode = Some(MemMode::Store);
                    ex.mem_size = size;
                    ex.mem_mask = ByteMask8::new(size, align);
                    assert!(ex.mem_mask.size() == size as u32 * 8);
                    ex.alu_out = rf.alu_b.wrapping_shl(8 * (8 - (size + align)) as u32);
                } else {
                    todo!("Misalignment exception");
                }
            }
            ExMode::StoreLeft(size) => {
                ex.addr = rf.alu_a.wrapping_add(rf.temp);
                ex.writeback_reg = 0;
                let align = (ex.addr & 0x7) as u32;
                ex.mem_mode = Some(MemMode::Store);
                ex.mem_size = size;
                ex.alu_out = rf.alu_b.wrapping_shl(8 * align);
                if size == 4 {
                    ex.mem_mask = ByteMask8::new(4 - (align & 0x3), align & 0x3);

                } else {
                    ex.mem_mask = ByteMask8::new(8 - align, align);
                }
            }
            ExMode::StoreRight(size) => {
                ex.addr = rf.alu_a.wrapping_add(rf.temp);
                ex.writeback_reg = 0;
                let align = (ex.addr & 0x7) as u32;
                ex.mem_mode = Some(MemMode::Store);
                ex.mem_size = size;
                ex.alu_out = rf.alu_b.wrapping_shl(8 * (align & 0x4));
                if size == 4 {
                    ex.mem_mask = ByteMask8::new(4 - (4 - (align & 0x3)), align & 0x4);
                } else {
                    ex.mem_mask = ByteMask8::new(8 - (8 - align), 0u32);
                }
            }
            ExMode::MemLoadLinked(size) => {
                ex.addr = rf.alu_a.wrapping_add(rf.temp);
                let align = (ex.addr & 0x7) as u8;
                if align & (size - 1) == 0 {
                    ex.mem_mode = Some(MemMode::LoadSignExtend(8 * align, 8 * (8 - size)));
                    ex.mem_size = size;
                } else {
                    todo!("Misalignment exception");
                }

                ex.ll_bit = true;
                ex.ll_addr = ex.addr;
            }
            ExMode::MemStoreConditional(size) => {
                ex.addr = rf.alu_a.wrapping_add(rf.temp);
                let align = (ex.addr & 0x7) as u32;
                if align & (size as u32 - 1) == 0 {
                    ex.mem_mode = if ex.ll_bit {
                        Some(MemMode::ConditionalStore)
                    } else {
                        Some(MemMode::ConditionalStoreFail)
                    };
                    ex.mem_size = size;
                    ex.mem_mask = ByteMask8::new(size, align);
                    ex.alu_out = rf.alu_b.wrapping_shl(8 * (8 - align));
                } else {
                    todo!("Misalignment exception");
                }
            }
            ExMode::LoadInternal(reg) => {
                // HWTEST: The VR manual says accessing these registers more than two cycles before
                //         and an instruction that uses to them is undefined (if an exception happens)
                //         So.... need to work out what's actually going on here.
                ex.alu_out = ex.hilo[reg as usize];
            }
            ExMode::StoreInternal(reg) => {
                // HWTEST: same as above
                ex.hilo[reg as usize] = rf.alu_a;
            }
            ExMode::CacheOp => {
                ex.addr = rf.alu_a.wrapping_add(rf.alu_b);
                let op = rf.writeback_reg;
                match op {
                    0b00001 => {
                        let tag = (ex.addr >> 4) & 0x1ff;
                        println!("Unimplemented CacheOp - DCache Index_Write_Back_Invalidate: {:03x}", tag);
                    }
                    0b01000 => {
                        let tag = (ex.addr >> 5) & 0x1ff;
                        println!("Unimplemented CacheOp - ICache Index_Store_Tag: {:03x}", tag);
                    }
                    op if op & 0x1 == 0 => {
                        let tag = (ex.addr >> 5) & 0x1ff;
                        panic!("Unimplemented CacheOp: ICache {:02x} to {:03x}", op, tag);
                    }
                    op if op & 0x1 == 1 => {
                        let tag = (ex.addr >> 4) & 0x1ff;
                        panic!("Unimplemented CacheOp: DCache {:02x} to {:03x}", op, tag);
                    }
                    op => panic!("Undefined CacheOp: {:02x}", op),
                }
                ex.writeback_reg = 0;
            }
            ExMode::ExUnimplemented => {
                println!("Unimplemented Exmode");
            }
        }
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

pub fn create() -> Pipeline {
    let reset_pc = 0xffff_ffff_bfc0_0000;

    Pipeline{
        ic: InstructionCache{
            cache_data: 0,
            cache_tag: CacheTag::empty(),
            // Start with the first instruction fetch already started.
            // Otherwise the RF stage will incorrectly start a ITLB miss on the first cycle
            expected_tag: CacheTag::new_uncached(reset_pc as u32 & 0x1fff_ffff),
            stalled: false,
        },
        rf: RegisterFile{
            next_pc: reset_pc,
            alu_a: 0,
            alu_b: 0,
            temp: 0,
            writeback_reg: 0,
            ex_mode: ExMode::Nop,
        },
        ex: Execute{
            next_pc: reset_pc,
            alu_out: 0,
            addr: 0,
            skip_next: false,
            mem_size: 0,
            mem_mode: None,
            mem_mask: Default::default(),
            trap: false,
            writeback_reg: 0,
            subinstruction_cycle: 0,
            hilo: [0, 0],
            ll_bit: false,
            ll_addr: 0,
        },
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
