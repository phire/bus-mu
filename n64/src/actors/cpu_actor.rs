

/// CpuActor: Emulates the CPU and MI (Mips Interface)

use actor_framework::{Actor, Time, Handler,  OutboxSend, SchedulerResult, ActorCreate};
use super::{N64Actors, pi_actor, bus_actor::{BusPair, ReturnBus, request_bus}};

use vr4300;

use crate::{actors::bus_actor::{BusActor, BusRequest}, c_bus::{self, CBus}, d_bus::DBus};

pub struct CpuActor {
    committed_time: Time,
    _cpu_overrun: u32,
    pub cpu_core: vr4300::Core,
    outstanding_mem_request: Option<vr4300::BusRequest>,
    req_type: Option<vr4300::RequestType>,
    bus_free: Time,
    bus: Option<Box<BusPair>>,
    /// tracks how many times `CpuActor::advance` has been called recursively
    /// (It can recurse when we can complete a memory request internally)
    recursion: u32,
}

actor_framework::make_outbox!(
    CpuOutbox<N64Actors, CpuActor> {
        bus: BusRequest,
        bus_return: Box<BusPair>,
        run: CpuRun,
        reg_write: c_bus::CBusWrite,
        reg: c_bus::CBusRead,
        request_resource: c_bus::ResourceRequest,
        return_resource: c_bus::Resource,
        pi_read: pi_actor::PiRead,
        pi_write: pi_actor::PiWrite,
    }
);

const RECURSION_LIMIT: u32 = 100;

struct CpuRun {}

fn to_cpu_time(bus_time: u64, odd: u64) -> u64 {
    // CPU has a 1.5x clock multiplier

    // We use the bottom bit of the absolute time (odd) so our extra cycles always
    // happen deterministically on the odd cycle of the primary system clock
    bus_time.saturating_add(bus_time / 2u64 + odd)
}

fn to_bus_time(cpu_time: u64, odd: u64) -> u64 {
    // CPU has a 1.5x clock multiplier
    cpu_time - ((cpu_time + odd * 2) / 3u64)
}

impl CpuActor {
    fn advance(&mut self, outbox: &mut CpuOutbox, _: CpuRun, limit: Time) -> SchedulerResult {
        let limit_64: u64 = limit.into();
        let mut commit_time_64: u64 = self.committed_time.into();

        //println!("CpuActor::advance({}, {})", limit_64, commit_time_64);

        let cycles: u64 = limit_64 - commit_time_64;

        let mut odd = commit_time_64 & 1u64;

        let mut cpu_cycles = to_cpu_time(cycles, odd);
        //assert!(cycles == to_bus_time(cpu_cycles, odd), "cycles {} != cpu_cycles {} when odd = {}", cycles, cpu_cycles, odd);
        loop {
            let result = self.cpu_core.advance(to_cpu_time(cycles, odd));

            let used_cycles = to_bus_time(result.cycles, odd);
            commit_time_64 += used_cycles;
            self.committed_time = commit_time_64.into();
            //println!("core did {} ({}) cycles and returned {} at cycle {}", used_cycles, result.cycles, result.reason, commit_time_64);
            assert!(used_cycles <= cycles, "{} > {} | {}, {}", used_cycles, cycles, cpu_cycles, result.cycles);

            match result.reason {
                vr4300::Reason::Limited => {
                    outbox.send::<CpuActor>(CpuRun {}, self.committed_time);
                }
                vr4300::Reason::SyncRequest => {
                    assert!(limit.is_resolved());
                    self.cpu_core.set_time(commit_time_64);

                    cpu_cycles -= result.cycles;
                    if cpu_cycles > 0 {
                        odd = commit_time_64 & 1u64;
                        continue;
                    }

                    outbox.send::<CpuActor>(CpuRun {}, self.committed_time);
                }
                vr4300::Reason::BusRequest(request) => {
                    // Request over C-BUS/D-BUS
                    return self.start_request(outbox, request, limit);
                }
            };
            return SchedulerResult::Ok;
        };
    }

