use std::collections::btree_set::Union;

use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

#[bitfield(bits = 32)]
#[derive(BitfieldSpecifier, Debug, Copy, Clone)]
pub struct IType {
    imm: B16,
    rt: B5,
    rs: B5,
    op: B6,
}

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct JType {
    target: B26,
    op: B6,
}

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct RType {
    funct: B6,
    sa: B5,
    rd: B5,
    rt: B5,
    rs: B5,
    op: B6,
}

#[bitfield(bits = 32)]
pub struct Inst {
    data: B26,
    op: B6,
}

impl Into<u32> for IType {
    fn into(self) -> u32 {
        u32::from_ne_bytes(self.into_bytes())
    }
}
impl Into<u32> for JType {
    fn into(self) -> u32 {
        u32::from_ne_bytes(self.into_bytes())
    }
}
impl Into<u32> for RType {
    fn into(self) -> u32 {
        u32::from_ne_bytes(self.into_bytes())
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Instruction {
    I(IType),
    J(JType),
    R(RType),
    ReservedInstructionException(u32),
}

impl Into<u32> for Instruction {
    fn into(self) -> u32 {
        match self {
            Instruction::I(i) => i.into(),
            Instruction::J(j) => j.into(),
            Instruction::R(r) => r.into(),
            Instruction::ReservedInstructionException(num) => num,
        }
    }
}

impl Instruction {
    fn into_bytes(self) -> [u8; 4] {
        match self {
            Instruction::I(i) => i.into_bytes(),
            Instruction::J(j) => j.into_bytes(),
            Instruction::R(r) => r.into_bytes(),
            Instruction::ReservedInstructionException(num) => num.to_ne_bytes(),
        }
    }
}

impl Into<IType> for Instruction {
    fn into(self) -> IType {
        IType::from_bytes(self.into_bytes())
    }
}
impl Into<RType> for Instruction {
    fn into(self) -> RType {
        RType::from_bytes(self.into_bytes())
    }
}
impl Into<JType> for Instruction {
    fn into(self) -> JType {
        JType::from_bytes(self.into_bytes())
    }
}

const MIPS_REG_NAMES: [&'static str; 32] = [
    "$zero", // Always 0
    "$at",   // r1 - Reserved for assembler
    "$v0", "$v1", // r2-r3 - Function return values
    "$a0", "$a1", "$a2", "$a3", // r4-r7 - function arguments
    "$t0", "t1", "$t2", "$t3", "$t4", "$t5", "$t6",
    "$t7", // r8-r15 - Temporaries (Caller saved)
    "$s0", "$s1", "$s2", "$s3", "$s4", "$s5", "$s6", "$s7", // r16-r23 - Saved  (Callee saved)
    "$t8", "$t9", // r26-r28 - Caller-saved temporaries
    "$k0", "$k1", // Reserved for OS kernel
    "$gp", // r28 - Global pointer
    "$sp", // r29 - Stack pointer
    "$fp", // r30 - Frame pointer
    "$ra", // r31 - Return address
];

impl Instruction {
    pub fn name(self) -> &'static str {
        let (_, info) = decode(self.into());
        info.name()
    }
    pub fn to_string(self) -> String {
        let (_, info) = decode(self.into());

        use Form::*;
        if let Some(form) = info.form() {
            let i: IType = self.into();
            let r: RType = self.into();

            let mut args = Vec::<String>::new();
            match form.dest(self) {
                Dest::Gpr(r) | Dest::Store(r) => {
                    args.push(MIPS_REG_NAMES[r as usize].to_owned());
                }
                Dest::Fpr(f) | Dest::StoreFpr(f) => {
                    args.push(format!("f{}", f));
                }
                Dest::None => {}
            }

            match form {
                RegRegImm(_) | BranchReg | RegImmBranch(_) | RegImmTrap(_)
                | RegImmTrapSigned(_) | JReg(_) | JRegLink(_) | MoveTo(_) => {
                    args.push(MIPS_REG_NAMES[i.rs() as usize].to_owned());
                }
                BranchRegReg | RegRegReg(_) | TrapRegReg(_) | MulDiv(_) | ShiftReg(_) => {
                    args.push(MIPS_REG_NAMES[r.rs() as usize].to_owned());
                    args.push(MIPS_REG_NAMES[r.rt() as usize].to_owned());
                }
                LoadBaseImm | StoreBaseImm | LoadFpuBaseImm | StoreFpuBaseImm => {
                    args.push(format!(
                        "0x{:#x}({})",
                        i.imm() as i16,
                        MIPS_REG_NAMES[i.rs() as usize]
                    ));
                }
                LoadUpper => {
                    args.push(format!("0x{:#x}", (i.imm() as u32) << 16));
                }
                ShiftImm(u8) => {
                    args.push(MIPS_REG_NAMES[r.rt() as usize].to_owned());
                    args.push(format!("{}", u8));
                }
                J26 => {
                    let j: JType = self.into();
                    args.push(format!("0x{:#x}", j.target() << 2));
                }
                ExceptionType(_) | MoveFrom(_) => {}
            }

            match form.imm_type() {
                Some(ImmType::Unsinged) => {
                    args.push(format!("{:#x}", i.imm()));
                }
                Some(ImmType::Signed) => {
                    args.push(format!("{:#x}", i.imm() as i16 as i32));
                }
                _ => {}
            }

            return format!("{:<7} {}", info.name(), args.join(", "));
        } else {
            return info.name().to_owned();
        }
    }
}

pub fn decode(inst: u32) -> (Instruction, &'static InstructionInfo) {
    let op = inst >> 26;
    let mut info = &PRIMARY_TABLE[op as usize];

    loop {
        match info {
            InstructionInfo::Special => {
                info = &SPECIAL_TABLE[(inst & 0x3f) as usize];
                continue;
            }
            InstructionInfo::RegImm => {
                info = &REGIMM_TABLE[((inst >> 16) & 0x1f) as usize];
                continue;
            }
            InstructionInfo::Op(_, _, form) => {
                return (form.to_instruction(inst), info);
            }
            InstructionInfo::Reserved => {
                return (Instruction::ReservedInstructionException(inst), info);
            }
            _ => {
                todo!("unimplemented");
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Form {
    J26,

    // IType
    RegRegImm(bool), // true == signed
    LoadUpper,       // subcase of RegRegImm
    BranchReg,
    BranchRegReg,
    LoadBaseImm,
    StoreBaseImm,
    LoadFpuBaseImm,
    StoreFpuBaseImm,
    RegImmBranch(u8),
    RegImmTrap(u8),
    RegImmTrapSigned(u8),

    // RType
    JReg(u8),
    JRegLink(u8),
    ShiftImm(u8),
    ShiftReg(u8),
    MoveFrom(u8),
    MoveTo(u8),
    MulDiv(u8),
    RegRegReg(u8),
    TrapRegReg(u8),
    ExceptionType(u8),
}

pub enum ImmType {
    Unsinged,
    Signed,
    PcOffset,
    Offset,
}

pub enum Dest {
    Gpr(u8),
    Fpr(u8),
    Store(u8),
    StoreFpr(u8),
    None,
}

impl Form {
    pub fn imm_type(&self) -> Option<ImmType> {
        use Form::*;
        match self {
            BranchReg | BranchRegReg | RegImmBranch(_) => Some(ImmType::PcOffset),
            LoadUpper | RegRegImm(false) | RegImmTrap(_) => Some(ImmType::Unsinged),
            RegRegImm(true) | RegImmTrapSigned(_) => Some(ImmType::Signed),
            LoadBaseImm | StoreBaseImm | LoadFpuBaseImm | StoreFpuBaseImm => Some(ImmType::Offset),
            _ => None,
        }
    }
    pub fn dest(&self, inst: Instruction) -> Dest {
        use Form::*;
        let i: IType = inst.into();
        let r: RType = inst.into();
        match self {
            RegRegImm(_) | LoadUpper | LoadBaseImm => Dest::Gpr(i.rt()),
            LoadFpuBaseImm => Dest::Fpr(i.rt()),
            StoreBaseImm => Dest::Store(i.rt()),
            StoreFpuBaseImm => Dest::StoreFpr(i.rt()),
            JRegLink(_) if r.rd() != 31 => Dest::Gpr(r.rd()),
            ShiftImm(_) | ShiftReg(_) | MoveFrom(_) | RegRegReg(_) => Dest::Gpr(r.rd()),
            _ => Dest::None,
        }
    }

    pub fn to_instruction(&self, inst: u32) -> Instruction {
        use Form::*;
        match self {
            J26 => {
                Instruction::J(JType::from_bytes(inst.to_le_bytes()))
            },
            RegRegImm(_) | LoadUpper | BranchReg | BranchRegReg | LoadBaseImm | StoreBaseImm
            | LoadFpuBaseImm | StoreFpuBaseImm | RegImmBranch(_) | RegImmTrap(_)
            | RegImmTrapSigned(_) => {
                Instruction::I(IType::from_bytes(inst.to_le_bytes()))
            },
            JReg(_) | JRegLink(_) | ShiftImm(_) | ShiftReg(_) | MoveFrom(_) | MoveTo(_)
            | MulDiv(_) | RegRegReg(_) | TrapRegReg(_) | ExceptionType(_) => {
                Instruction::R(RType::from_bytes(inst.to_le_bytes()))
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InstructionInfo {
    Reserved,
    Special,
    RegImm,
    Op(&'static str, u8, Form),
    Unimplemented(&'static str, u8),
    CopOp(),
}

impl InstructionInfo {
    pub fn name(&self) -> &'static str {
        match self {
            InstructionInfo::Reserved => "Reserved",
            InstructionInfo::Special => "Special",
            InstructionInfo::RegImm => "RegImm",
            InstructionInfo::Op(name, _, _) => name,
            InstructionInfo::CopOp() => "CopOp",
            InstructionInfo::Unimplemented(_, _) => todo!(),
        }
    }
    pub fn form(&self) -> Option<&Form> {
        match self {
            InstructionInfo::Reserved => None,
            InstructionInfo::Special => None,
            InstructionInfo::RegImm => None,
            InstructionInfo::Op(_, _, form) => Some(form),
            InstructionInfo::CopOp() => None,
            InstructionInfo::Unimplemented(_, _) => None,
        }
    }
}

const fn build_primary_table() -> [InstructionInfo; 64] {
    use InstructionInfo::*;

    // Almost everything in the primary table is IType.
    // The exceptions are the:
    //  - the two subtables. All RegImm are IType, all Special are RType
    //  - the COP ops, which are a mix of IType and RType
    //  = J and JAL which are JType.

    [
        Special,
        RegImm,
        Op("J", 0x2, Form::J26),
        Op("JAL", 0x3, Form::J26),
        Op("BEQ", 0x4, Form::BranchRegReg),
        Op("BNE", 0x5, Form::BranchRegReg),
        Op("BLEZ", 0x6, Form::BranchReg),
        Op("BGTZ", 0x7, Form::BranchReg),
        Op("ADDI", 0x8, Form::RegRegImm(true)),
        Op("ADDIU", 0x9, Form::RegRegImm(false)),
        Op("SLTI", 0xa, Form::RegRegImm(true)),
        Op("SLTIU", 0xb, Form::RegRegImm(false)),
        Op("ANDI", 0xc, Form::RegRegImm(false)),
        Op("ORI", 0xd, Form::RegRegImm(false)),
        Op("XORI", 0xe, Form::RegRegImm(false)),
        Op("LUI", 0xf, Form::LoadUpper),
        Unimplemented("COP0", 0x10),
        Unimplemented("COP1", 0x11),
        Unimplemented("COP2", 0x12),
        Reserved,
        Op("BEQL", 0x14, Form::BranchRegReg),
        Op("BNEL", 0x15, Form::BranchRegReg),
        Op("BLEZL", 0x16, Form::BranchReg),
        Op("BGTZL", 0x17, Form::BranchReg),
        Op("DADDI", 0x18, Form::RegRegImm(false)),
        Op("DADDIU", 0x19, Form::RegRegImm(false)),
        Op("LDL", 0x1a, Form::LoadBaseImm),
        Op("LDR", 0x1b, Form::LoadBaseImm),
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        Op("LB", 0x20, Form::LoadBaseImm),
        Op("LH", 0x21, Form::LoadBaseImm),
        Op("LWL", 0x22, Form::LoadBaseImm),
        Op("LW", 0x23, Form::LoadBaseImm),
        Op("LBU", 0x24, Form::LoadBaseImm),
        Op("LHU", 0x25, Form::LoadBaseImm),
        Op("LWR", 0x26, Form::LoadBaseImm),
        Op("LWU", 0x27, Form::LoadBaseImm),
        Op("SB", 0x28, Form::StoreBaseImm),
        Op("SH", 0x29, Form::StoreBaseImm),
        Op("SWL", 0x2a, Form::StoreBaseImm),
        Op("SW", 0x2b, Form::StoreBaseImm),
        Op("SDL", 0x2c, Form::StoreBaseImm),
        Op("SDR", 0x2d, Form::StoreBaseImm),
        Op("SWR", 0x2e, Form::StoreBaseImm),
        Op("CACHE", 0x2f, Form::StoreBaseImm),
        Op("LL", 0x30, Form::LoadBaseImm),
        Op("LWC1", 0x31, Form::LoadFpuBaseImm),
        Unimplemented("COP2", 0x32),
        Reserved,
        Op("LLD", 0x34, Form::LoadBaseImm),
        Op("LDC1", 0x35, Form::LoadFpuBaseImm),
        Unimplemented("COP2", 0x32),
        Op("LD", 0x37, Form::LoadBaseImm),
        Op("SC", 0x38, Form::StoreBaseImm),
        Op("SWC1", 0x39, Form::StoreFpuBaseImm),
        Unimplemented("COP2", 0x3a),
        Reserved,
        Op("SCD", 0x3c, Form::StoreBaseImm),
        Op("SDC1", 0x3d, Form::StoreBaseImm),
        Unimplemented("COP2", 0x3e),
        Op("SD", 0x3f, Form::StoreBaseImm),
    ]
}

const fn build_special_table() -> [InstructionInfo; 64] {
    use InstructionInfo::*;

    [
        // 0
        Op("SLL", 0, Form::ShiftImm(0x0)),
        Reserved,
        Op("SRL", 0, Form::ShiftImm(0x2)),
        Op("SRA", 0, Form::ShiftImm(0x3)),
        Op("SLLV", 0, Form::ShiftReg(0x4)),
        Reserved,
        Op("SRLV", 0, Form::ShiftReg(0x6)),
        Op("SRAV", 0, Form::ShiftReg(0x7)),
        // 1
        Op("JR", 0, Form::JReg(0x8)),
        Op("JALR", 0, Form::JRegLink(0x9)),
        Reserved,
        Reserved,
        Op("SYSCALL", 0, Form::ExceptionType(0xc)),
        Op("BREAK", 0, Form::ExceptionType(0xd)),
        Reserved,
        Op("SYNC", 0, Form::ExceptionType(0xf)),
        // 2
        Op("MFHI", 0, Form::MoveFrom(0x10)),
        Op("MTHI", 0, Form::MoveTo(0x11)),
        Op("MFLO", 0, Form::MoveFrom(0x12)),
        Op("MTLO", 0, Form::MoveTo(0x13)),
        Op("DSLLV", 0, Form::ShiftReg(0x14)),
        Reserved,
        Op("DSRLV", 0, Form::ShiftReg(0x16)),
        Op("DSRAV", 0, Form::ShiftReg(0x17)),
        // 3
        Op("MULT", 0, Form::MulDiv(0x18)),
        Op("MULTU", 0, Form::MulDiv(0x19)),
        Op("DIV", 0, Form::MulDiv(0x1a)),
        Op("DIVU", 0, Form::MulDiv(0x1b)),
        Op("DMULT", 0, Form::MulDiv(0x1c)),
        Op("DMULTU", 0, Form::MulDiv(0x1d)),
        Op("DDIV", 0, Form::MulDiv(0x1e)),
        Op("DDIVU", 0, Form::MulDiv(0x1f)),
        // 4
        Op("ADD", 0, Form::RegRegReg(0x20)),
        Op("ADDU", 0, Form::RegRegReg(0x21)),
        Op("SUB", 0, Form::RegRegReg(0x22)),
        Op("SUBU", 0, Form::RegRegReg(0x23)),
        Op("AND", 0, Form::RegRegReg(0x24)),
        Op("OR", 0, Form::RegRegReg(0x25)),
        Op("XOR", 0, Form::RegRegReg(0x26)),
        Op("NOR", 0, Form::RegRegReg(0x27)),
        // 5
        Reserved,
        Reserved,
        Op("SLT", 0, Form::RegRegReg(0x2a)),
        Op("SLTU", 0, Form::RegRegReg(0x2b)),
        Op("DADD", 0, Form::RegRegReg(0x2c)),
        Op("DADDU", 0, Form::RegRegReg(0x2d)),
        Op("DSUB", 0, Form::RegRegReg(0x2e)),
        Op("DSUBU", 0, Form::RegRegReg(0x2f)),
        // 6
        Op("TGE", 0, Form::TrapRegReg(0x30)),
        Op("TGEU", 0, Form::TrapRegReg(0x31)),
        Op("TLT", 02, Form::TrapRegReg(0x32)),
        Op("TLTU", 0, Form::TrapRegReg(0x33)),
        Op("TEQ", 0, Form::TrapRegReg(0x34)),
        Reserved,
        Op("TNE", 0, Form::TrapRegReg(0x36)),
        Reserved,
        // 7
        Op("DSLL", 0, Form::ShiftImm(0x38)),
        Reserved,
        Op("DSRL", 0, Form::ShiftImm(0x3a)),
        Op("DSRA", 0, Form::ShiftImm(0x3b)),
        Op("DSLL32", 0, Form::ShiftImm(0x3c)),
        Reserved,
        Op("DSRL32", 0, Form::ShiftImm(0x3e)),
        Op("DSRA32", 0, Form::ShiftImm(0x3f)),
    ]
}

const fn build_regimm_table() -> [InstructionInfo; 32] {
    use InstructionInfo::*;

    [
        Op("BLTZ", 1, Form::RegImmBranch(0x0)),
        Op("BGEZ", 1, Form::RegImmBranch(0x1)),
        Op("BLTZL", 1, Form::RegImmBranch(0x2)),
        Op("BGEZL", 1, Form::RegImmBranch(0x3)),
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        Op("TGEI", 1, Form::RegImmTrapSigned(0x8)),
        Op("TGEIU", 1, Form::RegImmTrap(0x9)),
        Op("TLTI", 1, Form::RegImmTrapSigned(0xa)),
        Op("TLTIU", 1, Form::RegImmTrap(0xb)),
        Op("TEQI", 1, Form::RegImmTrapSigned(0xc)),
        Reserved,
        Op("TNEI", 1, Form::RegImmTrapSigned(0xe)),
        Reserved,
        Op("BLTZAL", 1, Form::RegImmBranch(0x10)),
        Op("BGEZAL", 1, Form::RegImmBranch(0x11)),
        Op("BLTZALL", 1, Form::RegImmBranch(0x12)),
        Op("BGEZALL", 1, Form::RegImmBranch(0x13)),
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        Reserved,
    ]
}

const PRIMARY_TABLE: [InstructionInfo; 64] = build_primary_table();
const SPECIAL_TABLE: [InstructionInfo; 64] = build_special_table();
const REGIMM_TABLE: [InstructionInfo; 32] = build_regimm_table();
