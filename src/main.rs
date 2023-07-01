use cache::{ICache, DCache};
use microtlb::ITlb;
use pipeline::{MemoryReq, ExitReason};
use regfile::RegFile;

pub mod instructions;
pub mod pipeline;
pub mod coprocessor0;
pub mod cache;
pub mod microtlb;
pub mod joint_tlb;
pub mod regfile;

fn main() {
    let pif_rom = std::fs::read("pifdata.bin").unwrap();

    let mut pipeline = pipeline::create();
    let mut icache = ICache::new();
    let mut dcache = DCache::new();

    let mut itlb = ITlb::new();

    let mut regfile = RegFile::new();

    for i in 0..64 {
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
                        let offset = (addr - 0x1fc00000) as usize;
                        let bytes = &pif_rom[offset..(offset + 4)];
                        u32::from_be_bytes(bytes.try_into().unwrap())
                    }
                    _ => panic!("Invalid address: {:08x}", addr),
                };

                let (inst, _inst_info) = instructions::decode(word);
                pipeline.memory_responce(pipeline::MemoryResponce::UncachedInstructionRead(word), &mut icache, &mut dcache);
                println!("(uncached) {:04x}: {:08x}    {}", addr, word, inst.disassemble(addr as u64));
                continue;
            }
            ExitReason::Mem(MemoryReq::UncachedDataRead(addr, size)) => {
                println!("Uncached read: {:08x} ({} bytes)", addr, size);
            }
            ExitReason::Mem(MemoryReq::UncachedDataWrite(addr, size, data)) => {
                println!("Uncached write: {:08x} ({} bytes) = {:08x}", addr, size, data);
            }
            ExitReason::Ok => { continue; }
        }

        break;
    }
}