    fn start_c_bus(&mut self, outbox: &mut CpuOutbox, reason: vr4300::BusRequest, time: Time, limit: Time) -> SchedulerResult {
        use c_bus::RegBusResult::*;
        use vr4300::BusRequest::*;

        let bus = self.bus.as_mut().unwrap();

        match reason {
            BusRead32(req_type, address) => {
                match bus.c_bus.cpu_read(outbox, address, time) {
                    ReadCompleted(data) => self.finish_read32(outbox, req_type, data, time, limit),
                    WriteCompleted => unreachable!(),
                    Unmapped => todo!("Unmapped"),
                    Dispatched => {
                        self.req_type = Some(req_type);
                        SchedulerResult::Ok
                    }
                }
            }
            BusWrite32(_, address, data) => {
                match bus.c_bus.cpu_write(outbox, address, data, time) {
                    WriteCompleted => self.finish_write32(outbox, time, limit),
                    ReadCompleted(_) => unreachable!(),
                    Unmapped => todo!("Unmapped"),
                    Dispatched => SchedulerResult::Ok,
                }
            }
            _ => todo!("Wrong request type")
        }
    }

    fn start_request(&mut self, outbox: &mut CpuOutbox, request: vr4300::BusRequest, limit: Time) -> SchedulerResult {
        let request_time = core::cmp::max(self.bus_free, self.committed_time);

        match self.bus {
            None => {
                self.outstanding_mem_request = Some(request);
                request_bus(outbox, limit)
            }
            Some(_) => {
                if request.address() < 0x0400_0000 {
                    todo!("RAMBUS");
                } else {
                    self.start_c_bus(outbox, request, request_time, limit)
                }
            }
        }
    }

    /// The generic memory finish function
    /// Don't call this directly, call one of the specialized version instead
    #[inline(always)]
    fn finish_mem_unspecialised(&mut self, outbox: &mut CpuOutbox, req_type: vr4300::RequestType, mem_finished: MemFinished, time: Time, limit: Time) -> SchedulerResult {
        //println!("CPU: Finishing {:} = {:x?} at {:}", request, mem_finished, time);

        //assert!((u64::from(time) - u64::from(self.committed_time)) >= 1, "mem finished too fast");

        let finish_time = match &mem_finished {
            MemFinished::Read(message) => {
                // It takes length cycles to receive the data across the SysAD bus
                time.add(message.length())
            }
            MemFinished::Write(_) => {
                time // No data to transfer back
            }
        };

        self.bus_free = finish_time;

        // Advance the CPU upto the finish time
        let catchup_result = self.advance(outbox, CpuRun {  }, finish_time);

        match &mem_finished {
            MemFinished::Read(message) => {
                let new_req = self.cpu_core.finish_read(req_type, &message.data, message.length());

                if let Some(new_req) = new_req {
                    assert!(self.outstanding_mem_request.is_none());
                    self.start_request(outbox, new_req, limit);
                }
            }
            MemFinished::Write(message) => {
                self.cpu_core.finish_write(req_type, message.length());
            }
        }

        let can_recurse = self.recursion < RECURSION_LIMIT && limit > finish_time && outbox.contains::<CpuRun>();

        match catchup_result {
            SchedulerResult::Ok if can_recurse => {  }
            _ => { return catchup_result; }
        };

        // The CPU core is ready to run (it didn't issue a new memory request)
        // Might as well run it now to save scheduler overhead
        self.recursion += 1;
        let (_ , cpurun) = outbox.cancel();
        return self.advance(outbox, cpurun, limit);
    }

    #[inline(always)]
    #[allow(unused)]
    fn debug_check_finish_mem(req_type: vr4300::RequestType, mem_finished: MemFinished) {
        use vr4300::RequestType::*;
        match mem_finished {
            MemFinished::Read(ReadFinished{ length, data: _}) => match length {
                CpuLength::Word =>
                    debug_assert!(req_type == UncachedDataRead || req_type == UncachedInstructionRead),
                CpuLength::Dword => debug_assert!(req_type == UncachedDataRead),
                CpuLength::Qword => debug_assert!(req_type == DCacheFill),
                CpuLength::Octword => debug_assert!(req_type == ICacheFill),
            }
            MemFinished::Write(WriteFinished { length }) => {
                match length {
                    CpuLength::Word => debug_assert!(req_type == UncachedWrite),
                    CpuLength::Dword => debug_assert!(req_type == UncachedWrite),
                    CpuLength::Qword => debug_assert!(req_type == DCacheWriteback),
                    CpuLength::Octword => debug_assert!(false),
                }
            }
        }
    }

    #[inline(never)]
    fn finish_read32(&mut self, outbox: &mut CpuOutbox, req_type: vr4300::RequestType, data: u32, time: Time, limit: Time) -> SchedulerResult {
        match req_type {
            vr4300::RequestType::UncachedDataRead | vr4300::RequestType::UncachedInstructionRead => {
                self.finish_mem_unspecialised(outbox, req_type, MemFinished::Read(ReadFinished::word(data)), time, limit)
            }
            _ => { unreachable!() }
        }
    }

