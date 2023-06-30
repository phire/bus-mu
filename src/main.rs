pub mod instructions;
pub mod pipeline;
pub mod coprocessor0;

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

    pub fn miss(&mut self, _va: u64, _state: &MemSubsystemState) {}
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

struct RegFile {
    regs: [u64; 32],
    hilo: [u64; 2],
}

impl RegFile {
    pub fn read(&self, reg: u8) -> u64 {
        self.regs[reg as usize]
    }
    pub fn write(&mut self, reg: u8, val: u64) {
        if reg != 0 {
            self.regs[reg as usize] = val;
        }
    }
}

fn main() {
    let pif_rom = std::fs::read("pifdata.bin").unwrap();

    for i in 0..64 {
        let addr = i * 4;
        let bytes = &pif_rom[addr..(addr + 4)];
        let word = u32::from_be_bytes(bytes.try_into().unwrap());
        let (inst, _inst_info) = instructions::decode(word);
        //println!("{:x?} ", inst);
        println!("{:04x}: {:08x}    {}", addr, word, inst.disassemble(addr as u64));
        //println!("{:?} ", _inst_info);
    }
}
