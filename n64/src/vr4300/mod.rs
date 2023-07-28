
/// The VR4300 is the main CPU of the Nintendo 64.
///
/// It has an entire chip to itself, and is more-or-less an off-the-shelf part (it appears
/// Nintendo did some customization at the packaging level: moving some pins around, disabling JTAG)

use cache::{ICache, DCache};
use microtlb::ITlb;
use pipeline::{MemoryReq, ExitReason};
use regfile::RegFile;
use pipeline::Pipeline;

pub mod instructions;
pub mod pipeline;
pub mod coprocessor0;
pub mod cache;
pub mod microtlb;
pub mod joint_tlb;
pub mod regfile;

pub fn test() {
    let pif_rom = std::fs::read("pifdata.bin").unwrap();

    let mut pipeline = pipeline::create();
    let mut icache = ICache::new();
    let mut dcache = DCache::new();

    let mut itlb = ITlb::new();

    let mut regfile = RegFile::new();

    for i in 0..128 {
        println!("    cycle: {:3}, PC:{:08x}", i, pipeline.pc());
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
                let word = match addr {
                    0x1fc00000..=0x1fc007bf => {
                        let offset = (addr & 0x7fc) as usize;
                        let bytes = &pif_rom[offset..(offset + 4)];
                        u32::from_be_bytes(bytes.try_into().unwrap())
                    }
                    _ => panic!("Invalid address: {:08x}", addr),
                };

                let (inst, _inst_info) = instructions::decode(word);
                pipeline.memory_responce(pipeline::MemoryResponce::UncachedInstructionRead(word), &mut icache, &mut dcache, &mut regfile);
                println!("(uncached) {:04x}: {:08x}    {}", addr, word, inst.disassemble(addr as u64));
                continue;
            }
            ExitReason::Mem(MemoryReq::UncachedDataRead(addr, size)) => {
                println!("Uncached read: {:#08x} ({} bytes)", addr, size);
                let mut data ;
                match addr {
                    0x04040010 => { // SP Status
                        if i < 40 {
                            data = 0; // Busy
                        } else {
                            data = 1; // Idle
                        }
                    }
                    _ => { todo!(); }
                }
                pipeline.memory_responce(pipeline::MemoryResponce::UncachedDataRead(data), &mut icache, &mut dcache, &mut regfile);
                continue;
            }
            ExitReason::Mem(MemoryReq::UncachedDataWrite(addr, size, data)) => {
                println!("Uncached write: {:08x} ({} bytes) = {:08x}", addr, size, data);
            }
            ExitReason::Ok => { continue; }
        }

        break;
    }
}

pub struct Core {
    pipeline: Pipeline,
    icache: ICache,
    dcache: DCache,
    itlb: ITlb,
    regfile: RegFile,
    //bus: SysADBus,
    queued_flush: Option<(u32, [u8; 16])>
}

impl Core {
    pub fn run(&mut self, cycle_limit: u64) -> CoreRunResult {

        let mut cycles = 0;

        if let Some((addr, data)) = self.queued_flush.take() {
            return CoreRunResult {
                cycles,
                reason: Reason::BusWrite128(addr, data)
            }
        }

        while cycles < cycle_limit {
            cycles += 1;

            let reason = self.pipeline.cycle(
                &mut self.icache,
                &mut self.dcache,
                &mut self.itlb,
                &mut self.regfile
            );
            let reason = match reason {
                ExitReason::Ok => { continue; }
                ExitReason::Mem(MemoryReq::ICacheFill(addr)) => {
                    println!("ICache fill: {:08x}", addr);
                    Reason::BusRead256(addr)
                }
                ExitReason::Mem(MemoryReq::DCacheFill(addr)) => {
                    println!("DCache fill: {:08x}", addr);
                    Reason::BusRead128(addr)
                }
                ExitReason::Mem(MemoryReq::DCacheReplace(new_addr, old_addr, _data)) => {
                    println!("DCache replace: {:08x} -> {:08x}", old_addr, new_addr);

                    self.queued_flush = Some((old_addr, [0; 16]));
                    Reason::BusRead128(new_addr)
                }
                ExitReason::Mem(MemoryReq::UncachedInstructionRead(addr)) => {
                    println!("Uncached instruction read: {:08x}", addr);
                    Reason::BusRead32(addr)
                }
                ExitReason::Mem(MemoryReq::UncachedDataRead(addr, size)) => {
                    println!("Uncached data read: {:08x} ({} bytes)", addr, size);
                    match size {
                        1 | 2 | 4 => Reason::BusRead32(addr),
                        8 => Reason::BusRead64(addr),
                        _ => unreachable!(),
                    }
                }
                ExitReason::Mem(MemoryReq::UncachedDataWrite(addr, size, data)) => {
                    println!("Uncached data write: {:08x} ({} bytes) = {:08x}", addr, size, data);
                    match size {
                        1 => Reason::BusWrite8(addr, data as u32),
                        2 => Reason::BusWrite16(addr, data as u32),
                        4 => Reason::BusWrite32(addr, data as u32),
                        8 => Reason::BusWrite64(addr, data),
                        _ => unreachable!(),
                    }
                }
            };

            return CoreRunResult {
                cycles,
                reason,
            }
        }

        return CoreRunResult {
            cycles,
            reason: Reason::Limited,
        }
    }

    pub fn set_time(&mut self, time: u64) {
        todo!("pipeline.set_time");
    }

}

impl Default for Core {
    fn default() -> Self {
        Core {
            pipeline: pipeline::create(),
            icache: ICache::new(),
            dcache: DCache::new(),
            itlb: ITlb::new(),
            regfile: RegFile::new(),
            queued_flush: None,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Reason {
    Limited,
    SyncRequest,
    BusRead32(u32),
    BusRead64(u32),
    BusRead128(u32),
    BusRead256(u32),
    BusWrite8(u32, u32),
    BusWrite16(u32, u32),
    BusWrite24(u32, u32),
    BusWrite32(u32, u32),
    BusWrite64(u32, u64),
    BusWrite128(u32, [u8; 16]),
}

impl Reason {
    pub fn is_bus(&self) -> bool {
        match self {
            Reason::Limited | Reason::SyncRequest => { false }
            _ => { true }
        }
    }

    pub fn address(&self) -> u32 {
        match self {
            Reason::Limited => { unreachable!(); }
            Reason::SyncRequest => { unreachable!(); }
            Reason::BusRead32(addr) => { *addr }
            Reason::BusRead64(addr) => { *addr }
            Reason::BusRead128(addr) => { *addr }
            Reason::BusRead256(addr) => { *addr }
            Reason::BusWrite8(addr, _) => { *addr }
            Reason::BusWrite16(addr, _) => { *addr }
            Reason::BusWrite24(addr, _) => { *addr }
            Reason::BusWrite32(addr, _) => { *addr }
            Reason::BusWrite64(addr, _) => { *addr }
            Reason::BusWrite128(addr, _) => { *addr }
        }
    }
}

pub struct CoreRunResult {
    pub cycles: u64,
    pub reason: Reason,
}