    #[inline(never)]
    fn finish_read64(&mut self, outbox: &mut CpuOutbox, data: [u32; 2], time: Time, limit: Time) -> SchedulerResult {
        let finished = ReadFinished { length: CpuLength::Dword, data: [data[0], data[1], 0, 0, 0, 0, 0, 0] };
        self.finish_mem_unspecialised(outbox, vr4300::RequestType::UncachedDataRead, MemFinished::Read(finished), time, limit)
    }

    #[inline(never)]
    fn finish_read128(&mut self, outbox: &mut CpuOutbox, data: &[u32; 4], time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem_unspecialised(outbox, vr4300::RequestType::DCacheFill, MemFinished::Read(ReadFinished::qword(*data)), time, limit)
    }

    #[inline(never)]
    fn finish_read256(&mut self, outbox: &mut CpuOutbox, data: &[u32; 8], time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem_unspecialised(outbox, vr4300::RequestType::ICacheFill, MemFinished::Read(ReadFinished::octword(*data)), time, limit)
    }

    #[inline(never)]
    fn finish_write32(&mut self, outbox: &mut CpuOutbox, time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem_unspecialised(outbox, vr4300::RequestType::UncachedWrite, MemFinished::Write(WriteFinished::word()), time, limit)
    }

    #[inline(never)]
    fn finish_write64(&mut self, outbox: &mut CpuOutbox, time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem_unspecialised(outbox, vr4300::RequestType::UncachedWrite, MemFinished::Write(WriteFinished::dword()), time, limit)
    }

    #[inline(never)]
    fn finish_write128(&mut self, outbox: &mut CpuOutbox, time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem_unspecialised(outbox, vr4300::RequestType::DCacheWriteback, MemFinished::Write(WriteFinished::qword()), time, limit)
    }

}

impl Actor<N64Actors> for CpuActor {
    type OutboxType = CpuOutbox;
}

impl ActorCreate<N64Actors> for CpuActor {
    fn new(outbox: &mut CpuOutbox, time: Time) -> CpuActor {
        outbox.send::<CpuActor>(CpuRun {}, time);
        CpuActor {
            committed_time: Default::default(),
            _cpu_overrun: 0,
            cpu_core: Default::default(),
            outstanding_mem_request: None,
            bus: Some(Box::new(BusPair { c_bus: CBus::new(), d_bus: DBus::new() })),
            req_type: None,
            bus_free: Default::default(),
            recursion: 0,
        }
    }
}

impl Handler<N64Actors, ReadFinished> for CpuActor {
    #[inline(always)]
    fn recv(&mut self, outbox: &mut CpuOutbox, message: ReadFinished, time: Time, limit: Time) -> SchedulerResult {
        self.recursion = 0; // Reset recursion

        // PERF: Should we put outstanding_mem_request inside the ReadFinshed message?
        //       That would save the None check and panic
        //Self::debug_check_finish_mem(request.request_type(), MemFinished::Read(message.clone()));

        // PERF: We should do separate handlers for each length, save the dispatch
        match message.length {
            CpuLength::Word => {
                let req_type = self.req_type.take().unwrap();
                self.finish_read32(outbox, req_type, message.data[0], time, limit)
            }
            CpuLength::Dword => {
                self.finish_read64(outbox, message.data[0..1].try_into().unwrap(), time, limit)
            }
            CpuLength::Qword => {
                self.finish_read128(outbox, message.data[0..4].try_into().unwrap(), time, limit)
            }
            CpuLength::Octword => {
                self.finish_read256(outbox, &message.data, time, limit)
            }
        }
    }
}

impl Handler<N64Actors, WriteFinished> for CpuActor {
    #[inline(always)]
    fn recv(&mut self, outbox: &mut CpuOutbox, message: WriteFinished, time: Time, limit: Time) -> SchedulerResult {
        self.recursion = 0; // Reset recursion

        // PERF: We should do separate handlers for each length, save the dispatch
        match message.length {
            CpuLength::Word => self.finish_write32(outbox, time, limit),
            CpuLength::Dword => self.finish_write64(outbox, time, limit),
            CpuLength::Qword => self.finish_write128(outbox, time, limit),
            CpuLength::Octword => unreachable!()
        }
    }
}

