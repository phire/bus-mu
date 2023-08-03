
/// CpuActor: Emulates the CPU and MI (Mips Interface)

use actor_framework::{Actor, Time, Handler, Outbox, OutboxSend, SchedulerResult};
use super::{N64Actors, bus_actor::BusAccept, si_actor::SiActor, pi_actor::{PiActor, self}, vi_actor::ViActor, ai_actor::AiActor, rdp_actor::RdpActor};

use crate::{vr4300::{self}, actors::{bus_actor::{BusActor, BusRequest}, rsp_actor::{RspActor, self}}};

pub struct CpuActor {
    outbox: CpuOutbox,
    committed_time: Time,
    _cpu_overrun: u32,
    cpu_core: vr4300::Core,
    imem: Option<Box<[u32; 512]>>,
    dmem: Option<Box<[u32; 512]>>,
    outstanding_mem_request: Option<vr4300::Reason>,
    bus_free: Time,
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
    }
);

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
    cpu_time - ((cpu_time + odd) / 3u64)
}

impl Default for CpuActor {
    fn default() -> Self {
        let mut outbox : CpuOutbox = Default::default();
        outbox.send::<CpuActor>(CpuRun {}, Default::default());

        CpuActor {
            outbox,
            committed_time: Default::default(),
            _cpu_overrun: 0,
            cpu_core: Default::default(),
            imem: None,
            dmem: None,
            outstanding_mem_request: None,
            bus_free: Default::default(),
        }
    }
}

enum AdvanceResult {
    Limited,
    MemRequest,
}

impl CpuActor {
    fn advance(&mut self, limit: Time) -> AdvanceResult {
        let limit_64: u64 = limit.into();
        //println!("CpuActor::advance({})", limit_64);
        let mut commit_time_64: u64 = self.committed_time.into();
        let cycles: u64 = limit_64 - commit_time_64;

        let mut odd = commit_time_64 & 1u64;

        let mut cpu_cycles = to_cpu_time(cycles, odd);
        loop {
            let result = self.cpu_core.advance(to_cpu_time(cycles, odd));

            let used_cycles = to_bus_time(result.cycles, odd);
            commit_time_64 += used_cycles;
            self.committed_time = commit_time_64.into();
            println!("core did {} ({}) cycles and returned {} at cycle {}", used_cycles,  result.cycles, result.reason, commit_time_64);
            assert!(used_cycles <= cycles);

            return match result.reason {
                vr4300::Reason::Limited => {
                    self.outbox.send::<CpuActor>(CpuRun {}, self.committed_time);
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

                    self.outbox.send::<CpuActor>(CpuRun {}, self.committed_time);
                    AdvanceResult::Limited
                }
                reason => {
                    // Request over C-BUS/D-BUS
                    self.outstanding_mem_request = Some(reason);
                    let request_time = core::cmp::max(self.bus_free, self.committed_time);
                    // TODO: handle bus transfer times

                    self.outbox.send::<BusActor>(BusRequest::new::<Self>(1), request_time);
                    AdvanceResult::MemRequest
                }
            };
        };
    }

    fn finish_mem(&mut self, mem_finished: MemFinished, time: Time, limit: Time) -> SchedulerResult {
        let request = self.outstanding_mem_request.take().unwrap();
        println!("CPU: Finishing {:} at {:}", request, time);

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
        self.advance(time);

        let req_type = request.request_type();

        match &mem_finished {
            MemFinished::Read(message) => self.cpu_core.finish_read(req_type, &message.data, message.length()),
            MemFinished::Write(message) => self.cpu_core.finish_write(req_type, message.length()),
        }

        return match &self.outstanding_mem_request {
            Some(new_request) => {
                println!("CPU: Finished {:}, but CPU issued new request {:}", request, new_request);
                // The CPU core issued another memory request... let the scheduler handle it
                SchedulerResult::Ok
            }
            None => {
                println!("CPU: Finished {:}", request);
                // Otherwise, run the CPU now; Avoid scheduler overhead
                let result = self.recv(CpuRun{}, time, limit);
                self.message_delivered(time);

                result
            }
        }
    }
}

impl Actor<N64Actors> for CpuActor {
    fn get_message(&mut self) -> &mut actor_framework::MessagePacketProxy<N64Actors> {
        self.outbox.as_mut()
    }

    fn message_delivered(&mut self, _time: Time) {
        // hmmm....
    }
}


impl Handler<ReadFinished> for CpuActor {
    fn recv(&mut self, message: ReadFinished, time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem(MemFinished::Read(message), time, limit)
    }
}

impl Handler<WriteFinished> for CpuActor {
    fn recv(&mut self, message: WriteFinished, time: Time, limit: Time) -> SchedulerResult {
        self.finish_mem(MemFinished::Write(message), time, limit)
    }
}

