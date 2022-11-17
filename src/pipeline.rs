use crate::{
    CacheTag, ICache, ITlb, RegFile,
    instructions::{
        InstructionInfo,
        IType,
        RfMode,
        JType,
        RType,
        ExMode,
        CmpMode
    }
};

struct InstructionCache {
    cache_data: u32,
    cache_tag: CacheTag,
    expected_tag: Option<u32>,
}

enum AluMode {
    Add,
}

struct RegisterFile {
    next_pc: u64,
    alu_a: u64,
    alu_b: u64,
    temp: u64, // Either result of jump calculation, or value to store
    write_back: u8,
    ex_mode: ExMode,
    cmp_mode: CmpMode,
    store: bool,
}

struct Execute {
    next_pc: u64,
    alu_out: u64,
    addr: u64,
    skip_next: bool, // Used to skip the op about to be executed in RF stage
    mem_size: u8, // 0 is no mem access
    store: bool,
    trap: bool,
}
struct DataCache {}
struct WriteBack {}

struct Pipeline {
    ic: InstructionCache,
    rf: RegisterFile,
    ex: Execute,
    dc: DataCache,
    wb: WriteBack,
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

    fn run_ex(rf: &RegisterFile, ex: &mut Execute, hilo: &mut [u64; 2]) {
        ex.next_pc = rf.next_pc;
        ex.trap = false;
        ex.mem_size = 0;

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
                ex.alu_out = (rf.alu_a & 0xFFFF) | (rf.alu_b << 16);
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
                let out = a.wrapping_mul(b);
                let hi = (out >> 64) as u64;
                let lo = out as u64;
                *hilo = [hi, lo];
            }
            ExMode::MulU64 => {
                let out = rf.alu_a.wrapping_mul(rf.alu_b);
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
        }
    }
    pub fn cycle(
        &mut self,
        icache: &mut ICache,
        itlb: &mut ITlb,
        mem: crate::MemSubsystemState,
        regfile: &mut RegFile,
    ) {
        // Phase 1
        // IC
        // ...
        // RF
        // Instruction Cache Tag Check
        let hit =
            self.ic.cache_tag.valid() && Some(self.ic.cache_tag.tag()) == self.ic.expected_tag;

        // DC Phase 1
        if (self.ex.mem_size != 0) {
            let addr = self.ex.addr;
            let data = self.ex.alu_out;
            let size = self.ex.mem_size;
            match size {
                // 1 => mem.write_byte(addr, data as u8),
                // 2 => mem.write_half(addr, data as u16),
                // 4 => mem.write_word(addr, data as u32),
                // 8 => mem.write_double(addr, data),
                _ => unreachable!(),
            }
        }

        // EX
        Self::run_ex(&self.rf, &mut self.ex, &mut regfile.hilo);


        // Phase 2
        // IC
        (self.ic.cache_data, self.ic.cache_tag) = icache.fetch(self.rf.next_pc);
        self.ic.expected_tag = itlb.translate(self.rf.next_pc, &mem);

        // RF
        self.rf.next_pc = self.ex.next_pc + 4;
        if !self.ex.skip_next {
            let (inst, inst_info) = crate::instructions::decode(self.ic.cache_data);
            let j: JType = inst.into();
            let i: IType = inst.into();
            let r: RType = inst.into();

            if let InstructionInfo::Op(_, _, _, rf_mode, ex_mode) = *inst_info {
                self.rf.ex_mode = ex_mode;
                match rf_mode {
                    RfMode::JumpImm => {
                        let upper_bits = self.rf.next_pc & 0xffff_ffff_f000_0000;
                        self.rf.temp = (j.target() as u64) << 2 | upper_bits;
                        self.rf.write_back = 0;
                    }
                    RfMode::JumpImmLink => {
                        let upper_bits = self.rf.next_pc & 0xffff_ffff_f000_0000;
                        self.rf.temp = (j.target() as u64) << 2 | upper_bits;
                        self.rf.write_back = 31;
                    }
                    RfMode::JumpReg => {
                        self.rf.temp = regfile.read(r.rs());
                        self.rf.write_back = 0;
                    }
                    RfMode::JumpRegLink => {
                        self.rf.temp = regfile.read(r.rs());
                        self.rf.write_back = r.rd();
                    }
                    RfMode::BranchImm1 => {
                        self.rf.alu_a = regfile.read(i.rs());
                        self.rf.alu_b = 0;
                        let offset = (i.imm() as i16 as u64) << 2;
                        self.rf.temp = self.rf.next_pc + offset;
                        self.rf.write_back = 0;
                    }
                    RfMode::BranchImm2 => {
                        self.rf.alu_a = regfile.read(i.rs());
                        self.rf.alu_b = regfile.read(i.rt());
                        let offset = (i.imm() as i16 as u64) << 2;
                        self.rf.temp = self.rf.next_pc + offset;
                        self.rf.write_back = 0;
                    }
                    RfMode::BranchLinkImm => {
                        self.rf.alu_a = regfile.read(i.rs());
                        self.rf.alu_b = 0;
                        let offset = (i.imm() as i16 as u64) << 2;
                        self.rf.temp = self.rf.next_pc + offset;
                        self.rf.write_back = 31;
                    }
                    RfMode::ImmSigned => {
                        self.rf.alu_a = regfile.read(i.rs());
                        self.rf.alu_b = i.imm() as i16 as u64;
                        self.rf.write_back = i.rt();
                        self.rf.store = false;
                    }
                    RfMode::ImmUnsigned => {
                        self.rf.alu_a = regfile.read(i.rs());
                        self.rf.alu_b = i.imm() as u64;
                        self.rf.write_back = i.rt();
                    }
                    RfMode::StoreOp => {
                        self.rf.alu_a = regfile.read(i.rs());
                        self.rf.alu_b = i.imm() as u64;
                        self.rf.write_back = 0;
                        self.rf.temp = regfile.read(i.rt());
                        self.rf.store = true;
                    }
                    RfMode::RegReg => {
                        self.rf.alu_a = regfile.read(r.rs());
                        self.rf.alu_b = regfile.read(r.rt());
                        self.rf.write_back = r.rd();
                    }
                    RfMode::MulDiv => {
                        self.rf.alu_a = regfile.read(r.rs());
                        self.rf.alu_b = regfile.read(r.rt());
                        self.rf.write_back = 0;
                    }
                    RfMode::ShiftImm => {
                        self.rf.alu_a = r.rs() as u64;
                        self.rf.alu_b = regfile.read(r.rt());
                        self.rf.write_back = r.rd();
                    }
                    RfMode::ShiftImm32 => {
                        self.rf.alu_a = r.rs() as u64 + 32;
                        self.rf.alu_b = regfile.read(r.rt());
                        self.rf.write_back = r.rd();
                    }
                }
            } else {
                todo!("Exception on reserved instruction");
            }
        }


        // EX
        // nothing

        //
    }
}
