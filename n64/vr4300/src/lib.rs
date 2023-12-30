
/// The VR4300 is the main CPU of the Nintendo 64.
///
/// It has an entire chip to itself, and is more-or-less an off-the-shelf part (it appears
/// Nintendo did some customization at the packaging level: moving some pins around, disabling JTAG)

use cache::{ICache, DCache};
use microtlb::ITlb;
use pipeline::{MemoryReq, ExitReason};
use pipeline::Pipeline;
use common::util::ByteMask8;

use self::pipeline::MemoryResponce;

pub mod instructions;
pub mod pipeline;
pub mod coprocessor0;
pub mod cache;
pub mod microtlb;
pub mod joint_tlb;
pub mod regfile;
#[cfg(feature = "ui")]
pub mod ui;


#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
    //bus: SysADBus,
    queued_flush: Option<(u32, [u64; 2])>,
    count: u64,
}

impl Core {
    pub fn advance(&mut self, cycle_limit: u64) -> CoreRunResult {
        if self.pipeline.blocked() {
            return CoreRunResult {
                cycles: cycle_limit,
                reason: Reason::Limited,
            }
        }

        let mut cycles = 0;
        while cycles < cycle_limit {
            cycles += 1;

            let reason = self.pipeline.cycle(
                &mut self.icache,
                &mut self.dcache,
                &mut self.itlb,
            );
            // TODO: implement flush buffers
            let reason = Reason::BusRequest(match reason {
                Ok(()) | Err(ExitReason::Stalled) => { continue; }
                Err(ExitReason::Blocked) => {
                    cycles = cycle_limit;
                    break;
                }
                Err(ExitReason::Mem(mem)) => {
                    match mem {
                        MemoryReq::ICacheFill(addr) => {
                            println!("ICache fill: {:08x}", addr);
                            BusRequest::BusRead256(RequestType::ICacheFill, addr)
                        }
                        MemoryReq::DCacheFill(addr) => {
                            println!("DCache fill: {:08x}", addr);
                            BusRequest::BusRead128(RequestType::DCacheFill, addr)
                        }
                        MemoryReq::DCacheReplace(new_addr, old_addr, data) => {
                            println!("DCache replace: {:08x} -> {:08x}", old_addr, new_addr);
                            self.queued_flush = Some((old_addr, data));
                            BusRequest::BusRead128(RequestType::DCacheFill, new_addr)
                        }
                        MemoryReq::UncachedInstructionRead(addr) => {
                            //println!("Uncached instruction read: {:08x}", addr);
                            BusRequest::BusRead32(RequestType::UncachedInstructionRead, addr)
                        }
                        MemoryReq::UncachedDataReadWord(addr) => {
                            BusRequest::BusRead32(RequestType::UncachedDataRead, addr)
                        },
                        MemoryReq::UncachedDataReadDouble(addr) => {
                            BusRequest::BusRead64(RequestType::UncachedDataRead, addr)
                        },
                        MemoryReq::UncachedDataWriteWord(addr, data, mask) => {
                            BusRequest::BusWrite32(RequestType::UncachedWrite, addr, data, mask)
                        },
                        MemoryReq::UncachedDataWriteDouble(addr, data, mask) => {
                            BusRequest::BusWrite64(RequestType::UncachedWrite, addr, data, mask)
                        },
                    }
                }
            });

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
        todo!("pipeline.set_time {}", time);
    }

    #[inline(always)]
    pub fn finish_read(&mut self, request_type: RequestType, data: &[u64], transfers: usize) -> Option<BusRequest> {
        let mut mem_req = None;
        let response = match request_type {
            RequestType::UncachedInstructionRead => {
                // let addr = self.pipeline.pc();
                // let word = (data[0] >> (8 * (!addr & 4))) as u32;
                // let (inst, _inst_info) = instructions::decode(word);
                // println!("(uncached) {:04x}: {:08x}    {}", addr, word, inst.disassemble(addr as u64));
                self.count += 1;
                MemoryResponce::UncachedInstructionRead(data[0])
            }
            RequestType::DCacheFill => {
                mem_req = self.queued_flush.take().map(|(addr, data)| {
                    BusRequest::BusWrite128(RequestType::DCacheWriteback, addr, data)
                });

                MemoryResponce::DCacheFill(data.try_into().unwrap())
            }
            RequestType::UncachedDataRead => {
                MemoryResponce::UncachedDataRead(data[0])
            }
            RequestType::ICacheFill => {
                MemoryResponce::ICacheFill(data.try_into().unwrap())
            }
            _ => unreachable!(),
        };

        self.pipeline.memory_responce(
            response,
            transfers,
            &mut self.icache,
            &mut self.dcache,
        );

        mem_req
    }

    #[inline(always)]
    pub fn finish_write(&mut self, request_type: RequestType, words: usize) {
        let response = match request_type {
            RequestType::UncachedWrite => {
                MemoryResponce::UncachedDataWrite
            }
            _ => unreachable!(),
        };
        self.pipeline.memory_responce(
            response,
            words,
            &mut self.icache,
            &mut self.dcache,
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
            queued_flush: None,
            count: 0,
        }
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        eprintln!("Core executed {} instructions", self.count);
    }
}

#[derive(Copy, Clone)]
pub enum Reason {
    Limited,
    SyncRequest,
    BusRequest(BusRequest),
}

#[derive(Copy, Clone, Debug)]
pub enum BusRequest {
    BusRead32(RequestType, u32),
    BusRead64(RequestType, u32),
    BusRead128(RequestType, u32),
    BusRead256(RequestType, u32),
    BusWrite32(RequestType, u32, u64, ByteMask8),
    BusWrite64(RequestType, u32, u64, ByteMask8),
    BusWrite128(RequestType, u32, [u64; 2]),
}

impl BusRequest {
    #[inline(always)]
    pub fn address(&self) -> u32 {
        match self {
            BusRequest::BusRead32(_, addr) => { *addr }
            BusRequest::BusRead64(_, addr) => { *addr }
            BusRequest::BusRead128(_, addr) => { *addr }
            BusRequest::BusRead256(_, addr) => { *addr }
            BusRequest::BusWrite32(_, addr, _, _) => { *addr }
            BusRequest::BusWrite64(_, addr, _, _) => { *addr }
            BusRequest::BusWrite128(_, addr, _) => { *addr }
        }
    }