impl Handler<N64Actors, CpuRun> for CpuActor {
    #[inline(always)]
    fn recv(&mut self, outbox: &mut CpuOutbox, msg: CpuRun, time: Time, limit: Time) -> SchedulerResult {
        debug_assert!(time == self.committed_time);
        if time == limit {
            outbox.send::<CpuActor>(msg, time);

            // Let the scheduler know we are zero limited
            return SchedulerResult::ZeroLimit;
        }

        self.recursion = 0; // Reset recursion
        self.advance(outbox, msg, limit)
    }
}

impl Handler<N64Actors, Box<BusPair>> for CpuActor {
    fn recv(&mut self, outbox: &mut CpuOutbox, bus: Box<BusPair>, time: Time, limit: Time) -> SchedulerResult {
        let request = self.outstanding_mem_request.clone().unwrap();

        self.recursion = 0;
        self.bus = Some(bus);
        if request.address() < 0x0400_0000 {
            todo!("RAMBUS")
        } else {
            self.start_c_bus(outbox, request, time, limit)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CpuLength {
    Word = 1,
    Dword = 2,
    Qword = 4,
    Octword = 8,
}

#[derive(Debug, Clone, Copy)]
pub struct ReadFinished {
    length: CpuLength,
    pub data: [u32; 8]
}

impl ReadFinished {
    pub fn word(data: u32) -> Self {
        Self {
            length: CpuLength::Word,
            data: [data, 0, 0, 0, 0, 0, 0, 0]
        }
    }
    pub fn dword(data: u64) -> Self {
        Self {
            length: CpuLength::Dword,
            data: [(data >> 32) as u32, data as u32, 0, 0, 0, 0, 0, 0]
        }
    }
    pub fn qword(data: [u32; 4]) -> Self {
        Self {
            length: CpuLength::Qword,
            data: [data[0], data[1], data[2], data[3], 0, 0, 0, 0]
        }
    }
    pub fn octword(data: [u32; 8]) -> Self {
        Self {
            length: CpuLength::Octword,
            data
        }
    }

    pub fn length(&self) -> u64 {
        self.length as u64
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WriteFinished {
    length: CpuLength
}

#[derive(Debug)]
enum MemFinished {
    Read(ReadFinished),
    Write(WriteFinished)
}

impl WriteFinished {
    pub fn word() -> Self { Self { length: CpuLength::Word }}
    pub fn dword() -> Self { Self { length: CpuLength::Dword } }
    pub fn qword() -> Self { Self { length: CpuLength::Qword } }
    pub fn octword() -> Self { Self { length: CpuLength::Octword } }
    pub fn length(&self) -> u64 { self.length as u64 }
}

impl Handler<N64Actors, c_bus::Resource> for CpuActor {
    fn recv(&mut self, outbox: &mut CpuOutbox, resource: c_bus::Resource, time: Time, limit: Time) -> SchedulerResult {
        use c_bus::RegBusResult;

        self.recursion = 0; // Reset recursion
        let bus = self.bus.as_mut().expect("Should own Bus");

        // If a resource was requested, we must own c_bus
        match bus.c_bus.receive_resource(outbox, resource, time) {
            RegBusResult::WriteCompleted => {
                self.finish_write32(outbox, time, limit)
            }
            RegBusResult::ReadCompleted(data) => {
                let req_type = self.req_type.take().unwrap();
                self.finish_read32(outbox, req_type, data, time, limit)
            }
            RegBusResult::Dispatched => {
                SchedulerResult::Ok
            }
            RegBusResult::Unmapped => {
                todo!("Unmapped")
            }
        }
    }
}

impl Handler<N64Actors, ReturnBus> for CpuActor {
    fn recv(&mut self, outbox: &mut Self::OutboxType, _: ReturnBus, time: Time, _limit: Time) -> SchedulerResult {
        outbox.send::<BusActor>(self.bus.take().unwrap(), time)
    }
}

impl Handler<N64Actors, c_bus::ResourceReturnRequest> for CpuActor {
    fn recv(&mut self, outbox: &mut CpuOutbox, request: c_bus::ResourceReturnRequest, time: Time, _limit: Time) -> SchedulerResult {
        let bus = self.bus.as_mut().expect("Should own CBus");
        bus.c_bus.return_resource(outbox, request, time);

        SchedulerResult::Ok
    }
}

impl CpuActor {
    pub fn get_core(&mut self) -> &mut vr4300::Core {
        &mut self.cpu_core
    }
}
