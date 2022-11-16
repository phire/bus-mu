use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

#[bitfield(bits = 32)]
#[derive(BitfieldSpecifier, Debug, Copy, Clone)]
pub struct IType {
    pub imm: B16,
    pub rt: B5,
    pub rs: B5,
    op: B6,
}

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct JType {
    pub target: B26,
    op: B6,
}

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct RType {
    funct: B6,
    pub sa: B5,
    pub rd: B5,
    pub rt: B5,
    pub rs: B5,
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
                RegImm(_) | BranchReg | RegImmBranch(_) | RegImmTrap(_)
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
            InstructionInfo::Op(_, _, form, _, _) => {
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
    RegImm(bool), // true == signed
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
            LoadUpper | RegImm(false) | RegImmTrap(_) => Some(ImmType::Unsinged),
            RegImm(true) | RegImmTrapSigned(_) => Some(ImmType::Signed),
            LoadBaseImm | StoreBaseImm | LoadFpuBaseImm | StoreFpuBaseImm => Some(ImmType::Offset),
            _ => None,
        }
    }
    pub fn dest(&self, inst: Instruction) -> Dest {
        use Form::*;
        let i: IType = inst.into();
        let r: RType = inst.into();
        match self {
            RegImm(_) | LoadUpper | LoadBaseImm => Dest::Gpr(i.rt()),
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
            RegImm(_) | LoadUpper | BranchReg | BranchRegReg | LoadBaseImm | StoreBaseImm
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
    Op(&'static str, u8, Form, RfMode, ExMode),
    Unimplemented(&'static str, u8),
    CopOp(),
}

impl InstructionInfo {
    pub fn name(&self) -> &'static str {
        match self {
            InstructionInfo::Reserved => "Reserved",
            InstructionInfo::Special => "Special",
            InstructionInfo::RegImm => "RegImm",
            InstructionInfo::Op(name, _, _, _, _) => name,
            InstructionInfo::CopOp() => "CopOp",
            InstructionInfo::Unimplemented(_, _) => todo!(),
        }
    }
    pub fn form(&self) -> Option<&Form> {
        match self {
            InstructionInfo::Reserved => None,
            InstructionInfo::Special => None,
            InstructionInfo::RegImm => None,
            InstructionInfo::Op(_, _, form, _, _) => Some(form),
            InstructionInfo::CopOp() => None,
            InstructionInfo::Unimplemented(_, _) => None,
        }
    }
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
    StoreOp,
    RegReg,
    MulDiv,
    ShiftImm,
    ShiftImm32,
}

#[derive(Debug, Clone, Copy)]
pub enum ExMode {
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
    Mem(u8),
    MemUnsigned(u8),
    MemLeft(u8),
    MemRight(u8),
    MemLinked(u8),
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


const fn build_primary_table() -> [InstructionInfo; 64] {
    use InstructionInfo::*;
    use RfMode::*;
    use ExMode::*;
    use CmpMode::*;

    // Almost everything in the primary table is IType.
    // The exceptions are the:
    //  - the two subtables. All RegImm are IType, all Special are RType
    //  - the COP ops, which are a mix of IType and RType
    //  = J and JAL which are JType.

    [
        Special,
        RegImm,
        Op("J", 0x2, Form::J26, JumpImm, Jump),
        Op("JAL", 0x3, Form::J26, JumpImmLink, Jump),
        Op("BEQ", 0x4, Form::BranchRegReg, BranchImm2, Branch(Eq)),
        Op("BNE", 0x5, Form::BranchRegReg,  BranchImm2, Branch(Ne)),
        Op("BLEZ", 0x6, Form::BranchReg, BranchImm1, Branch(Le)),
        Op("BGTZ", 0x7, Form::BranchReg, BranchImm1, Branch(Gt)),
        // 1
        Op("ADDI", 0x8, Form::RegImm(true), ImmSigned, Add32),
        Op("ADDIU", 0x9, Form::RegImm(true), ImmSigned, AddU32),
        Op("SLTI", 0xa, Form::RegImm(true), ImmSigned, SetLess),
        Op("SLTIU", 0xb, Form::RegImm(true), ImmSigned, SetLessU),
        Op("ANDI", 0xc, Form::RegImm(false), ImmUnsigned, And),
        Op("ORI", 0xd, Form::RegImm(false), ImmUnsigned, Or),
        Op("XORI", 0xe, Form::RegImm(false), ImmUnsigned, Xor),
        Op("LUI", 0xf, Form::LoadUpper, ImmUnsigned, InsertUpper),
        // 2
        Unimplemented("COP0", 0x10),
        Unimplemented("COP1", 0x11),
        Unimplemented("COP2", 0x12),
        Reserved,
        Op("BEQL", 0x14, Form::BranchRegReg, BranchImm2, BranchLikely(Eq)),
        Op("BNEL", 0x15, Form::BranchRegReg, BranchImm2, BranchLikely(Ne)),
        Op("BLEZL", 0x16, Form::BranchReg, BranchImm1, BranchLikely(Le)),
        Op("BGTZL", 0x17, Form::BranchReg, BranchImm1, BranchLikely(Gt)),
        // 3
        Op("DADDI", 0x18, Form::RegImm(true), ImmSigned, Add64),
        Op("DADDIU", 0x19, Form::RegImm(true), ImmSigned, AddU64),
        Op("LDL", 0x1a, Form::LoadBaseImm, ImmSigned, MemLeft(8)),
        Op("LDR", 0x1b, Form::LoadBaseImm, ImmSigned, MemRight(8)),
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        // 4
        Op("LB", 0x20, Form::LoadBaseImm, ImmSigned, Mem(1)),
        Op("LH", 0x21, Form::LoadBaseImm, ImmSigned, Mem(2)),
        Op("LWL", 0x22, Form::LoadBaseImm, ImmSigned, MemLeft(4)),
        Op("LW", 0x23, Form::LoadBaseImm, ImmSigned, Mem(4)),
        Op("LBU", 0x24, Form::LoadBaseImm, ImmSigned, MemUnsigned(1)),
        Op("LHU", 0x25, Form::LoadBaseImm, ImmSigned, MemUnsigned(2)),
        Op("LWR", 0x26, Form::LoadBaseImm, ImmSigned, MemRight(4)),
        Op("LWU", 0x27, Form::LoadBaseImm, ImmSigned, MemUnsigned(4)),
        // 5
        Op("SB", 0x28, Form::StoreBaseImm, StoreOp, Mem(1)),
        Op("SH", 0x29, Form::StoreBaseImm, StoreOp, Mem(2)),
        Op("SWL", 0x2a, Form::StoreBaseImm, StoreOp, MemLeft(4)),
        Op("SW", 0x2b, Form::StoreBaseImm, StoreOp, Mem(4)),
        Op("SDL", 0x2c, Form::StoreBaseImm, StoreOp, MemLeft(8)),
        Op("SDR", 0x2d, Form::StoreBaseImm, StoreOp, MemRight(8)),
        Op("SWR", 0x2e, Form::StoreBaseImm, StoreOp, MemRight(4)),
        Unimplemented("Cache", 0x2f),
        // 6
        Op("LL", 0x30, Form::LoadBaseImm, ImmSigned, MemLinked(4)),
        Unimplemented("LWC1", 0x31),// Form::LoadFpuBaseImm, 0),
        Unimplemented("COP2", 0x32),
        Reserved,
        Op("LLD", 0x34, Form::LoadBaseImm, ImmSigned, MemLinked(8)),
        Unimplemented("LDC1", 0x35), // Form::LoadFpuBaseImm, 0),
        Unimplemented("COP2", 0x32),
        Op("LD", 0x37, Form::LoadBaseImm, ImmSigned, Mem(8)),
        // 7
        Op("SC", 0x38, Form::StoreBaseImm, StoreOp, MemLinked(4)),
        Unimplemented("SWC1", 0x39), //Form::StoreFpuBaseImm, 0),
        Unimplemented("COP2", 0x3a),
        Reserved,
        Op("SCD", 0x3c, Form::StoreBaseImm, StoreOp, MemLinked(8)),
        Unimplemented("SDC1", 0x3d), //Form::StoreBaseImm, 0),
        Unimplemented("COP2", 0x3e),
        Op("SD", 0x3f, Form::StoreBaseImm, StoreOp, Mem(8)),
    ]
}

const fn build_special_table() -> [InstructionInfo; 64] {
    use InstructionInfo::*;
    use RfMode::*;
    use ExMode::*;

    [
        // 0
        Op("SLL", 0, Form::ShiftImm(0x0), ShiftImm, ShiftLeft32),
        Reserved,
        Op("SRL", 0, Form::ShiftImm(0x2), ShiftImm, ShiftRight32),
        Op("SRA", 0, Form::ShiftImm(0x3), ShiftImm, ShiftRightArith32),
        Op("SLLV", 0, Form::ShiftReg(0x4), RegReg, ShiftLeft32),
        Reserved,
        Op("SRLV", 0, Form::ShiftReg(0x6), RegReg, ShiftRight32),
        Op("SRAV", 0, Form::ShiftReg(0x7), RegReg, ShiftRightArith32),
        // 1
        Op("JR", 0, Form::JReg(0x8), JumpReg, Jump),
        Op("JALR", 0, Form::JRegLink(0x9), JumpRegLink, Jump),
        Reserved,
        Reserved,
        Unimplemented("SYSCALL", 0), // Form::ExceptionType(0xc), 0),
        Unimplemented("BREAK", 0), // Form::ExceptionType(0xd), 0),
        Reserved,
        Unimplemented("SYNC", 0), // Form::ExceptionType(0xf), 0),
        // 2
        Unimplemented("MFHI", 0), // Form::MoveFrom(0x10), 0),
        Unimplemented("MTHI", 0), // Form::MoveTo(0x11), 0),
        Unimplemented("MFLO", 0), // Form::MoveFrom(0x12), 0),
        Unimplemented("MTLO", 0), // Form::MoveTo(0x13), 0),
        Op("DSLLV", 0, Form::ShiftReg(0x14), RegReg, ShiftLeft64),
        Reserved,
        Op("DSRLV", 0, Form::ShiftReg(0x16), RegReg, ShiftRight64),
        Op("DSRAV", 0, Form::ShiftReg(0x17), RegReg, ShiftRightArith64),
        // 3
        Op("MULT", 0, Form::MulDiv(0x18), MulDiv, Mul32),
        Op("MULTU", 0, Form::MulDiv(0x19), MulDiv, MulU32),
        Op("DIV", 0, Form::MulDiv(0x1a), MulDiv, Div32),
        Op("DIVU", 0, Form::MulDiv(0x1b), MulDiv, DivU32),
        Op("DMULT", 0, Form::MulDiv(0x1c), MulDiv, Mul64),
        Op("DMULTU", 0, Form::MulDiv(0x1d), MulDiv, MulU64),
        Op("DDIV", 0, Form::MulDiv(0x1e),  MulDiv, Div64),
        Op("DDIVU", 0, Form::MulDiv(0x1f), MulDiv, DivU64),
        // 4
        Op("ADD", 0, Form::RegRegReg(0x20), RegReg, Add32),
        Op("ADDU", 0, Form::RegRegReg(0x21), RegReg, AddU32),
        Op("SUB", 0, Form::RegRegReg(0x22), RegReg, Sub32),
        Op("SUBU", 0, Form::RegRegReg(0x23), RegReg, SubU32),
        Op("AND", 0, Form::RegRegReg(0x24), RegReg, And),
        Op("OR", 0, Form::RegRegReg(0x25), RegReg, Or),
        Op("XOR", 0, Form::RegRegReg(0x26), RegReg, Xor),
        Op("NOR", 0, Form::RegRegReg(0x27), RegReg, Nor),
        // 5
        Reserved,
        Reserved,
        Op("SLT", 0, Form::RegRegReg(0x2a), RegReg, SetLess),
        Op("SLTU", 0, Form::RegRegReg(0x2b), RegReg, SetLessU),
        Op("DADD", 0, Form::RegRegReg(0x2c), RegReg, Add64),
        Op("DADDU", 0, Form::RegRegReg(0x2d), RegReg, AddU64),
        Op("DSUB", 0, Form::RegRegReg(0x2e), RegReg, Sub64),
        Op("DSUBU", 0, Form::RegRegReg(0x2f), RegReg, SubU64),
        // 6
        //Op("TGE", 0, Form::TrapRegReg(0x30), 0),
        Unimplemented("TGE", 0), // Form::TrapRegReg(0x30), 0),
        Unimplemented("TGEU", 0), // Form::TrapRegReg(0x31), 0),
        Unimplemented("TLT", 0), // Form::TrapRegReg(0x32), 0),
        Unimplemented("TLTU", 0), // Form::TrapRegReg(0x33), 0),
        Unimplemented("TEQ", 0), // Form::TrapRegReg(0x34), 0),
        Reserved,
        Unimplemented("TNE", 0), // Form::TrapRegReg(0x36), 0),
        Reserved,
        // 7
        Op("DSLL", 0, Form::ShiftImm(0x38), ShiftImm, ShiftLeft64),
        Reserved,
        Op("DSRL", 0, Form::ShiftImm(0x3a), ShiftImm, ShiftRight64),
        Op("DSRA", 0, Form::ShiftImm(0x3b), ShiftImm, ShiftRightArith64),
        Op("DSLL32", 0, Form::ShiftImm(0x3c), ShiftImm32, ShiftLeft64),
        Reserved,
        Op("DSRL32", 0, Form::ShiftImm(0x3e), ShiftImm32, ShiftRight64),
        Op("DSRA32", 0, Form::ShiftImm(0x3f), ShiftImm32, ShiftRightArith64),
    ]
}

const fn build_regimm_table() -> [InstructionInfo; 32] {
    use InstructionInfo::*;
    use RfMode::*;
    use ExMode::*;
    use CmpMode::*;

    [
        Op("BLTZ", 1, Form::RegImmBranch(0x0), BranchImm1, Branch(Lt)),
        Op("BGEZ", 1, Form::RegImmBranch(0x1), BranchImm1, Branch(Ge)),
        Op("BLTZL", 1, Form::RegImmBranch(0x2), BranchImm1, BranchLikely(Lt)),
        Op("BGEZL", 1, Form::RegImmBranch(0x3), BranchImm1, BranchLikely(Ge)),
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        Unimplemented("TGEI", 1), // Form::RegImmTrapSigned(0x8), 0),
        Unimplemented("TGEIU", 1), // Form::RegImmTrapUnsigned(0x9), 0),
        Unimplemented("TLTI", 1), // Form::RegImmTrapSigned(0xa), 0),
        Unimplemented("TLTIU", 1), // Form::RegImmTrapUnsigned(0xb), 0),
        Unimplemented("TEQI", 1), // Form::RegImmTrapSigned(0xc), 0),
        Reserved,
        Unimplemented("TNEI", 1), // Form::RegImmTrapSigned(0xe), 0),
        Reserved,
        Op("BLTZAL", 1, Form::RegImmBranch(0x10), BranchLinkImm, Branch(Lt)),
        Op("BGEZAL", 1, Form::RegImmBranch(0x11), BranchLinkImm, Branch(Ge)),
        Op("BLTZALL", 1, Form::RegImmBranch(0x12), BranchLinkImm, BranchLikely(Lt)),
        Op("BGEZALL", 1, Form::RegImmBranch(0x13), BranchLinkImm, BranchLikely(Ge)),
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