impl Handler<CpuRun> for CpuActor {
    fn recv(&mut self, _: CpuRun, time: Time, limit: Time) -> SchedulerResult {
        debug_assert!(time == self.committed_time);
        if time == limit {
            self.outbox.send::<CpuActor>(CpuRun {}, time);

            // Let the scheduler know we are zero limited
            return SchedulerResult::ZeroLimit;
        }

        self.advance(limit);

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
    fn do_reg<Dest>(&mut self, reason: vr4300::Reason, time: Time)
    where
        Dest: Actor<N64Actors> + Handler<CpuRegRead> + Handler<CpuRegWrite>
    {
        match reason {
            vr4300::Reason::BusRead32(_, address) => {
                self.outbox.send::<Dest>(CpuRegRead { address: address }, time);
            }
            vr4300::Reason::BusWrite32(_, address, data) => {
                self.outbox.send::<Dest>(CpuRegWrite { address: address, data: data }, time);
            }
            _ => { panic!("unexpected bus operation") }
        }
    }

    fn do_rspmem(&mut self, reason: vr4300::Reason, time: Time) {
        let mem = match reason.address() & 0x1000 == 0 {
            true => self.imem.as_mut(),
            false => self.dmem.as_mut(),
        };

        if let Some(mem) = mem {
            let offset = ((reason.address() >> 2) & 0x3ff) as usize;

            match reason {
                vr4300::Reason::BusRead32(_, _) => {
                    let data = mem[offset];
                    self.recv(ReadFinished::word(data), time, time);
                }
                vr4300::Reason::BusWrite32(_, _, data) => {
                    mem[offset] = data;
                    self.recv(WriteFinished::word(), time, time);
                }
                _ => { panic!("unexpected bus operation") }
            }
        } else {
            // The CPU doesn't currently have ownership of imem/dmem, need to request it from RspActor
            self.outstanding_mem_request = Some(reason);
            self.outbox.send::<RspActor>(rsp_actor::ReqestMemOwnership {}, time)
        }
    }
}

impl Handler<BusAccept> for CpuActor {
    fn recv(&mut self, _: BusAccept, time: Time, _limit: Time) -> SchedulerResult {
        let reason = self.outstanding_mem_request.clone().unwrap();
        let address = reason.address();

        match address & 0xfff0_0000 {
            0x0000_0000..=0x03ff_ffff => { // RDRAM
                todo!("RDRAM")
            }
            0x0400_0000 => match address & 0x040c_0000 { // RSP
                0x0400_0000 if address & 0x1000 == 0 => { // DMEM Direct access
                    self.do_rspmem(reason, time);
                    todo!("IMEM access {}", reason);
                }
                0x0400_0000 if address & 0x1000 != 0 => { // IMEM Direct access
                    println!("IMEM access {}", reason);
                    self.do_rspmem(reason, time);
                }
                0x0404_0000 | 0x0408_0000 => { // RSP Register
                    self.do_reg::<RspActor>(reason, time);
                }
                0x040c_0000 => { // Unmapped {
                    todo!("Unmapped")
                }
                _ => unreachable!()
            }
            0x0410_0000 => { // RDP Command Regs
                self.do_reg::<RdpActor>(reason, time);
            }
            0x0420_0000 => {
                todo!("RDP Span Regs")
            }
            0x0430_0000 => {
                todo!("MIPS Interface")
            }
            0x0440_0000 => { // Video Interface
                self.do_reg::<ViActor>(reason, time);
            }
            0x0450_0000 => { // Audio Interface
                self.do_reg::<AiActor>(reason, time);
            }
            0x0460_0000 => { // Peripheral Interface
                self.do_reg::<PiActor>(reason, time);
            }
            0x0470_0000 => {
                todo!("RDRAM Interface")
            }
            0x0480_0000 => { // Serial Interface
                self.do_reg::<SiActor>(reason, time);
            }
            0x0490_0000..=0x04ff_ffff => {
                todo!("Unmapped")
            }
            0x1fc0_0000 => { // SI External Bus
                self.do_reg::<SiActor>(reason, time);
            }
            0x0500_0000..=0x7fff_0000 => { // PI External bus
                match reason {
                    vr4300::Reason::BusRead32(_, _) => {
                        self.outbox.send::<PiActor>(pi_actor::PiRead::new(address), time);
                    }
                    vr4300::Reason::BusWrite32(_, _, data) => {
                        self.outbox.send::<PiActor>(pi_actor::PiWrite::new(address, data), time);
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

pub struct WriteFinished {
    length: CpuLength
}

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

impl Handler<rsp_actor::TransferMemOwnership> for CpuActor {
    fn recv(&mut self, message: rsp_actor::TransferMemOwnership, time: Time, _limit: Time) -> SchedulerResult {
        self.imem = Some(message.imem);
        self.dmem = Some(message.dmem);

        // We can now complete the memory request to imem or dmem
        let reason = self.outstanding_mem_request.clone().unwrap();
        self.do_rspmem(reason, time);

        SchedulerResult::Ok
    }
}

impl Handler<rsp_actor::ReqestMemOwnership> for CpuActor {
    fn recv(&mut self, _message: rsp_actor::ReqestMemOwnership, time: Time, _limit: Time) -> SchedulerResult {
        self.outbox.send::<RspActor>(rsp_actor::TransferMemOwnership {
            imem: self.imem.take().unwrap(),
            dmem: self.imem.take().unwrap(),
        }, time.add(4));

        SchedulerResult::Ok
    }
}
