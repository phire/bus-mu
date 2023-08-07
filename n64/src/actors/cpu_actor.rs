

/// CpuActor: Emulates the CPU and MI (Mips Interface)

use actor_framework::{Actor, Time, Handler,  OutboxSend, SchedulerResult, ActorCreate};
use super::{N64Actors, bus_actor::BusAccept, si_actor::SiActor, pi_actor::{PiActor, self}, vi_actor::ViActor, ai_actor::AiActor, rdp_actor::RdpActor};

use crate::{vr4300::{self}, actors::{bus_actor::{BusActor, BusRequest}, rsp_actor::{RspActor, self}}};

pub struct CpuActor {
    committed_time: Time,
    _cpu_overrun: u32,
    cpu_core: vr4300::Core,
    imem: Option<Box<[u32; 1024]>>,
    dmem: Option<Box<[u32; 1024]>>,
    outstanding_mem_request: Option<vr4300::Reason>,
    bus_free: Time,
    recursion: u32,
}

actor_framework::make_outbox!(
    CpuOutbox<N64Actors, CpuActor> {
        bus: BusRequest,
        run: CpuRun,
        reg_write: CpuRegWrite,
        reg: CpuRegRead,
        request_rsp_mem: rsp_actor::ReqestMemOwnership,
        return_rsp_mem: rsp_actor::TransferMemOwnership,
        pi_read: pi_actor::PiRead,
        pi_write: pi_actor::PiWrite,
        read_finished: ReadFinished,
        write_finished: WriteFinished,
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
    // TODO: Check if the logic for odd is anywhere near correct
    cpu_time - ((cpu_time + odd * 2) / 3u64)
}

enum AdvanceResult {
    Limited,
    MemRequest,
}

impl CpuActor {
    fn advance(&mut self, outbox: &mut CpuOutbox, _: CpuRun, limit: Time) -> AdvanceResult {
        let limit_64: u64 = limit.into();
        let mut commit_time_64: u64 = self.committed_time.into();

        //println!("CpuActor::advance({}, {})", limit_64, commit_time_64);

        let cycles: u64 = limit_64 - commit_time_64;

        let mut odd = commit_time_64 & 1u64;

        let mut cpu_cycles = to_cpu_time(cycles, odd);
        assert!(cycles == to_bus_time(cpu_cycles, odd), "cycles {} != cpu_cycles {} when odd = {}", cycles, cpu_cycles, odd);
        loop {
            let result = self.cpu_core.advance(to_cpu_time(cycles, odd));

            let used_cycles = to_bus_time(result.cycles, odd);
            commit_time_64 += used_cycles;
            self.committed_time = commit_time_64.into();
            //println!("core did {} ({}) cycles and returned {} at cycle {}", used_cycles, result.cycles, result.reason, commit_time_64);
            assert!(used_cycles <= cycles, "{} > {} | {}, {}", used_cycles, cycles, cpu_cycles, result.cycles);

            return match result.reason {
                vr4300::Reason::Limited => {
                    outbox.send::<CpuActor>(CpuRun {}, self.committed_time);
                    AdvanceResult::Limited
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
                    AdvanceResult::Limited
                }
                reason => {
                    // Request over C-BUS/D-BUS
                    self.start_request(outbox, reason, limit);

                    AdvanceResult::MemRequest
                }
            };
        };
    }

    fn start_request(&mut self, outbox: &mut CpuOutbox, request: vr4300::Reason, limit: Time) {
        self.outstanding_mem_request = Some(request);
        let request_time = core::cmp::max(self.bus_free, self.committed_time);

        if self.recursion < RECURSION_LIMIT && request_time < limit {
            // If nothing else needs to run before this request, we know we will win bus arbitration
            // and we can avoid the scheduler
            self.recursion += 1;

            self.recv(outbox, BusAccept{}, request_time.add(1), limit);
        } else {
            outbox.send::<BusActor>(BusRequest::new::<Self>(1), request_time);
        }
    }

    fn finish_mem(&mut self, outbox: &mut CpuOutbox, mem_finished: MemFinished, time: Time, limit: Time) -> SchedulerResult {
        let request = self.outstanding_mem_request.take().unwrap();
        //println!("CPU: Finishing {:} = {:x?} at {:}", request, mem_finished, time);

        assert!((u64::from(time) - u64::from(self.committed_time)) >= 1, "mem finished too fast");

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
        self.advance(outbox, CpuRun {  }, finish_time);

        let req_type = request.request_type();

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

        if self.recursion < RECURSION_LIMIT && limit > finish_time {
            if outbox.contains::<CpuRun>() {
                // The CPU core is ready to run (it didn't issue a new memory request)
                // Might as well run it now to save scheduler overhead
                self.recursion += 1;
                let (_ , cpurun) = outbox.cancel();
                self.advance(outbox, cpurun, limit);
            }
        }

        SchedulerResult::Ok
    }
}

impl Actor<N64Actors> for CpuActor {
    type OutboxType = CpuOutbox;

    fn message_delivered(&mut self, _outbox: &mut CpuOutbox, _time: Time) {
        self.recursion = 0;
    }
}

impl ActorCreate<N64Actors> for CpuActor {
    fn new(outbox: &mut CpuOutbox, time: Time) -> CpuActor {
        outbox.send::<CpuActor>(CpuRun {}, time);
        CpuActor {
            committed_time: Default::default(),
            _cpu_overrun: 0,
            cpu_core: Default::default(),
            imem: None,
            dmem: None,
            outstanding_mem_request: None,
            bus_free: Default::default(),
            recursion: 0,
        }
    }
}

impl Handler<N64Actors, ReadFinished> for CpuActor {
    fn recv(&mut self, outbox: &mut CpuOutbox, message: ReadFinished, time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem(outbox, MemFinished::Read(message), time, limit)
    }
}

impl Handler<N64Actors, WriteFinished> for CpuActor {
    fn recv(&mut self, outbox: &mut CpuOutbox, message: WriteFinished, time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem(outbox, MemFinished::Write(message), time, limit)
    }
}

impl Handler<N64Actors, CpuRun> for CpuActor {
    fn recv(&mut self, outbox: &mut CpuOutbox, msg: CpuRun, time: Time, limit: Time) -> SchedulerResult {
        debug_assert!(time == self.committed_time);
        if time == limit {
            outbox.send::<CpuActor>(msg, time);

            // Let the scheduler know we are zero limited
            return SchedulerResult::ZeroLimit;
        }

        self.advance(outbox, msg, limit);

        SchedulerResult::Ok
    }
}

pub struct CpuRegRead {
    pub address: u32
}

pub struct CpuRegWrite {
    pub address: u32,
    pub data: u32
}

impl CpuActor {
    fn do_reg<Dest>(&mut self, outbox: &mut CpuOutbox, reason: vr4300::Reason, time: Time)
    where
        Dest: Handler<N64Actors, CpuRegRead>
            + Handler<N64Actors, CpuRegWrite>
    {
        match reason {
            vr4300::Reason::BusRead32(_, address) => {
                outbox.send::<Dest>(CpuRegRead { address: address }, time);
            }
            vr4300::Reason::BusWrite32(_, address, data) => {
                outbox.send::<Dest>(CpuRegWrite { address: address, data: data }, time);
            }
            _ => { panic!("unexpected bus operation") }
        }
    }

    fn do_rspmem(&mut self, outbox: &mut CpuOutbox, reason: vr4300::Reason, time: Time, limit: Time) {
        let mem = match reason.address() & 0x1000 == 0 {
            true => self.dmem.as_mut(),
            false => self.imem.as_mut(),
        };

        if let Some(mem) = mem {
            let offset = ((reason.address() >> 2) & 0x3ff) as usize;

            match reason {
                vr4300::Reason::BusRead32(_, _) => {
                    let data = mem[offset];
                    self.finish_mem(outbox, MemFinished::Read(ReadFinished::word(data)), time, limit);
                }
                vr4300::Reason::BusWrite32(_, _, data) => {
                    mem[offset] = data;
                    self.finish_mem(outbox, MemFinished::Write(WriteFinished::word()), time, limit);
                }
                _ => { panic!("unexpected bus operation") }
            }
        } else {
            // The CPU doesn't currently have ownership of imem/dmem, need to request it from RspActor
            self.outstanding_mem_request = Some(reason);
            outbox.send::<RspActor>(rsp_actor::ReqestMemOwnership {}, time)
        }
    }
}

impl Handler<N64Actors, BusAccept> for CpuActor {
    fn recv(&mut self, outbox: &mut CpuOutbox, _: BusAccept, time: Time, limit: Time) -> SchedulerResult {
        let reason = self.outstanding_mem_request.clone().unwrap();
        let address = reason.address();

        match address & 0xfff0_0000 {
            0x0000_0000..=0x03ff_ffff => { // RDRAM
                todo!("RDRAM")
            }
            0x0400_0000 => match address & 0x040c_0000 { // RSP
                0x0400_0000 if address & 0x1000 == 0 => { // DMEM Direct access
                    //println!("DMEM access {}", reason);
                    self.do_rspmem(outbox, reason, time, limit);
                }
                0x0400_0000 if address & 0x1000 != 0 => { // IMEM Direct access
                    //println!("IMEM access {}", reason);
                    self.do_rspmem(outbox, reason, time, limit);
                }
                0x0404_0000 | 0x0408_0000 => { // RSP Register
                    self.do_reg::<RspActor>(outbox, reason, time);
                }
                0x040c_0000 => { // Unmapped {
                    todo!("Unmapped")
                }
                _ => unreachable!()
            }
            0x0410_0000 => { // RDP Command Regs
                self.do_reg::<RdpActor>(outbox, reason, time);
            }
            0x0420_0000 => {
                todo!("RDP Span Regs")
            }
            0x0430_0000 => {
                todo!("MIPS Interface")
            }
            0x0440_0000 => { // Video Interface
                self.do_reg::<ViActor>(outbox, reason, time);
            }
            0x0450_0000 => { // Audio Interface
                self.do_reg::<AiActor>(outbox, reason, time);
            }
            0x0460_0000 => { // Peripheral Interface
                self.do_reg::<PiActor>(outbox, reason, time);
            }
            0x0470_0000 => {
                todo!("RDRAM Interface")
            }
            0x0480_0000 => { // Serial Interface
                self.do_reg::<SiActor>(outbox, reason, time);
            }
            0x0490_0000..=0x04ff_ffff => {
                todo!("Unmapped")
            }
            0x1fc0_0000 => { // SI External Bus
                self.do_reg::<SiActor>(outbox, reason, time);
            }
            0x0500_0000..=0x7fff_0000 => { // PI External bus
                match reason {
                    vr4300::Reason::BusRead32(_, _) => {
                        outbox.send::<PiActor>(pi_actor::PiRead::new(address), time);
                    }
                    vr4300::Reason::BusWrite32(_, _, data) => {
                        outbox.send::<PiActor>(pi_actor::PiWrite::new(address, data), time);
                    }
                    _ => { panic!("unexpected bus operation") }
                }
            }
            0x8000_0000..=0xffff_ffff => {
                todo!("Unmapped")
            }
            _ => unreachable!()
        }
        SchedulerResult::Ok
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CpuLength {
    Word = 1,
    Dword = 2,
    QWord = 4,
    OctWord = 8,
}

#[derive(Debug)]
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
            length: CpuLength::QWord,
            data: [data[0], data[1], data[2], data[3], 0, 0, 0, 0]
        }
    }
    pub fn octword(data: [u32; 8]) -> Self {
        Self {
            length: CpuLength::OctWord,
            data
        }
    }

    pub fn length(&self) -> u64 {
        self.length as u64
    }
}

