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
}

struct RegisterFile {
    next_pc: u64,
    alu_a: u64,
    alu_b: u64,
    temp: u64, // Either result of jump calculation, or value to store
    writeback_reg: u8,
    ex_mode: ExMode,
    cmp_mode: CmpMode,
    store: bool,
    stalled: bool,
}

struct Execute {
    next_pc: u64,
    alu_out: u64,
    addr: u64,
    skip_next: bool, // Used to skip the op about to be executed in RF stage
    mem_size: u8, // 0 is no mem access
    store: bool,
    trap: bool,
    writeback_reg: u8,
}
struct DataCache {
    cache_attempt: DataCacheAttempt,
    tlb_tag: CacheTag,
    writeback_reg: u8,
    alu_out: u64,
    store: bool,
    mem_size: u8,
}
// struct WriteBack {}

pub struct Pipeline {
    ic: InstructionCache,
    rf: RegisterFile,
    ex: Execute,
    dc: DataCache,
    //wb: WriteBack,
}

pub enum MemoryReq
{
    ICacheFill(u32),
    DCacheFill(u32),
    DCacheReplace(u32, u32, [u8; 16]),
    UncachedInstructionRead(u32),
    UncachedDataRead(u32, u8),
    UncachedDataWrite(u32, u8, u64),
}

pub enum MemoryResponce
{
    ICacheFill([u32; 8]),
    DCacheFill([u8; 16]),
    UncachedInstructionRead(u32),
    UncachedDataRead(u64),
    UncachedDataWrite,
}

pub enum ExitReason
{
    Ok,
    //Stall(u8),
    Mem(MemoryReq),
}


