

/// CpuActor: Emulates the CPU and MI (Mips Interface)

use actor_framework::{Actor, Time, Handler,  OutboxSend, SchedulerResult, ActorInit, Outbox};
use super::{N64Actors, pi_actor, bus_actor::{BusPair, ReturnBus, request_bus}};

use vr4300::{self, RequestType};

use crate::{actors::bus_actor::{BusActor, BusRequest}, c_bus::{self, CBus}, d_bus::DBus, N64Config};

pub struct CpuActor {
    committed_time: Time,
    _cpu_overrun: u32,
    pub cpu_core: vr4300::Core,
    outstanding_mem_request: Option<vr4300::BusRequest>,
    c_bus_req: Option<(u32, vr4300::RequestType)>,
    bus_free: Time,
    bus: Option<Box<BusPair>>,
    /// tracks how many times `CpuActor::advance` has been called recursively
    /// (It can recurse when we can complete a memory request internally)
    recursion: u32,
    interrupted_msg: CpuOutbox,
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

    fn start_c_bus(&mut self, outbox: &mut CpuOutbox, request: vr4300::BusRequest, time: Time, limit: Time) -> SchedulerResult {
        use c_bus::RegBusResult::*;
        use vr4300::BusRequest::*;

        let bus = self.bus.as_mut().unwrap();

        match request {
            BusRead32(req_type, address) => {
                let shift = (!address & 0x4) * 8;
                match bus.c_bus.cpu_read(outbox, address, time) {
                    ReadCompleted(data) => {
                        let data = (data as u64) << shift;
                        self.finish_read32(outbox, req_type, data, time, limit)
                    }
                    WriteCompleted => unreachable!(),
                    Unmapped => todo!("Unmapped"),
                    Dispatched => {
                        self.c_bus_req = Some((shift, req_type));
                        SchedulerResult::Ok
                    }
                }
            }
            BusWrite32(_, address, data, _mask) => {
                // cbus discards the mask.
                // The cpu pipeline has already shifted the data to the correct position within an
                // aligned 64bit double, but only the critical word is send over the SYSAD bus.
                // We need to implement that here.
                let shift = (!address & 0x4) * 8;
                let word = (data >> shift) as u32;
                match bus.c_bus.cpu_write(outbox, address, word, time) {
                    WriteCompleted => self.finish_write32(outbox, time, limit),
                    ReadCompleted(_) => unreachable!(),
                    Unmapped => todo!("Unmapped"),
                    Dispatched => SchedulerResult::Ok,
                }
            }
            _ => {
                // These probably all cause the CPU to lock-up, as CBUS will only ever acknowledge a word
                todo!("Wrong request type for CBus, {:}", request)
            }
        }
    }