#[derive(Debug)]
pub struct WriteFinished {
    length: CpuLength
}

#[derive(Debug)]
enum MemFinished {
    Read(ReadFinished),
    Write(WriteFinished)
}

impl WriteFinished {
    pub fn word() -> Self {
        Self {
            length: CpuLength::Word
        }
    }
    pub fn dword() -> Self {
        Self {
            length: CpuLength::Dword
        }
    }
    pub fn qword() -> Self {
        Self {
            length: CpuLength::QWord
        }
    }
    pub fn octword() -> Self {
        Self {
            length: CpuLength::OctWord
        }
    }

    pub fn length(&self) -> u64 {
        self.length as u64
    }
}

impl Handler<N64Actors, rsp_actor::TransferMemOwnership> for CpuActor {
    fn recv(&mut self, outbox: &mut CpuOutbox, message: rsp_actor::TransferMemOwnership, time: Time, limit: Time) -> SchedulerResult {
        self.imem = Some(message.imem);
        self.dmem = Some(message.dmem);

        // We can now complete the memory request to imem or dmem
        let reason = self.outstanding_mem_request.clone().unwrap();
        self.do_rspmem(outbox, reason, time, limit);

        SchedulerResult::Ok
    }
}

impl Handler<N64Actors, rsp_actor::ReqestMemOwnership> for CpuActor {
    fn recv(&mut self, outbox: &mut CpuOutbox, _: rsp_actor::ReqestMemOwnership, time: Time, _limit: Time) -> SchedulerResult {
        let msg = rsp_actor::TransferMemOwnership {
            imem: self.imem.take().unwrap(),
            dmem: self.imem.take().unwrap(),
        };

        outbox.send::<RspActor>(msg, time.add(4));

        SchedulerResult::Ok
    }
}
