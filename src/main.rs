

struct CacheTag(u32);
impl CacheTag {

    #[inline]
    pub fn empty() -> CacheTag{ CacheTag(0) }
    pub fn new(tag: u32) -> CacheTag{ CacheTag(tag & 0xfffffe00 | 1) }

    #[inline]
    pub fn tag(&self) -> u32 {
        self.0 & 0xfffffe00
    }

    pub fn valid(&self) -> bool {
        (self.0 & 1) == 1
    }
}


struct ICache {
    data: [[u32; 8]; 512],
    tag: [CacheTag; 512],
}

impl ICache {
    pub fn fetch(&self, va: u64) -> (u32, CacheTag) {
        let word = va & 0x3;
        let line = va >> 2;

        (self.data[line as usize][word as usize], self.tag[line as usize])
    }
}

struct TlbEntry {
    vpn: u64,
    pfn: u32, // Pre-shifted
    asid: u8,
    g: bool,
}

struct ITlb {
    entires: [TlbEntry; 2],
    lru: u8,
}

struct MemSubsystemState {
    bit32: bool,
    asid: u8,
}

impl ITlb {
    pub fn translate(&mut self, mut va: u64, state: &MemSubsystemState) -> Option<u32> {
        if state.bit32 {
            // sign-extend
            va = va as u32 as i32 as i64 as u64;
        }
        // PERF: put a single-entry cache in front of this?

        // micro-tlb is hardcoded to just two 4k pages
        let vpn = va >> 12;
        let offset = (va & 0xfff) as u32;
        for (i, entry) in self.entires.iter().enumerate() {
            // HWTEST: Does micro-tlb even check asid?
            if entry.vpn == vpn && (entry.g || entry.asid == state.asid) {
                self.lru = i as u8;
                return Some(entry.pfn | offset);
            }
        }
        return None;
    }

    pub fn miss(&mut self, va: u64, state: &MemSubsystemState) {

    }
}

// struct JTlb {
//     entires: [TlbEntry; 32],
//     random: u8,
// }

// impl JTlb {
//     pub fn translate(&mut self, va: u64, asid: u8) -> Option<u32> {
//         // PERF: put a hash-map in front of this?

//         let vpn = va >> 12;
//         let offset = (va & 0xfff) as u32;
//         for (i, entry) in self.entires.iter().enumerate() {
//             //
//             if entry.vpn == vpn && {
//                 self.lru = i;
//                 return Some(entry.pfn | offset);
//             }
//         }
//         return None;
//     }
// }

mod pipeline {
    use crate::{ICache, ITlb, CacheTag};

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
        rs: u8,
        ut: u8,
        alu: AluMode
    }

    struct Execute {

    }
    struct DataCache {

    }
    struct WriteBack {

    }

    struct Pipeline {
        ic: InstructionCache,
        rf: RegisterFile,
        ex: Execute,
        dc: DataCache,
        wb: WriteBack,
    }


    impl Pipeline {
        pub fn cycle(&mut self, icache: &mut ICache, itlb: &mut ITlb) {
        // Phase 1
            // IC
                // Nothing
            // RF
            // Instruction Cache Tag Check
            let hit = self.ic.cache_tag.valid() &&
                Some(self.ic.cache_tag.tag()) == self.ic.expected_tag;


        // Phase 2
            // IC
            (self.ic.cache_data, self.ic.cache_tag) = icache.fetch(self.rf.next_pc);
            self.ic.expected_tag = itlb.translate(self.rf.next_pc);

            // RF
            self.rf.next_pc += 4;
            let inst_type = decode
            self.rf.rs =

            // EX


        }
    }
}

mod decoder {
    use modular_bitfield::{
        bitfield,
        specifiers::*,
        BitfieldSpecifier,
    };

    #[derive(BitfieldSpecifier)]
    #[bits = 6]
    pub enum Opcode {
        SPECIAL = 0b000_000,
        REGIMM,
        J,
        JAL,
        BEQ,
        BNE,
        BLEZ,
        BGTZ,

        ADDI = 0b001_000,
        ADDIU,
        SLTI,
        SLTIU,
        ANDI,
        ORI,
        XORI,
        LUI,

