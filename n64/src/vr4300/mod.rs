
/// The VR4300 is the main CPU of the Nintendo 64.
///
/// It has an entire chip to itself, and is more-or-less an off-the-shelf part (it appears
/// Nintendo did some customization at the packaging level: moving some pins around, disabling JTAG)

use cache::{ICache, DCache};
use microtlb::ITlb;
use pipeline::{MemoryReq, ExitReason};
use regfile::RegFile;
use pipeline::Pipeline;

use self::pipeline::MemoryResponce;

pub mod instructions;
pub mod pipeline;
pub mod coprocessor0;
pub mod cache;
pub mod microtlb;
pub mod joint_tlb;
pub mod regfile;


#[derive(Copy, Clone, Debug)]
pub enum RequestType {
    ICacheFill,
    DCacheFill,
    DCacheWriteback,
    UncachedInstructionRead,
    UncachedDataRead,
    UncachedWrite,
}

pub struct Core {
    pipeline: Pipeline,
    icache: ICache,
    dcache: DCache,
    itlb: ITlb,
    regfile: RegFile,
    //bus: SysADBus,
    queued_flush: Option<(u32, [u8; 16])>,
}

impl Core {
    pub fn advance(&mut self, cycle_limit: u64) -> CoreRunResult {

        let mut cycles = 0;

        if let Some((addr, data)) = self.queued_flush.take() {
            return CoreRunResult {
                cycles,
                reason: Reason::BusWrite128(RequestType::DCacheWriteback, addr, data)
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
                ExitReason::Blocked => {
                    cycles = cycle_limit;
                    break;
                }
                ExitReason::Mem(MemoryReq::ICacheFill(addr)) => {
                    println!("ICache fill: {:08x}", addr);
                    Reason::BusRead256(RequestType::ICacheFill, addr)
                }
                ExitReason::Mem(MemoryReq::DCacheFill(addr)) => {
                    println!("DCache fill: {:08x}", addr);
                    Reason::BusRead128(RequestType::DCacheFill, addr)
                }
                ExitReason::Mem(MemoryReq::DCacheReplace(new_addr, old_addr, _data)) => {
                    println!("DCache replace: {:08x} -> {:08x}", old_addr, new_addr);
                    self.queued_flush = Some((old_addr, [0; 16]));
                    Reason::BusRead128(RequestType::DCacheFill, new_addr)
                }
                ExitReason::Mem(MemoryReq::UncachedInstructionRead(addr)) => {
                    println!("Uncached instruction read: {:08x}", addr);
                    Reason::BusRead32(RequestType::UncachedInstructionRead, addr)
                }
                ExitReason::Mem(MemoryReq::UncachedDataRead(addr, size)) => {
                    println!("Uncached data read: {:08x} ({} bytes)", addr, size);
                    match size {
                        1 | 2 | 4 => Reason::BusRead32(RequestType::UncachedDataRead, addr),
                        8 => Reason::BusRead64(RequestType::UncachedDataRead, addr),
                        _ => unreachable!(),
                    }
                }
                ExitReason::Mem(MemoryReq::UncachedDataWrite(addr, size, data)) => {
                    println!("Uncached data write: {:08x} ({} bytes) = {:08x}", addr, size, data);
                    match size {
                        1 => Reason::BusWrite8(RequestType::UncachedWrite, addr, data as u32),
                        2 => Reason::BusWrite16(RequestType::UncachedWrite, addr, data as u32),
                        4 => Reason::BusWrite32(RequestType::UncachedWrite, addr, data as u32),
                        8 => Reason::BusWrite64(RequestType::UncachedWrite, addr, data),
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

    pub fn finish_read(&mut self, request_type: RequestType, data: &[u32; 8], length: u64) {
        let response = match request_type {
            RequestType::UncachedInstructionRead => {
                let word = data[0];
                let (inst, _inst_info) = instructions::decode(word);
                let addr = self.pipeline.pc();
                println!("(uncached) {:04x}: {:08x}    {}", addr, word, inst.disassemble(addr as u64));
                MemoryResponce::UncachedInstructionRead(data[0])
            }
            RequestType::DCacheFill => {
                todo!()
                //MemoryResponce::DCacheFill([mem])
            }
            RequestType::UncachedDataRead => {
                match length {
                    1 => {
                        MemoryResponce::UncachedDataRead(data[0] as u64)
                    }
                    2 => {
                        MemoryResponce::UncachedDataRead((data[0] as u64) << 16 | (data[1] as u64))
                    }
                    _ => unreachable!(),
                }
            }
            RequestType::ICacheFill => {
                MemoryResponce::ICacheFill(*data)
            }
            _ => unreachable!(),
        };

        self.pipeline.memory_responce(
            response,
            &mut self.icache,
            &mut self.dcache,
            &mut self.regfile
        )
    }

    pub fn finish_write(&mut self, request_type: RequestType, length: u64) {
        let response = match request_type {
            RequestType::UncachedWrite => {
                MemoryResponce::UncachedDataWrite
            }
            _ => unreachable!(),
        };
        self.pipeline.memory_responce(
            response,
            &mut self.icache,
            &mut self.dcache,
            &mut self.regfile
        )
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
    BusRead32(RequestType, u32),
    BusRead64(RequestType, u32),
    BusRead128(RequestType, u32),
    BusRead256(RequestType, u32),
    BusWrite8(RequestType, u32, u32),
    BusWrite16(RequestType, u32, u32),
    BusWrite24(RequestType, u32, u32),
    BusWrite32(RequestType, u32, u32),
    BusWrite64(RequestType, u32, u64),
    BusWrite128(RequestType, u32, [u8; 16]),
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
            Reason::Limited => { panic!(); }
            Reason::SyncRequest => { panic!(); }
            Reason::BusRead32(_, addr) => { *addr }
            Reason::BusRead64(_, addr) => { *addr }
            Reason::BusRead128(_, addr) => { *addr }
            Reason::BusRead256(_, addr) => { *addr }
            Reason::BusWrite8(_, addr, _) => { *addr }
            Reason::BusWrite16(_, addr, _) => { *addr }
            Reason::BusWrite24(_, addr, _) => { *addr }
            Reason::BusWrite32(_, addr, _) => { *addr }
            Reason::BusWrite64(_, addr, _) => { *addr }
            Reason::BusWrite128(_, addr, _) => { *addr }
        }
    }

    pub fn request_type(&self) -> RequestType {
        match self {
            Reason::Limited => { panic!(); }
            Reason::SyncRequest => { panic!(); }
            Reason::BusRead32(request_type, _) => { *request_type }
            Reason::BusRead64(request_type, _) => { *request_type }
            Reason::BusRead128(request_type, _) => { *request_type }
            Reason::BusRead256(request_type, _) => { *request_type }
            Reason::BusWrite8(request_type, _, _) => { *request_type }
            Reason::BusWrite16(request_type, _, _) => { *request_type }
            Reason::BusWrite24(request_type, _, _) => { *request_type }
            Reason::BusWrite32(request_type, _, _) => { *request_type }
            Reason::BusWrite64(request_type, _, _) => { *request_type }
            Reason::BusWrite128(request_type, _, _) => { *request_type }
        }
    }
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reason::Limited => write!(f, "Limited"),
            Reason::SyncRequest => write!(f, "SyncRequest"),
            Reason::BusRead32(RequestType::UncachedDataRead ,addr) => write!(f, "ReadData32({:#08x})", addr),
            Reason::BusRead32(RequestType::UncachedInstructionRead ,addr) => write!(f, "ReadInst32({:#08x})", addr),
            Reason::BusRead64(RequestType::UncachedDataRead, addr) => write!(f, "ReadData64({:#08x})", addr),
            Reason::BusRead128(RequestType::DCacheFill, addr) => write!(f, "FillDCache128({:#08x})", addr),
            Reason::BusRead256(RequestType::ICacheFill, addr) => write!(f, "FillICache256({:#08x})", addr),
            Reason::BusWrite8(RequestType::UncachedWrite, addr, data) => write!(f, "Write8({:#08x}, {:#02x})", addr, data),
            Reason::BusWrite16(RequestType::UncachedWrite, addr, data) => write!(f, "Write16({:#08x}, {:#04x})", addr, data),
            Reason::BusWrite24(RequestType::UncachedWrite, addr, data) => write!(f, "Write24({:#08x}, {:#06x})", addr, data),
            Reason::BusWrite32(RequestType::UncachedWrite, addr, data) => write!(f, "Write32({:#08x}, {:#08x})", addr, data),
            Reason::BusWrite64(RequestType::UncachedWrite, addr, data) => write!(f, "Write64({:#08x}, {:#016x})", addr, data),
            Reason::BusWrite128(RequestType::DCacheWriteback, addr, data) => write!(f, "WriteCache128({:08x}, {:?})", addr, data),
            _ => panic!("Unknown reason"),
        }
    }
}

pub struct CoreRunResult {
    pub cycles: u64,
    pub reason: Reason,
}