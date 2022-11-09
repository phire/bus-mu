
enum Form {
    IType,
    JType,
    RType,
}

struct Info {
        name: &'static str,
        op: u8,
        form: Form,
}

enum InstructionInfo {
    Reserved,
    Op (&'static str, u8, Form),
}

const fn build_primary_table() -> [InstructionInfo; 64] {
    use InstructionInfo::*;
    [
        Op( "SPECIAL", 0x0, Form::RType ),
        Op( "REGIMM", 0x1, Form::RType ),
        Op( "J", 0x2, Form::JType ),
        Op( "JAL", 0x3, Form::JType ),
        Op( "BEQ", 0x4, Form::IType ),
        Op( "BNE", 0x5, Form::IType ),
        Op( "BLEZ", 0x6, Form::IType ),
        Op( "BGTZ", 0x7, Form::IType ),

        Op( "ADDI", 0x8, Form::IType ),
        Op( "ADDIU", 0x9, Form::IType ),
        Op( "SLTI", 0xa, Form::IType ),
        Op( "SLTIU", 0xb, Form::IType ),
        Op( "ANDI", 0xc, Form::IType ),
        Op( "ORI", 0xd, Form::IType ),
        Op( "XORI", 0xe, Form::IType ),
        Op( "LUI", 0xf, Form::IType ),

        Op( "COP0", 0x10, Form::IType ),
        Op( "COP1", 0x11, Form::IType ),
        Op( "COP2", 0x12, Form::IType ),
        Reserved,
        Op( "BEQL", 0x14, Form::IType ),
        Op( "BNEL", 0x15, Form::IType ),
        Op( "BLEZL", 0x16, Form::IType ),
        Op( "BGTZL", 0x17, Form::IType ),

        Op( "DADDI", 0x18, Form::IType ),
        Op( "DADDIU", 0x19, Form::IType ),
        Op( "LDL", 0x1a, Form::IType ),
        Op( "LDR", 0x1b, Form::IType ),
        Reserved,
        Reserved,
        Reserved,
        Reserved,

        Op( "LB", 0x20, Form::IType ),
        Op( "LH", 0x21, Form::IType ),
        Op( "LWL", 0x22, Form::IType ),
        Op( "LW", 0x23, Form::IType ),
        Op( "LBU", 0x24, Form::IType ),
        Op( "LHU", 0x25, Form::IType ),
        Op( "LWR", 0x26, Form::IType ),
        Op( "LWU", 0x27, Form::IType ),

        Op( "SB", 0x28, Form::IType ),
        Op( "SH", 0x29, Form::IType ),
        Op( "SWL", 0x2a, Form::IType ),
        Op( "SW", 0x2b, Form::IType ),
        Op( "SDL", 0x2c, Form::IType ),
        Op( "SDR", 0x2d, Form::IType ),
        Op( "SWR", 0x2e, Form::IType ),
        Op( "CACHE", 0x2f, Form::IType ),

        Op( "LL", 0x30, Form::IType ),
        Op( "LWC1", 0x31, Form::IType ),
        Op( "LWC2", 0x32, Form::IType ),
        Reserved,
        Op( "LLD", 0x34, Form::IType ),
        Op( "LDC1", 0x35, Form::IType ),
        Op( "LDC2", 0x36, Form::IType ),
        Op( "LD", 0x37, Form::IType ),

        Op( "SC", 0x38, Form::IType ),
        Op( "SWC1", 0x39, Form::IType ),
        Op( "SWC2", 0x3a, Form::IType ),
        Reserved,
        Op( "SCD", 0x3c, Form::IType ),
        Op( "SDC1", 0x3d, Form::IType ),
        Op( "SDC2", 0x3e, Form::IType ),
        Op( "SD", 0x3f, Form::IType ),
    ]
}

const PrimaryTable: [InstructionInfo; 64] = build_primary_table();