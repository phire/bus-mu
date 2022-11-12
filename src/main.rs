pub mod instructions;

#[derive(Clone, Copy, Debug)]
struct CacheTag(u32);
impl CacheTag {
    #[inline]
    pub fn empty() -> CacheTag {
        CacheTag(0)
    }
    pub fn new(tag: u32) -> CacheTag {
        CacheTag(tag & 0xfffffe00 | 1)
    }

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

        (
            self.data[line as usize][word as usize],
            self.tag[line as usize],
        )
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

    pub fn miss(&mut self, va: u64, state: &MemSubsystemState) {}
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
    use crate::{CacheTag, ICache, ITlb};

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
        alu: AluMode,
    }

    struct Execute {}
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
        pub fn cycle(
            &mut self,
            icache: &mut ICache,
            itlb: &mut ITlb,
            mem: crate::MemSubsystemState,
        ) {
            // Phase 1
            // IC
            // Nothing
            // RF
            // Instruction Cache Tag Check
            let hit =
                self.ic.cache_tag.valid() && Some(self.ic.cache_tag.tag()) == self.ic.expected_tag;

            // Phase 2
            // IC
            (self.ic.cache_data, self.ic.cache_tag) = icache.fetch(self.rf.next_pc);
            self.ic.expected_tag = itlb.translate(self.rf.next_pc, &mem);

            // RF
            self.rf.next_pc += 4;
            //let inst_type = decode
            //self.rf.rs =

            // EX
        }
    }
}

fn main() {
    let (inst, inst_info) = instructions::decode(0x3529E463);
    println!("{:x?} ", inst);
    println!("{:}", inst.to_string());
    println!("{:?} ", inst_info);
    println!("{:}", std::mem::size_of::<instructions::Form>())
    //println!("Hello, world!");
}