        COP0 = 0b010_000,
        COP1,
        COP2,
        // reserved
        BEQL,
        BNEL,
        BLEZL,
        BGTZL,

        DADDI = 0b011_000,
        DADDIU,
        LDL,
        LDR,
        // reserved
        // reserved
        // reserved
        // reserved

        LB = 0b100_000,
        LH,
        LWL,
        LW,
        LBU,
        LHU,
        LWR,
        LWU,

        SB = 0b101_000,
        SH,
        SWL,
        SW,
        SDL,
        SDR,
        SWR,
        CACHE,

        LL = 0b110_000,
        LWC1,
        LWC2,
        // reserved
        LLDe,
        LDC1,
        LDC2,
        LD,
        // reserved

        SC = 0b111_000,
        SWC1,
        SWC2,
        // reserved
        SCD,
        SCD1,
        SCD2,
        SD,
        // reserved
    }


    #[derive(BitfieldSpecifier)]
    #[bits = 6]
    pub enum Special {
        SLL = 0b000_000,
        // reserved
        SRR = 0b000_010,
        SRA,
        SLLV,
        // reserved
        SRLV = 0b000_110,
        SRAV,

        JR = 0b001_000,
        JALR,
        MOVZ,
        MOVN,
        SYSCALL,
        BREAK,
        // reserved
        SYNC,

        MFHI = 0b010_000,
        MTHI,
        MFLO,
        MTLO,
        DSLLV,
        // reserved
        DSRLV = 0b010_110,
        DSRAV,

        MULT = 0b011_000,
        MULTU,
        DIV,
        DIVU,
        DMULT,
        DMULTU,
        DDIV,
        DDIVU,

        ADD = 0b100_000,
        ADDU,
        SUB,
        SUBU,
        AND,
        OR,
        XOR,
        NOR,

        SLT = 0b101_000,
        SLTU,
        DADD,
        DADDU,
        DSUB,
        DSUBU,
        // reserved
        // reserved

        TGE = 0b110_000,
        TGEU,
        TLT,
        TLTU,
        TEQ,
        // reserved
        TNE,
        // reserved

        DSLL = 0b111_000,
        // reserved
        DSRL = 0b111_010,
        DSRA,
        DSLL32,
        // reserved
        DSRL32 = 0b111_110,
        DSRA32,
    }

    #[derive(BitfieldSpecifier)]
    #[bits = 5]
    pub enum Regimm {
        BLTZ = 0b000_00,
        BGEZ,
        BLTZL,
        BGEZL,
        // reserved
        // reserved
        // reserved
        // reserved

        TGEI = 0b010_00,
        TGEIU,
        TLTI,
        TLTIU,
        TEQI,
        // reserved
        TNEI,
        // reserved

        BLTZAL = 0b100_00,
        BGEZAL,
        BLTZALL,
        BGEZALL,
        // reserved
        // reserved
        // reserved
        // reserved
    }

    #[bitfield(bits = 32)]
    #[derive(BitfieldSpecifier)]
    pub struct IType {
        imm: B16,
        rt: B5,
        rs: B5,
        op: Opcode,
    }

    #[bitfield(bits = 32)]
    pub struct JType {
        target: B26,
        op: Opcode,
    }

    #[bitfield(bits = 32)]
    pub struct RType {
        funct: B6,
        sa: B5,
        rd: B5,
        rt: B5,
        rs: B5,
        op: Opcode,
    }

    #[bitfield(bits = 32)]
    pub struct Inst {
        data: B26,
        op: Opcode,
    }

    enum Instruction {
        I(IType),
        J(JType),
        R(RType),
    }

    fn decode(inst: u32) -> Instruction {
        let op = Opcode::from(inst >> 26);

        match i.op() {
            SPECIAL => {
                return Instruction::R(RType::from_bytes(inst.to_le_bytes()));
            },
            J | JAL => {
                return Instruction::J(JType::from_bytes(inst.to_le_bytes()));
            },
            _ => {
                return Instruction::I(IType::from_bytes(inst.to_le_bytes()));
            },
            }
        }
    }
}

fn main() {
    println!("Hello, world!");
}