impl Pipeline {
    fn compare(cmp: CmpMode, a: u64, b: u64) -> bool {
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

    pub fn cycle(
        &mut self,
        icache: &mut ICache,
        dcache: &mut DCache,
        itlb: &mut ITlb,
        regfile: &mut RegFile,
    ) -> ExitReason {
        // We evaluate the pipeline in reverse order.
        // So each stage can use the previous stage's output before it's overwritten
        // This also allows us to stall the pipeline by returning early.

        // ==================
        // Stage 5: WriteBack
        // ==================
        {
            // TODO: CP0 bypass interlock
            let mut writeback_value = self.dc.alu_out;

            // Finish DCache access from last stage
            if self.dc.mem_size != 0 {
                let cache_attempt = self.dc.cache_attempt;
                let tlb_tag = self.dc.tlb_tag;
                if cache_attempt.is_hit(tlb_tag) {
                    let mem_size = self.dc.mem_size as usize;
                    if self.dc.store {
                        cache_attempt.write(dcache, mem_size, writeback_value);
                    } else {
                        writeback_value = cache_attempt.read(&dcache, mem_size);
                    }
                } else {
                    return ExitReason::Mem(cache_attempt.do_miss(&dcache, tlb_tag, self.dc.mem_size, self.dc.store, writeback_value));
                }
            }

            // Register file writeback
            if self.dc.writeback_reg != 0 {
                // TODO: truncate to 32 bits if we are in 32bit mode
                regfile.write(self.dc.writeback_reg, writeback_value);
            }
        }

        // ==================
        // Stage 4: DataCache
        // ==================
        {
            if self.ex.mem_size != 0 {
                let addr = self.ex.addr;

                self.dc.cache_attempt = dcache.open(addr);
                // TODO: Implement TLB lookups
                self.dc.tlb_tag = CacheTag::new_uncached((addr as u32) & 0x1fff_ffff);
            }

            if self.ex.mem_size != 0 || self.ex.writeback_reg != 0 {
                // Forward from EX
                self.dc.alu_out = self.ex.alu_out;
                self.dc.writeback_reg = self.ex.writeback_reg;
                self.dc.store = self.ex.store;
                self.dc.mem_size = self.ex.mem_size;
            }
        }
        // ================
        // Stage 3: Execute
        // ================
        {
            // PERF: we can probably move these stalls/skips/hazards into the jump table
            if self.rf.stalled {
                self.ex.mem_size = 0;
                self.ex.writeback_reg = 0;
                return ExitReason::Ok;
            }

            if self.ex.skip_next || regfile.hazard_detected() {
                // branch likely instructions invalidate the branch-delay slot's instruction
                // if they aren't taken
                self.ex.skip_next = false;
                self.ex.mem_size = 0;
                self.ex.writeback_reg = 0;
            } else {
                Self::run_ex_phase1(&self.rf, &mut self.ex, &mut regfile.hilo);

                // TODO: return here if ex needs multiple cycles?
            }

            regfile.bypass(
                self.ex.writeback_reg,
                match self.ex.mem_size {
                    0 => Some(self.ex.alu_out),
                    _ => None
                });
        }

        // ======================
        // Stage 2: Register File
        // ======================
        {
            // First we check the result of the Instruction Cache stage
            // ICache always returns an instruction, but it might be the wrong one
            // The only way to know is to check the output of ITLB matches the tag ICache returned

            if !self.ic.expected_tag.is_valid() {
                // ITLB missed. We need to query the Joint-TLB for a result
                if self.ic.expected_tag == CacheTag::empty() {
                    // Umm.... FIXME!
                } else {
                    todo!("JTLB lookup");
                }
                //return ExitReason::Ok;
            } else if self.ic.cache_tag != self.ic.expected_tag {
                self.rf.stalled = true;
                if self.ic.expected_tag.is_uncached() {
                    let lower_bits = (self.rf.next_pc as u32) & 0xfff;
                    // Do an uncached instruction fetch
                    return ExitReason::Mem(
                        MemoryReq::UncachedInstructionRead(self.ic.expected_tag.tag() | lower_bits));

                } else {
                    let cache_line = (self.rf.next_pc as u32) & 0x0000_3fe0;
                    let physical_address = self.ic.expected_tag.tag() | cache_line;
                    return ExitReason::Mem(
                        MemoryReq::ICacheFill(physical_address)
                    );
                }
            } else {
                // ICache hit. We can continue with the rest of this stage
                Self::run_regfile(self.ic.cache_data, &mut self.rf, regfile);

                if regfile.hazard_detected() {
                    // regfile detected a hazard (register value is dependent on memory load)
                    // The output of this stage is invalid, but we can exit early and retry next cycle
                    return ExitReason::Ok;
                }
                self.rf.next_pc = self.ex.next_pc + 4;
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
            //println!("RF: {:?}", rf_mode);
            match rf_mode {
                // PERF: Maybe this can be simplified down to just a few flags
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
                    rf.temp = regfile.read(r.rs());
                    rf.writeback_reg = 0;
                }
                RfMode::JumpRegLink => {
                    rf.temp = regfile.read(r.rs());
                    rf.writeback_reg = r.rd();
                }
                RfMode::BranchImm1 => {
                    rf.alu_a = regfile.read(i.rs());
                    rf.alu_b = 0;
                    let offset = (i.imm() as i16 as u64) << 2;
                    rf.temp = rf.next_pc + offset;
                    rf.writeback_reg = 0;
                }
                RfMode::BranchImm2 => {
                    rf.alu_a = regfile.read(i.rs());
                    rf.alu_b = regfile.read(i.rt());
                    let offset = (i.imm() as i16 as u64) << 2;
                    rf.temp = rf.next_pc.wrapping_add(offset);
                    rf.writeback_reg = 0;
                }
                RfMode::BranchLinkImm => {
                    rf.alu_a = regfile.read(i.rs());
                    rf.alu_b = 0;
                    let offset = (i.imm() as i16 as u64) << 2;
                    rf.temp = rf.next_pc + offset;
                    rf.writeback_reg = 31;
                }
                RfMode::ImmSigned => {
                    rf.alu_a = regfile.read(i.rs());
                    rf.alu_b = i.imm() as i16 as u64;
                    rf.writeback_reg = i.rt();
                    rf.store = false;
                }
                RfMode::ImmUnsigned => {
                    rf.alu_a = regfile.read(i.rs());
                    rf.alu_b = i.imm() as u64;
                    rf.writeback_reg = i.rt();
                }
                RfMode::StoreOp => {
                    rf.alu_a = regfile.read(i.rs());
                    rf.alu_b = i.imm() as u64;
                    rf.writeback_reg = 0;
                    rf.temp = regfile.read(i.rt());
                    rf.store = true;
                }
                RfMode::RegReg => {
                    rf.alu_a = regfile.read(r.rs());
                    rf.alu_b = regfile.read(r.rt());
                    rf.writeback_reg = r.rd();
                }
                RfMode::RegRegNoWrite => {
                    rf.alu_a = regfile.read(r.rs());
                    rf.alu_b = regfile.read(r.rt());
                    rf.writeback_reg = 0;
                }
                RfMode::SmallImm => {
                    rf.alu_a = r.rs() as u64;
                    rf.alu_b = regfile.read(r.rt());
                    rf.writeback_reg = r.rd();
                }
                RfMode::SmallImmOffset32 => {
                    rf.alu_a = r.rs() as u64 + 32;
                    rf.alu_b = regfile.read(r.rt());
                    rf.writeback_reg = r.rd();
                }
                RfMode::SmallImmNoWrite => {
                    rf.alu_a = r.rs() as u64;
                    rf.alu_b = regfile.read(r.rt());
                    rf.writeback_reg = 0;
                }
                RfMode::RfUnimplemented => {
                    println!("Unimplemented Rfmode");
                }
            }
        } else {
            todo!("Exception on reserved instruction");
        }
    }

    fn run_ex_phase1(rf: &RegisterFile, ex: &mut Execute, hilo: &mut [u64; 2]) {
        ex.next_pc = rf.next_pc;
        ex.trap = false;
        ex.mem_size = 0;
        ex.writeback_reg = rf.writeback_reg;

        //println!("EX: {:?}", rf.ex_mode);

        match rf.ex_mode {
            ExMode::Jump => {
                ex.next_pc = rf.temp;
            }
            ExMode::Branch(cmp) => {
                if Self::compare(cmp, rf.alu_a, rf.alu_b) {
                    ex.next_pc = rf.temp;
                }
            }
            ExMode::BranchLikely(cmp) => {
                if Self::compare(cmp, rf.alu_a, rf.alu_b) {
                    ex.next_pc = rf.temp;
                    ex.skip_next = true;
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
                let out = rf.alu_b.wrapping_sub(rf.alu_a) as u32;
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
                let out = (rf.alu_a as i64).wrapping_mul(rf.alu_b as i64);
                let hi = (out >> 32) as i32 as u64; // sign extend
                let lo = out as i32 as u64; // sign extend
                *hilo = [hi, lo];
            }
            ExMode::MulU32 => {
                let out = (rf.alu_a as u64).wrapping_mul(rf.alu_b as u64);
                let hi = (out >> 32) as u32 as u64; // sign extend
                let lo = out as u32 as u64; // sign extend
                *hilo = [hi, lo];
            }
            ExMode::Mul64 => {
                let a = rf.alu_a as i64 as i128;
                let b = rf.alu_b as i64 as i128;
                let out: u128 = a.wrapping_mul(b) as u128;
                let hi = (out as u128 >> 64) as u64;
                let lo = out as u64;
                *hilo = [hi, lo];
            }
            ExMode::MulU64 => {
                let a = rf.alu_a as u128;
                let b = rf.alu_b as u128;
                let out: u128 = a.wrapping_mul(b);
                let hi = (out >> 64) as u64;
                let lo = out as u64;
                *hilo = [hi, lo];
            }
            ExMode::Div32 => {
                if rf.alu_b as i32 == 0 {
                    // Manual says this is undefined. Ares implements it as:
                    let lo = if (rf.alu_a as i32) < 0 { u64::MAX } else { 1 };
                    let hi = rf.alu_a as i32 as u64;
                    *hilo = [hi, lo];
                } else {
                    let div = (rf.alu_a as i32).wrapping_div(rf.alu_b as i32);
                    let rem = (rf.alu_a as i32).wrapping_rem(rf.alu_b as i32);
                    let hi = rem as u64;
                    let lo = div as u64;
                    *hilo = [hi, lo];
                }
            }
            ExMode::DivU32 => {
                if rf.alu_b as u32 == 0 {
                    // Ares:
                    let lo = u64::MAX;
                    let hi = rf.alu_a as i32 as u64;
                    *hilo = [hi, lo];
                } else {
                    let div = (rf.alu_a as u32).wrapping_div(rf.alu_b as u32);
                    let rem = (rf.alu_a as u32).wrapping_rem(rf.alu_b as u32);
                    let hi = rem as u64;
                    let lo = div as u64;
                    *hilo = [hi, lo];
                }
            }
            ExMode::Div64 => {
                if rf.alu_b == 0 {
                    // Ares:
                    let lo = if (rf.alu_a as i64) < 0 { u64::MAX } else { 1 };
                    let hi = rf.alu_a;
                    *hilo = [hi, lo];
                } else {
                    let div = (rf.alu_a as i64).wrapping_div(rf.alu_b as i64);
                    let rem = (rf.alu_a as i64).wrapping_rem(rf.alu_b as i64);
                    let hi = rem as u64;
                    let lo = div as u64;
                    *hilo = [hi, lo];
                }
            }
            ExMode::DivU64 => {
                if rf.alu_b == 0 {
                    // Ares:
                    let lo = u64::MAX;
                    let hi = rf.alu_a;
                    *hilo = [hi, lo];
                } else {
                    let div = rf.alu_a.wrapping_div(rf.alu_b);
                    let rem = rf.alu_a.wrapping_rem(rf.alu_b);
                    let hi = rem as u64;
                    let lo = div as u64;
                    *hilo = [hi, lo];
                }
            }
            ExMode::Mem(size) => {
                ex.addr = rf.alu_a.wrapping_add(rf.alu_b);
                ex.alu_out = rf.temp;
                ex.mem_size = size;
                ex.store = rf.store;
            }
            ExMode::MemUnsigned(size) => {
                ex.addr = rf.alu_a.wrapping_add(rf.alu_b);
                ex.alu_out = rf.temp;
                ex.mem_size = size;
                ex.store = rf.store;
            }
            ExMode::MemLeft(_) => todo!(),
            ExMode::MemRight(_) => todo!(),
            ExMode::MemLinked(_) => todo!(),
            ExMode::ExUnimplemented => {
                println!("Unimplemented Exmode");
            }
        }
    }

    pub fn memory_responce(&mut self, info: MemoryResponce, icache: &mut ICache,
        dcache: &mut DCache, regfile: &mut RegFile) {
        match info {
            MemoryResponce::ICacheFill(data) => {
                // Reconstruct line/offset from program counter
                let line = (self.pc() as usize >> 5) & 0x1ff;
                let offset = (self.pc() as usize) & 0x1f;
                let new_tag = self.ic.expected_tag;

                icache.finish_fill(line, new_tag, data);

                self.ic.cache_data = data[offset];
                self.ic.cache_tag = new_tag;
                self.rf.stalled = false;
            }
            MemoryResponce::UncachedInstructionRead(word) => {
                self.ic.cache_data = word;
                self.ic.cache_tag = self.ic.expected_tag;
                self.rf.stalled = false;
            }
            MemoryResponce::DCacheFill(data) => {
                self.dc.cache_attempt.finish_fill(dcache, self.dc.tlb_tag, data);
            }
            MemoryResponce::UncachedDataRead(value) => {
                if self.dc.writeback_reg != 0 {
                    // TODO: truncate to 32 bits if we are in 32bit mode
                    regfile.write(self.dc.writeback_reg, value);
                    self.dc.writeback_reg = 0;
                }
                self.dc.mem_size = 0;
            }
            MemoryResponce::UncachedDataWrite => {
                // Do nothing
            }
        }
    }

}

pub fn create() -> Pipeline {
    Pipeline{
        ic: InstructionCache{
            cache_data: 0,
            cache_tag: CacheTag::empty(),
            expected_tag: CacheTag::empty(),
        },
        rf: RegisterFile{
            next_pc: 0xffff_ffff_bfc0_0000,
            alu_a: 0,
            alu_b: 0,
            temp: 0,
            writeback_reg: 0,
            ex_mode: ExMode::Add32,
            cmp_mode: CmpMode::Eq,
            store: false,
            stalled: false,
        },
        ex: Execute{
            next_pc: 0,
            alu_out: 0,
            addr: 0,
            skip_next: false,
            mem_size: 0,
            store: false,
            trap: false,
            writeback_reg: 0,
        },
        dc: DataCache{
            cache_attempt: DataCacheAttempt::empty(),
            tlb_tag: CacheTag::empty(),
            writeback_reg: 0,
            alu_out: 0,
            store: false,
            mem_size: 0,
        },
        //wb: WriteBack{},
    }
}