    #[inline(always)]
    pub fn request_type(&self) -> RequestType {
        match self {
            BusRequest::BusRead32(request_type, _) => { *request_type }
            BusRequest::BusRead64(request_type, _) => { *request_type }
            BusRequest::BusRead128(request_type, _) => { *request_type }
            BusRequest::BusRead256(request_type, _) => { *request_type }
            BusRequest::BusWrite32(request_type, _, _, _) => { *request_type }
            BusRequest::BusWrite64(request_type, _, _, _) => { *request_type }
            BusRequest::BusWrite128(request_type, _, _) => { *request_type }
        }
    }


    pub fn read(&self) -> bool {
        match self {
            BusRequest::BusRead32(_, _) => { true }
            BusRequest::BusRead64(_, _) => { true }
            BusRequest::BusRead128(_, _) => { true }
            BusRequest::BusRead256(_, _) => { true }
            BusRequest::BusWrite32(_, _, _, _) => { false }
            BusRequest::BusWrite64(_, _, _, _) => { false }
            BusRequest::BusWrite128(_, _, _) => { false }
        }
    }
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reason::Limited => write!(f, "Limited"),
            Reason::SyncRequest => write!(f, "SyncRequest"),
            Reason::BusRequest(req) => write!(f, "{}", req),
        }
    }
}
impl std::fmt::Display for BusRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BusRequest::BusRead32(RequestType::UncachedDataRead ,addr) => write!(f, "ReadData32({:#010x})", addr),
            BusRequest::BusRead32(RequestType::UncachedInstructionRead ,addr) => write!(f, "ReadInst32({:#010x})", addr),
            BusRequest::BusRead64(RequestType::UncachedDataRead, addr) => write!(f, "ReadData64({:#010x})", addr),
            BusRequest::BusRead128(RequestType::DCacheFill, addr) => write!(f, "FillDCache128({:#010x})", addr),
            BusRequest::BusRead256(RequestType::ICacheFill, addr) => write!(f, "FillICache256({:#010x})", addr),
            BusRequest::BusWrite32(RequestType::UncachedWrite, addr, data, mask) => {
                let shift = 8 * (addr & 0x4);
                let word = (data >> shift) as u32;
                let mask_val = (mask.value() >> shift) as u32;
                if mask_val == 0xffff_ffff {
                    write!(f, "Write32({:#010x}, {:#010x})", addr, word)
                } else {
                    write!(f, "Write32({:#010x}, ({:#010x} & {:?}))", addr, word, mask)
                }
            }
            BusRequest::BusWrite64(RequestType::UncachedWrite, addr, data, mask) => {
                if mask.value() == !0u64 {
                    write!(f, "Write64({:#010x}, {:#018x})", addr, data)
                } else {
                    write!(f, "Write64({:#010x}, ({:#018x} & {:#018x}))", addr, data, mask.value())
                }
            }
            BusRequest::BusWrite128(RequestType::DCacheWriteback, addr, data) => {
                write!(f, "WriteCache128({:08x}, {:?})", addr, data)
            }
            _ => panic!("Unknown BusRequest {:?}", self),
        }
    }
}

pub struct CoreRunResult {
    pub cycles: u64,
    pub reason: Reason,
}