    fn start_d_bus(&mut self, outbox: &mut CpuOutbox, request: vr4300::BusRequest, time: Time, limit: Time) -> SchedulerResult {
        use vr4300::BusRequest::*;

        let d_bus = &mut self.bus.as_mut().unwrap().d_bus;

        match request {
            BusRead32(req_type, addr) => {
                let (cycles, data) = d_bus.read_qword(addr & !0x7);
                self.finish_read32(outbox, req_type, data, time.add(cycles), limit)
            }
            BusRead64(_, addr) => {
                let (cycles, data) = d_bus.read_qword(addr & !0x7);
                self.finish_read64(outbox, data, time.add(cycles), limit)
            }
            BusRead128(_, addr) => {
                let _align = addr & 0x7;
                let addr = addr & !0x7;
                // TODO: We need to calculate critical word first timings
                // HWTEST: does MI always wait for both words to be ready?
                //         I think the SYSAD bus wants all 128bits at once
                let (cycles1, lower) = d_bus.read_qword(addr);
                let (cycles2, upper) = d_bus.read_qword(addr.wrapping_add(8));
                let time = time.add(cycles1 + cycles2);
                let data = [lower, upper];

                self.finish_read128(outbox, &data, time, limit)
            }
            BusRead256(_, addr) => {
                let mut buffer = [0; 4];
                let mut total_cycles = 0;

                // The VR4300 only does 256bit aligned reads, so critical word first doesn't matter
                assert!(addr & 0x1f == 0);
                for i in 0..4 {
                    let (cycles, data) = d_bus.read_qword(addr & !0x7 + i * 8);
                    buffer[i as usize] = data;
                    total_cycles += cycles;
                }
                self.finish_read256(outbox, &buffer, time.add(total_cycles), limit)
            }
            BusWrite32(_, addr, data, mask) => {
                let cycles = d_bus.write_qword_masked(addr & !0x7, data, mask);
                self.finish_write32(outbox, time.add(cycles), limit)
            }
            BusWrite64(_, addr, data, mask) => {
                // mask is used for Store Double Left/Store Double Right
                let cycles = d_bus.write_qword_masked(addr & !0x7, data, mask);
                self.finish_write64(outbox, time.add(cycles), limit)
            }
            BusWrite128(_, addr, data) => {
                // Writes are always 128 bit aligned
                assert!(addr & 0xf == 0);
                let mut cycles = d_bus.write_qword(addr, data[0]);
                cycles += d_bus.write_qword(addr + 8, data[1]);
                self.finish_write128(outbox, time.add(cycles), limit)
            }
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
                    self.start_d_bus(outbox, request, request_time, limit)
                } else {
                    self.start_c_bus(outbox, request, request_time, limit)
                }
            }
        }
    }

    /// The generic memory finish function
    /// Don't call this directly, call one of the specialized version instead
    #[inline(always)]
    fn finish_mem_unspecialised(&mut self, outbox: &mut CpuOutbox, req_type: vr4300::RequestType, data: &[u64], transfers: usize, time: Time, limit: Time) -> SchedulerResult {
        //println!("CPU: Finishing {:} = {:x?} at {:}", request, mem_finished, time);

        //assert!((u64::from(time) - u64::from(self.committed_time)) >= 1, "mem finished too fast");

        let write = match req_type {
            RequestType::UncachedWrite | RequestType::DCacheWriteback => true,
            _ => false,
        };

        let finish_time = if write {
            time // No data to transfer back
        } else {
            // It takes length cycles to receive the data across the SysAD bus
            time.add(transfers as u64)
        };

        self.bus_free = finish_time;

        // Advance the CPU upto the finish time
        // FIXME: The vr4300 actually stalls and doesn't need to do any work...
        //        Except in caches where this request wasn't triggered by an interlock
        let catchup_result = self.advance(outbox, CpuRun {  }, finish_time);

        if write {
            self.cpu_core.finish_write(req_type, transfers);
        } else {
            let new_req = self.cpu_core.finish_read(req_type, data, transfers);

            if let Some(new_req) = new_req {
                assert!(self.outstanding_mem_request.is_none());
                self.start_request(outbox, new_req, limit);
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

    #[inline(never)]
    fn finish_read32(&mut self, outbox: &mut CpuOutbox, req_type: vr4300::RequestType, data: u64, time: Time, limit: Time) -> SchedulerResult {
        match req_type {
            vr4300::RequestType::UncachedDataRead | vr4300::RequestType::UncachedInstructionRead => {
                self.finish_mem_unspecialised(outbox, req_type, &[data], 1, time, limit)
            }
            _ => { unreachable!() }
        }
    }

    #[inline(never)]
    fn finish_read64(&mut self, outbox: &mut CpuOutbox, data: u64, time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem_unspecialised(outbox, vr4300::RequestType::UncachedDataRead, &[data], 2, time, limit)
    }

    #[inline(never)]
    fn finish_read128(&mut self, outbox: &mut CpuOutbox, data: &[u64; 2], time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem_unspecialised(outbox, vr4300::RequestType::DCacheFill, data, 4, time, limit)
    }

    #[inline(never)]
    fn finish_read256(&mut self, outbox: &mut CpuOutbox, data: &[u64; 4], time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem_unspecialised(outbox, vr4300::RequestType::ICacheFill, data, 8, time, limit)
    }

    #[inline(never)]
    fn finish_write32(&mut self, outbox: &mut CpuOutbox, time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem_unspecialised(outbox, vr4300::RequestType::UncachedWrite, &[0; 0], 1, time, limit)
    }

    #[inline(never)]
    fn finish_write64(&mut self, outbox: &mut CpuOutbox, time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem_unspecialised(outbox, vr4300::RequestType::UncachedWrite, &[0; 0], 2, time, limit)
    }

    #[inline(never)]
    fn finish_write128(&mut self, outbox: &mut CpuOutbox, time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem_unspecialised(outbox, vr4300::RequestType::DCacheWriteback, &[0; 0], 4, time, limit)
    }

}

impl Actor<N64Actors> for CpuActor {
    type OutboxType = CpuOutbox;

    fn delivering<Message>(&mut self, outbox: &mut Self::OutboxType, _: &Message, _: Time)
        where
            Message: 'static,
    {
        if std::any::TypeId::of::<Message>() == std::any::TypeId::of::<Box<BusPair>>() {
            outbox.restore(&mut self.interrupted_msg);
        }
    }
}

impl ActorInit<N64Actors> for CpuActor {
    fn init(_config: &N64Config, outbox: &mut CpuOutbox, time: Time) -> Result<CpuActor, anyhow::Error> {
        outbox.send::<CpuActor>(CpuRun {}, time);
        Ok(CpuActor {
            committed_time: Default::default(),
            _cpu_overrun: 0,
            cpu_core: Default::default(),
            outstanding_mem_request: None,
            bus: Some(Box::new(BusPair { c_bus: CBus::new(), d_bus: DBus::new() })),
            c_bus_req: None,
            bus_free: Default::default(),
            recursion: 0,
            interrupted_msg: Default::default(),
        })
    }
}

impl Handler<N64Actors, c_bus::ReadFinished> for CpuActor {
    #[inline(always)]
    fn recv(&mut self, outbox: &mut CpuOutbox, message: c_bus::ReadFinished, time: Time, limit: Time) -> SchedulerResult {
        self.recursion = 0; // Reset recursion

        // PERF: Should we put c_bus_req inside the ReadFinshed message?
        //       That would save the None check and panic

        let (shift, req_type) = self.c_bus_req.take().unwrap();
        let data = (message.data as u64) << shift;
        self.finish_read32(outbox, req_type, data, time, limit)
    }
}

impl Handler<N64Actors, c_bus::WriteFinished> for CpuActor {
    #[inline(always)]
    fn recv(&mut self, outbox: &mut CpuOutbox, _: c_bus::WriteFinished, time: Time, limit: Time) -> SchedulerResult {
        self.recursion = 0; // Reset recursion

        self.finish_write32(outbox, time, limit)
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
            self.start_d_bus(outbox, request, time, limit)
        } else {
            self.start_c_bus(outbox, request, time, limit)
        }
    }
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
                let (shift, req_type) = self.c_bus_req.take().unwrap();
                let data = (data as u64) << shift;
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
        outbox.stash(&mut self.interrupted_msg);

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
