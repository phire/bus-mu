use crate::{instructions::{JType, IType, RType, InstructionInfo}, pipeline::MemoryReq, regfile::RegFile};

use super::{execute::{ExMode, Execute}, ExitReason, instruction_cache::InstructionCache};
pub struct RegisterFile {
    pub next_pc: u64,
    pub alu_a: u64,
    pub alu_b: u64,
    pub temp: u64, // Either result of jump calculation, or value to store
    pub writeback_reg: u8,
    pub ex_mode: ExMode,
}


#[derive(Debug, Clone, Copy)]
pub enum RfMode {
    JumpImm,
    JumpImmLink, // could squash these links?
    JumpReg,
    JumpRegLink,
    BranchImm1,
    BranchImm2,
    BranchLinkImm,
    ImmSigned,
    ImmUnsigned,
    Mem,
    RegReg,
    RegRegNoWrite,
    SmallImm,
    SmallImmOffset32,
    SmallImmNoWrite,
    RfUnimplemented,
}

impl RegisterFile {

    #[inline(always)]
    pub fn cycle(&mut self, ic: &mut InstructionCache, ex: &Execute, regs: &mut RegFile, writeback_has_work: bool) -> Result<(), ExitReason> {
        // First we check the result of the Instruction Cache stage
        // ICache always returns an instruction, but it might be the wrong one
        // The only way to know is to check the output of ITLB matches the tag ICache returned

        // PERF: How to we tell the compiler this first case is the most likely?
        if ic.cache_tag == ic.expected_tag {
            debug_assert!(ic.stalled == false);
            debug_assert!(ic.expected_tag.is_valid());

            regs.bypass(
                ex.writeback_reg,
                match ex.mem_mode {
                    Some(_) => Some(ex.alu_out),
                    None => None
                });

            // ICache hit. We can continue with the rest of this stage
            run_regfile(ic.cache_data, self, regs);

            if regs.hazard_detected() {
                // regfile detected a hazard (register value is dependent on memory load)
                // The output of this stage is invalid, but we will retry next cycle
                self.ex_mode = ExMode::Nop;
                return Err(ExitReason::Stalled);
            }

            self.next_pc = ex.next_pc + 4;

        } else if !ic.stalled && ic.cache_tag != ic.expected_tag {
            // PERF: Can we tell the compiler this block is more likely than the next?

            debug_assert!(ic.expected_tag.is_valid());
            ic.stalled = true;
            self.ex_mode = ExMode::Nop;

            let req = if ic.expected_tag.is_uncached() {
                let lower_bits = (self.next_pc as u32) & 0xfff;
                // Do an uncached instruction fetch
                MemoryReq::UncachedInstructionRead(ic.expected_tag.tag() | lower_bits)
            } else {
                let cache_line = (self.next_pc as u32) & 0x0000_3fe0;
                let physical_address = ic.expected_tag.tag() | cache_line;

                MemoryReq::ICacheFill(physical_address)
            };

            return Err(ExitReason::Mem(req));
        } else if ic.stalled {
            // We should be able to get away with a simplified blocked check here
            if !writeback_has_work {
                return Err(ExitReason::Blocked);
            } else {
                return Err(ExitReason::Stalled);
            }
        } else {
            debug_assert!(!ic.expected_tag.is_valid());

            // ITLB missed. We need to query the Joint-TLB for a result
            todo!("JTLB lookup");
            //return ExitReason::Ok;
        }

        Ok(())
    }

}


fn run_regfile(instruction_word: u32, rf: &mut RegisterFile, regfile: &mut RegFile) {
    let (inst, inst_info) = crate::instructions::decode(instruction_word);
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

impl Default for RegisterFile {
    fn default() -> Self {
        RegisterFile{
            next_pc: super::RESET_PC,
            alu_a: 0,
            alu_b: 0,
            temp: 0,
            writeback_reg: 0,
            ex_mode: ExMode::Nop,
        }
    }
}