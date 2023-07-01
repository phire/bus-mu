use pipeline::{MemoryReq, ExitReason};

pub mod instructions;
pub mod pipeline;
pub mod coprocessor0;

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub struct CacheTag(u32);
impl CacheTag {
    #[inline]
    pub fn empty() -> CacheTag {
        CacheTag(0)
    }
    pub fn invalid() -> CacheTag {
        CacheTag(0xcccc_ccce)
    }

    pub fn new(tag: u32) -> CacheTag {
        CacheTag(tag & 0xffff_e000 | 1)
    }

    pub fn new_uncached(addr: u32) -> CacheTag {
        CacheTag(addr | 3)
    }

    #[inline]
    pub fn tag(&self) -> u32 {
         self.0 & 0xffff_e000
    }

    pub fn is_valid(&self) -> bool {
        (self.0 & 1) == 1
    }

    pub fn is_uncached(&self) -> bool {
        (self.0 & 3) == 3
    }

    pub fn get_uncached(&self) -> u32 {
        self.0 & 0xffff_fffc
    }

    pub fn is_dirty(&self) -> bool {
        (self.0 & 7) == 5
    }
}

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub enum ICacheState {
    Normal,
    Filling,
    Refilled,
}

pub struct ICache {
    tag: [CacheTag; 512],
    data: [[u32; 8]; 512],
    state: ICacheState,
    /// The last uncached read
    uncached_read: (u32, CacheTag),

}

impl ICache {
    pub fn fetch(&mut self, va: u64) -> (u32, CacheTag) {
        if va & 0xe000_0000 == 0xa000_0000 { // uncached via kseg1
            // We just return the last uncached read
            return self.uncached_read;
        }
        let word = va & 0x3;
        let line = (va >> 2) & 0x1ff;

        return (
            self.data[line as usize][word as usize],
            self.tag[line as usize],
        );
    }
}

pub struct DCache {
    data: [[u8; 16]; 512],
    tag: [CacheTag; 512],
    /// The last uncached read
    uncached_read: (u32, CacheTag),
}

impl DCache {
    pub fn open(&mut self, addr: u64) -> DataCacheAttempt {
        let line = ((addr & 0x1ff0) >> 4) as usize;

        return DataCacheAttempt {
            tag: self.tag[line],
            line: line as u16,
            offset: (addr & 0xf) as u8
        };
    }
}

#[derive(Clone, Copy)]
pub struct DataCacheAttempt {
    tag: CacheTag,
    line: u16,
    offset: u8
}

impl DataCacheAttempt {
    pub fn empty() -> DataCacheAttempt {
        DataCacheAttempt {
            tag: CacheTag::invalid(),
            line: 0,
            offset: 0,
        }
    }

    pub fn is_hit(self, tlb_tag: CacheTag) -> bool {
        self.tag == tlb_tag && self.tag.is_valid()
    }

    pub fn do_miss(self, dcache: &DCache, tlb_tag: CacheTag) -> MemoryReq {
        let line = self.line as u32;
        let physical_address = tlb_tag.tag() | line;
        if self.tag.is_dirty() {
            let flush_physical_address = self.tag.tag() | line;
            MemoryReq::DCacheReplace(
                physical_address,
                flush_physical_address,
                dcache.data[self.line as usize],
            )
        } else {
            MemoryReq::DCacheFill(physical_address)
        }
    }

    pub fn write(self, dcache: &mut DCache, size: usize, data: u64) {
        // PERF: check if this function produces good code
        let start = self.offset as usize;
        let data_bytes = data.to_le_bytes();
        for i in 0..(size as usize) {
            dcache.data[self.line as usize][start + i] = data_bytes[i];
        }
    }

    pub fn read(self, dcache: &DCache, size: usize) -> u64 {
        // PERF: check if this function produces good code
        let start = self.offset as usize;
        let mut data_bytes = [0; 8];
        for i in 0..size {
            data_bytes[i] = dcache.data[self.line as usize][start + size - i];
        }

        return u64::from_le_bytes(data_bytes);
    }

}

struct TlbEntry {
    vpn: u64,
    pfn: u32, // Pre-shifted
    _asid: u8,
    g: bool,
}

pub struct ITlb {
    entires: [TlbEntry; 2],
    lru: u8, // vr4300 user manual says:
             //    Micro-TLB "uses the least-recently- used (LRU) replacement algorithm"
}

// struct MemSubsystemState {
//     bit32: bool,
//     asid: u8,
// }

