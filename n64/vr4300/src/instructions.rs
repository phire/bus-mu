use modular_bitfield::{bitfield, specifiers::*};
use super::coprocessor0::COP0_REG_NAMES;

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct IType {
    pub imm: B16,
    pub rt: B5,
    pub rs: B5,
    #[skip]
    op: B6,
}

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct JType {
    pub target: B26,
    #[skip]
    op: B6,
}

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct RType {
    pub funct: B6,
    pub sa: B5,
    pub rd: B5,
    pub rt: B5,
    pub rs: B5,
    pub op: B6,
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

impl Into<IType> for RType {
    fn into(self) -> IType {
        IType::from_bytes(self.into_bytes())
    }
}

impl Into<JType> for RType {
    fn into(self) -> JType {
        JType::from_bytes(self.into_bytes())
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Instruction {
    I(IType),
    J(JType),
    R(RType),
    ReservedInstructionException(u32),
    Unimplemented(u32),
}

impl Into<u32> for Instruction {
    fn into(self) -> u32 {
        match self {
            Instruction::I(i) => i.into(),
            Instruction::J(j) => j.into(),
            Instruction::R(r) => r.into(),
            Instruction::ReservedInstructionException(num) => num,
            Instruction::Unimplemented(num) => num,
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
            Instruction::Unimplemented(num) => num.to_ne_bytes(),
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

pub const MIPS_REG_NAMES: [&'static str; 32] = [
    "$zero", // Always 0
    "$at",   // r1 - Reserved for assembler
    "$v0", "$v1", // r2-r3 - Function return values
    "$a0", "$a1", "$a2", "$a3", // r4-r7 - function arguments
    "$t0", "$t1", "$t2", "$t3", "$t4", "$t5", "$t6",
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

    /// Provides a string representation of the instruction (as disassembly)
    pub fn disassemble(self, address:u64) -> String {
        let (_, info) = decode(self.into());

        use Form::*;
        if let Some(form) = info.form() {
            let i: IType = self.into();
            let r: RType = self.into();

            // We will collect the arguments into this vector
            let mut args = Vec::<String>::new();

            // Handles most instructions that do normal things with the destination register
            match form.dest(self) {
                Dest::Gpr(reg) | Dest::Store(reg) => {
                    args.push(MIPS_REG_NAMES[reg as usize].to_owned());
                }
                Dest::Fpr(f) | Dest::StoreFpr(f) => {
                    args.push(format!("f{}", f));
                }
                Dest::CacheOp(op) => {
                    args.push(format!("{:02x}", op));
                }
                Dest::None => {}
            }

            // Handle the various special cases
            match form {
                // One Reg
                RegImm(_) | BranchReg | RegImmBranch(_) | RegImmTrap(_)
                | RegImmTrapSigned(_) | JReg(_) | JRegLink(_) | MoveTo(_) => {
                    args.push(MIPS_REG_NAMES[i.rs() as usize].to_owned());
                }
                // Two Regs
                BranchRegReg | RegRegReg(_) | TrapRegReg(_) | MulDiv(_) | ShiftReg(_) => {
                    args.push(MIPS_REG_NAMES[r.rs() as usize].to_owned());
                    args.push(MIPS_REG_NAMES[r.rt() as usize].to_owned());
                }
                // Cop single-register
                CopReg => {
                    args.push(MIPS_REG_NAMES[r.rt() as usize].to_owned());
                    args.push(COP0_REG_NAMES[r.rd() as usize].to_owned());
                }
                // Load/Stores
                LoadBaseImm | StoreBaseImm | LoadFpuBaseImm | StoreFpuBaseImm | Cache => {
                    args.push(format!(
                        "0x{:#x}({})",
                        i.imm() as i16,
                        MIPS_REG_NAMES[i.rs() as usize]
                    ));
                }
                // LUI (Load Upper Immediate) is a special case
                LoadUpper => {
                    let imm = (i.imm() as u32) << 16;
                    args.push(format!("{:#x}", imm as i32));
                }
                // Shift by immediate
                ShiftImm(_) => {
                    args.push(MIPS_REG_NAMES[r.rt() as usize].to_owned());
                    args.push(format!("{}", r.rs())); // the RS field gets used an immediate
                }
                // Absolute jumps
                J26 => {
                    let j: JType = self.into();
                    let target = (address & !0x0fff_ffff) | ((j.target() as u64) << 2);
                    args.push(format!("{:#x}", target));
                }
                // These don't have any arguments
                ExceptionType(_) | MoveFrom(_) => {}
            }

            // handle basic immediate formats
            match form.imm_type() {
                Some(ImmType::Unsinged) => {
                    args.push(format!("{:#x}", i.imm()));
                }
                Some(ImmType::Signed) => {
                    args.push(format!("{:#x}", i.imm() as i16 as i32));
                }
                Some(ImmType::PcOffset) => {
                    let offset = (i.imm() as i16 as i64) << 2;
                    let target = address as i64 + offset + 4;
                    args.push(format!("{:010x}", target as u64));
                }
                // Other immediate formats are handled above, as a premature optimization
                _ => {}
            }

            // Finally, join the arguments together and build the final string
            return format!("{:<7} {}", info.name(), args.join(", "));
        } else {
            return info.name().to_owned();
        }
    }
}

pub fn decode(inst_word: u32) -> (Instruction, &'static InstructionInfo) {
    // we pre-decode to R-Type, as it's the only type decode logic uses
    let inst = RType::from_bytes(inst_word.to_le_bytes());

    let mut info = &PRIMARY_TABLE[inst.op() as usize];
    loop {
        match info {
            InstructionInfo::Special => {
                info = &SPECIAL_TABLE[inst.funct() as usize];
                continue;
            }
            InstructionInfo::RegImm => {
                info = &REGIMM_TABLE[inst.rt() as usize];
                continue;
            }
            InstructionInfo::CopOp(0) => {
                if inst.rs() < 16 {
                    info = &COP0_TABLE[inst.rs() as usize];
                } else {
                    info = &COP0_FN_TABLE[inst.funct() as usize];
                }
                continue;
            }
            InstructionInfo::CopOp(n) => {
                unimplemented!("COP{} not implemented - {:08x}", n, inst_word);
            }
            InstructionInfo::Op(_, _, form, _, _) => {
                return (form.to_instruction(inst), info);
            }
            InstructionInfo::Reserved => {
                return (Instruction::ReservedInstructionException(inst.into()), info);
            }
            _ => {
                return (Instruction::Unimplemented(inst.into()), info);
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
    Cache,

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

    // Coprocessor
    CopReg,
}

pub enum ImmType {
    Unsinged,
    Signed,
    SignedUpper,
    PcOffset,
    Offset,
}

pub enum Dest {
    Gpr(u8),
    Fpr(u8),
    Store(u8),
    StoreFpr(u8),
    CacheOp(u8),
    None,
}

impl Form {
    pub fn imm_type(&self) -> Option<ImmType> {
        use Form::*;
        match self {
            BranchReg | BranchRegReg | RegImmBranch(_) => Some(ImmType::PcOffset),
            RegImm(false) | RegImmTrap(_) => Some(ImmType::Unsinged),
            RegImm(true) | RegImmTrapSigned(_) => Some(ImmType::Signed),
            LoadUpper => Some(ImmType::SignedUpper),
            LoadBaseImm | StoreBaseImm | LoadFpuBaseImm | StoreFpuBaseImm | Cache => Some(ImmType::Offset),
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
            Cache => Dest::CacheOp(r.rt()),
            _ => Dest::None,
        }
    }

    pub fn to_instruction(&self, inst: RType) -> Instruction {
        use Form::*;
        match self {
            J26 => {
                Instruction::J(inst.into())
            },
            RegImm(_) | LoadUpper | BranchReg | BranchRegReg | LoadBaseImm | StoreBaseImm
            | LoadFpuBaseImm | StoreFpuBaseImm | RegImmBranch(_) | RegImmTrap(_)
            | RegImmTrapSigned(_) | Cache => {
                Instruction::I(inst.into())
            },
            JReg(_) | JRegLink(_) | ShiftImm(_) | ShiftReg(_) | MoveFrom(_) | MoveTo(_)
            | MulDiv(_) | RegRegReg(_) | TrapRegReg(_) | ExceptionType(_) | CopReg => {
                Instruction::R(inst)
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
    CopOp(u8),
}

impl InstructionInfo {
    pub fn name(&self) -> &'static str {
        match self {
            InstructionInfo::Reserved => "Reserved",
            InstructionInfo::Special => "Special",
            InstructionInfo::RegImm => "RegImm",
            InstructionInfo::Op(name, _, _, _, _) => name,
            InstructionInfo::CopOp(_) => "CopOp",
            InstructionInfo::Unimplemented(name, _) => name,
        }
    }
    pub fn form(&self) -> Option<&Form> {
        match self {
            InstructionInfo::Reserved => None,
            InstructionInfo::Special => None,
            InstructionInfo::RegImm => None,
            InstructionInfo::Op(_, _, form, _, _) => Some(form),
            InstructionInfo::CopOp(_) => None,
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
    RegRegNoWrite,
    SmallImm,
    SmallImmOffset32,
    SmallImmNoWrite,
    RfUnimplemented,
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
    Mem(u8),
    MemUnsigned(u8),
    MemLeft(u8),
    MemRight(u8),
    MemLinked(u8),
    LoadInternal(InternalReg),
    StoreInternal(InternalReg),
    CacheOp,
    ExUnimplemented,
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

#[derive(Debug, Clone, Copy)]
pub enum InternalReg {
    HI = 0,
    LO = 1,
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
        CopOp(0),
        CopOp(1),
        CopOp(2),
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
        Op("CACHE", 0x2f, Form::Cache, ImmSigned, CacheOp),
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
        Op("SLL", 0, Form::ShiftImm(0x0), SmallImm, ShiftLeft32),
        Reserved,
        Op("SRL", 0, Form::ShiftImm(0x2), SmallImm, ShiftRight32),
        Op("SRA", 0, Form::ShiftImm(0x3), SmallImm, ShiftRightArith32),
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
        // HWTEST: I'm not sure what should happen in RfMode for MFHI/MFLO. Almost every other i
        //         instruction does at least one register load (causing a false hazard, which is visible)
        Op("MFHI", 0, Form::MoveFrom(0x10), ImmUnsigned, LoadInternal(InternalReg::HI)),
        // HWTEST: Same question with RfMode for MTHI/MTLO
        Op("MTHI", 0, Form::MoveTo(0x11), RegRegNoWrite, StoreInternal(InternalReg::HI)),
        Op("MFLO", 0, Form::MoveFrom(0x12), ImmUnsigned, LoadInternal(InternalReg::LO)),
        Op("MTLO", 0, Form::MoveTo(0x13), RegRegNoWrite, StoreInternal(InternalReg::LO)),
        Op("DSLLV", 0, Form::ShiftReg(0x14), RegReg, ShiftLeft64),
        Reserved,
        Op("DSRLV", 0, Form::ShiftReg(0x16), RegReg, ShiftRight64),
        Op("DSRAV", 0, Form::ShiftReg(0x17), RegReg, ShiftRightArith64),
        // 3
        Op("MULT", 0, Form::MulDiv(0x18), RegRegNoWrite, Mul32),
        Op("MULTU", 0, Form::MulDiv(0x19), RegRegNoWrite, MulU32),
        Op("DIV", 0, Form::MulDiv(0x1a), RegRegNoWrite, Div32),
        Op("DIVU", 0, Form::MulDiv(0x1b), RegRegNoWrite, DivU32),
        Op("DMULT", 0, Form::MulDiv(0x1c), RegRegNoWrite, Mul64),
        Op("DMULTU", 0, Form::MulDiv(0x1d), RegRegNoWrite, MulU64),
        Op("DDIV", 0, Form::MulDiv(0x1e),  RegRegNoWrite, Div64),
        Op("DDIVU", 0, Form::MulDiv(0x1f), RegRegNoWrite, DivU64),
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
        Op("DSLL", 0, Form::ShiftImm(0x38), SmallImm, ShiftLeft64),
        Reserved,
        Op("DSRL", 0, Form::ShiftImm(0x3a), SmallImm, ShiftRight64),
        Op("DSRA", 0, Form::ShiftImm(0x3b), SmallImm, ShiftRightArith64),
        Op("DSLL32", 0, Form::ShiftImm(0x3c), SmallImmOffset32, ShiftLeft64),
        Reserved,
        Op("DSRL32", 0, Form::ShiftImm(0x3e), SmallImmOffset32, ShiftRight64),
        Op("DSRA32", 0, Form::ShiftImm(0x3f), SmallImmOffset32, ShiftRightArith64),
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

const fn build_cop0_table() -> [InstructionInfo; 16] {
    use InstructionInfo::*;

    [
        // 0
        Unimplemented("MFC0", 0x0),
        Unimplemented("DMFC0", 0x1),
        Unimplemented("CFC0", 0x2),
        Reserved,
        Op("MTC0", 0x4, Form::CopReg, RfMode::SmallImmNoWrite, ExMode::ExUnimplemented),
        Unimplemented("DMTC0", 0x5),
        Unimplemented("CTC0", 0x6),
        Reserved,
        // 8
        Unimplemented("BCC0", 0x8),
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        Reserved,
        Reserved,
    ]
}

const fn build_cop0_fn_table() -> [InstructionInfo; 64] {
    use InstructionInfo::*;

    let mut table = [Reserved; 64];
    table[0x1] = Unimplemented("TLBR", 1);
    table[0x2] = Unimplemented("TLBWI", 2);
    table[0x6] = Unimplemented("TLBWR", 6);
    table[0x8] = Unimplemented("TLBP", 8);
    table[0x18] = Unimplemented("ERET", 8);

    return table;
}

const PRIMARY_TABLE: [InstructionInfo; 64] = build_primary_table();
const SPECIAL_TABLE: [InstructionInfo; 64] = build_special_table();
const REGIMM_TABLE: [InstructionInfo; 32] = build_regimm_table();
const COP0_TABLE: [InstructionInfo; 16] = build_cop0_table();
const COP0_FN_TABLE: [InstructionInfo; 64] = build_cop0_fn_table();
