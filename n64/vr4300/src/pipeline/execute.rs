use common::util::ByteMask8;

use super::{register_file::RegisterFile, data_cache::MemMode, ExitReason};


pub struct Execute {
    pub next_pc: u64,
    pub alu_out: u64,
    pub addr: u64,
    pub skip_next: bool, // Used to skip the op about to be executed in RF stage
    pub mem_size: u8,
    pub mem_mode: Option<MemMode>,
    pub mem_mask: ByteMask8,
    pub trap: bool,
    pub writeback_reg: u8,

    // internal storage
    pub hilo: [u64; 2],
    pub ll_bit: bool,
    pub ll_addr: u64,

    pub subinstruction_cycle: u32,
}

impl Default for Execute {
    fn default() -> Self {
        Execute{
            next_pc: super::RESET_PC,
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
        }
    }
}

impl Execute {
    #[inline(always)]
    pub fn cycle(&mut self, rf: &RegisterFile) -> Result<(), ExitReason> {
        self.mem_mode = None;
        self.writeback_reg = 0;

        // PERF: we might be able to move this skip logic into the jump table
        if self.skip_next {
            match rf.ex_mode {
                ExMode::Nop => {
                    // FIXME: This is going to break when there is a nop instruction in the branch delay slot
                }
                _ => {
                    // For some reason... branch likely instructions invalidate the branch-delay
                    // slot's instruction if they aren't taken... Which is backwards
                    //println!("Skipping instruction {:?}", self.rf.ex_mode);
                    self.skip_next = false;
                    self.next_pc = rf.next_pc;
                }
            }
        } else {
            run_ex_phase1(rf,self);

            if self.subinstruction_cycle != 0 {
                // The pipeline is stalled, executing a multi-cycle instruction
                return Err(ExitReason::Stalled);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ExMode {
    Nop,
    Jump,
    Branch(CmpMode),
    BranchLikely(CmpMode),
    Add32,
    AddU32,
    Add64,
    AddU64,
    Sub32,
    Sub64,
    SubU32,
    SubU64,
    SetLess,
    SetLessU,
    And,
    Or,
    Xor,
    Nor,
    InsertUpper,
    ShiftLeft32,
    ShiftRight32,
    ShiftRightArith32,
    ShiftLeft64,
    ShiftRight64,
    ShiftRightArith64,
    Mul32,
    MulU32,
    Div32,
    DivU32,
    Mul64,
    MulU64,
    Div64,
    DivU64,
    Load(u8),
    LoadUnsigned(u8),
    LoadLeft(u8),
    LoadRight(u8),
    MemLoadLinked(u8),
    Store(u8),
    StoreLeft(u8),
    StoreRight(u8),
    MemStoreConditional(u8),
    LoadInternal(InternalReg),
    StoreInternal(InternalReg),
    CacheOp,
    ExUnimplemented,
}

#[derive(Debug, Clone, Copy)]
pub enum InternalReg {
    HI = 0,
    LO = 1,
}

#[derive(Debug, Clone, Copy)]
pub enum CmpMode {
    Eq,
    Ne,
    Le,
    Ge,
    Lt,
    Gt,
}

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
            if compare(cmp, rf.alu_a as i64, rf.alu_b as i64) {
                ex.next_pc = rf.temp;
                ex.alu_out = rf.next_pc + 4;
            } else {
                // Cancel write to link register
                ex.writeback_reg = 0;
            }
        }
        ExMode::BranchLikely(cmp) => {
            if compare(cmp, rf.alu_a as i64, rf.alu_b as i64) {
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