impl ITlb {
    /// The CPU pipeline will call this every cycle for the instruction it's about to execute
    ///
    /// # Arguments
    ///
    /// * `va` - The 64bit (sign-extended) virtual address to translate
    /// * `state` - The current state of the memory subsystem
    ///
    /// # Returns
    ///
    /// Returns a cache tag if the
    ///
    pub fn translate(&mut self, va: u64) -> CacheTag {
        // PERF: put a single-entry cache in front of this?

        // These segments bypass TLB
        // HWTEST: Is TLB bypassing actually done here?
        //         It's theoretically possible that a JTLB lookup creates fake entries for these
        match va {
            0xffff_ffff_8000_0000..=0xffff_ffff_9fff_ffff => { // kseg0
                return CacheTag::new_uncached(va as u32 & 0x1fff_ffff);
            }
            0xffff_ffff_a000_0000..=0xffff_ffff_bfff_ffff => { // kseg1
                return CacheTag::new_uncached(va as u32 & 0x1fff_ffff);
            }
            _ => {}
        }

        // ACCURACY: Need to do permission checks
        //           But do we do it here, or when loading from JTLB?

        // micro-tlb is hardcoded to just two 4k pages
        let vpn = va >> 12;
        //let offset = (va & 0xfff) as u32;
        for (i, entry) in self.entires.iter().enumerate() {
            // TODO: Asid check
            // HWTEST: Does micro-tlb even check asid?
            //         Night make sense to only check it when loading from JTLB

            // TODO: handle pages marked as uncached
            let asid_match = true; // entry.asid == state.asid;
            if entry.vpn == vpn && (entry.g || asid_match) {
                self.lru = i as u8;
                return CacheTag::new(entry.pfn);
            }
        }
        return CacheTag::empty();
    }

    // pub fn miss(&mut self, _va: u64, _state: &MemSubsystemState) -> Option<u32> {
    //     // This is called when the pipeline didn't match on the previous translate
    //     // We need to load the correct TLB entry from JTLB (if it exists)

    //     // If it doesn't exist, the pipeline will raise a TLB miss exception, and the OS
    //     // is expected to update JTLB with the correct entry

    //     todo!("TLB miss")
    // }
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

pub struct RegFile {
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

    let mut pipeline = pipeline::create();
    let mut icache = ICache {
        tag: [CacheTag::invalid(); 512],
        data: [[0; 8]; 512],
        state: ICacheState::Normal,
        uncached_read: (0, CacheTag::invalid()),
    };
    let mut dcache = DCache {
        data: [[0; 16]; 512],
        tag: [CacheTag::invalid(); 512],
        uncached_read: (0, CacheTag::invalid()),
    };
    let mut itlb = ITlb {
        entires: [
            TlbEntry {
                vpn: 0,
                pfn: 0,
                _asid: 0,
                g: true,
            },
            TlbEntry {
                vpn: 0,
                pfn: 0,
                _asid: 0,
                g: true,
            },
        ],
        lru: 0,
    };

    let mut regfile = RegFile {
        regs: [0; 32],
        hilo: [0; 2],
    };

    for i in 0..64 {
        println!("    cycle {:3}:", i);
        let reason = pipeline.cycle(&mut icache, &mut dcache, &mut itlb, &mut regfile);

        match reason {
            ExitReason::Mem(MemoryReq::ICacheFill(addr)) => {
                println!("ICache fill: {:08x}", addr);
            }
            ExitReason::Mem(MemoryReq::DCacheFill(addr)) => {
                println!("DCache fill: {:08x}", addr);
            }
            ExitReason::Mem(MemoryReq::DCacheReplace(new_addr, old_addr, _data)) => {
                println!("DCache replace: {:08x} -> {:08x}", old_addr, new_addr);
            }
            ExitReason::Mem(MemoryReq::UncachedInstructionRead(addr)) => {
                println!("Uncached Instruction req: {:08x}", addr);
            }
            ExitReason::Mem(MemoryReq::UncachedDataRead(addr, size)) => {
                println!("Uncached read: {:08x} ({} bytes)", addr, size);
            }
            ExitReason::Mem(MemoryReq::UncachedDataWrite(addr, size, data)) => {
                println!("Uncached write: {:08x} ({} bytes) = {:08x}", addr, size, data);
            }
            ExitReason::Ok => { continue;}
        }

        break;

        // let addr = i * 4;
        // let bytes = &pif_rom[addr..(addr + 4)];
        // let word = u32::from_be_bytes(bytes.try_into().unwrap());
        // let (inst, _inst_info) = instructions::decode(word);
        // //println!("{:x?} ", inst);
        // println!("{:04x}: {:08x}    {}", addr, word, inst.disassemble(addr as u64));
        // //println!("{:?} ", _inst_info);
    }
}
