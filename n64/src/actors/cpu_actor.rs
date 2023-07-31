
/// CpuActor: Emulates the CPU and MI (Mips Interface)

use actor_framework::{Actor, Time, Handler, Outbox, OutboxSend};
use super::{N64Actors, bus_actor::BusAccept, si_actor::SiActor, pi_actor::PiActor, vi_actor::ViActor, ai_actor::AiActor};

use crate::{vr4300, actors::{bus_actor::{BusActor, BusRequest}, rsp_actor::RspActor}};

pub struct CpuActor {
    outbox: CpuOutbox,
    committed_time: Time,
    cpu_overrun: u32,
    cpu_core: vr4300::Core,
    imem: Option<Box<[u32; 512]>>,
    dmem: Option<Box<[u32; 512]>>,
    outstanding_mem_request: Option<vr4300::Reason>,
}

actor_framework::make_outbox!(
    CpuOutbox<N64Actors, CpuActor> {
        bus: BusRequest,
        run: CpuRun,
        reg_write: CpuRegWrite,
        reg: CpuRegRead,
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
            cpu_overrun: 0,
            cpu_core: Default::default(),
            imem: None,
            dmem: None,
            outstanding_mem_request: None,
        }
    }
}

impl CpuActor {
    fn advance(&mut self, limit: Time) {
        if self.outstanding_mem_request.is_some() {
            // We are stalled waiting for a memory request to return
            return;
        }

        let limit_64: u64 = limit.into();
        //println!("CpuActor::advance({})", limit_64);
        let mut commit_time_64: u64 = self.committed_time.into();
        let cycles: u64 = limit_64 - commit_time_64;
        let mut odd = commit_time_64 & 1u64;

        let mut cpu_cycles = to_cpu_time(cycles, odd);
        loop {
            let result = self.cpu_core.run(to_cpu_time(cycles, odd));

            let used_cycles = to_bus_time(result.cycles, odd);
            commit_time_64 += used_cycles;
            self.committed_time = commit_time_64.into();
            println!("core did {} ({}) cycles and returned {} at cycle {}", used_cycles,  result.cycles, result.reason, commit_time_64);
            assert!(used_cycles <= cycles);

            return match result.reason {
                vr4300::Reason::Limited => {
                    self.outbox.send::<CpuActor>(CpuRun {}, self.committed_time);
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
                }
                reason => {
                    // Request over C-BUS/D-BUS
                    self.outstanding_mem_request = Some(reason);
                    self.outbox.send::<BusActor>(BusRequest::new::<Self>(1), self.committed_time);
                }
            };
        };
    }
}

impl Actor<N64Actors> for CpuActor {
    fn get_message(&mut self) -> &mut actor_framework::MessagePacketProxy<N64Actors> {
        self.outbox.as_mut()
    }

    fn message_delivered(&mut self, time: Time) {
        // hmmm....
    }
}

impl Handler<CpuRun> for CpuActor {
    fn recv(&mut self, _: CpuRun, time: Time, limit: Time) {
        self.committed_time = time;
        self.advance(limit);
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
            vr4300::Reason::BusRead32(address) => {
                self.outbox.send::<Dest>(CpuRegRead { address: address }, time);
            }
            vr4300::Reason::BusWrite32(address, data) => {
                self.outbox.send::<Dest>(CpuRegWrite { address: address, data: data }, time);
            }
            _ => { panic!("unexpected bus operation") }
        }
    }
}

impl Handler<BusAccept> for CpuActor {
    fn recv(&mut self, _: BusAccept, time: Time, _limit: Time) {
        let reason = self.outstanding_mem_request.take().unwrap();
        let address = reason.address();

        match address & 0xfff0_0000 {
            0x0000_0000..=0x03ff_ffff => { // RDRAM
                todo!("RDRAM")
            }
            0x0400_0000 => match address & 0x040c_0000 { // RSP
                0x0400_0000 if address & 0x1000 == 0 => { // DMEM Direct access
                    todo!("RSP DMEM")
                }
                0x0400_0000 if address & 0x1000 != 0 => { // IMEM Direct access
                    todo!("RSP IMEM")
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
                todo!("RDP Command Regs")
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
            0x0450_0000 => {
                self.do_reg::<AiActor>(reason, time);
            }
            0x0460_0000 => { // Peripheral Interface
                self.do_reg::<PiActor>(reason, time);
            }
            0x0470_0000 => {
                todo!("RDRAM Interface")
            }
            0x0480_0000 => {
                todo!("Serial Interface")
            }
            0x0490_0000..=0x04ff_ffff => {
                todo!("Unmapped")
            }
            0x1fc0_0000 => { // SI External Bus
                self.do_reg::<SiActor>(reason, time);
            }
            0x0500_0000..=0x7fff_0000 => { // PI External bus
                todo!("PI External Bus")
            }
            0x8000_0000..=0xffff_ffff => {
                todo!("Unmapped")
            }
            _ => unreachable!()
        }
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

impl Handler<ReadFinished> for CpuActor {
    fn recv(&mut self, message: ReadFinished, time: Time, _limit: Time) {
        // It takes length cycles to send the data across the SysAD bus
        self.outbox.send::<CpuActor>(CpuRun{}, time.add(message.length()));
        self.cpu_core.finish_read(message);
    }
}

pub struct WriteFinished {
    length: CpuLength
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

impl Handler<WriteFinished> for CpuActor {
    fn recv(&mut self, message: WriteFinished, time: Time, _limit: Time) {
        // It takes length cycles to send the data across the SysAD bus
        self.outbox.send::<CpuActor>(CpuRun{}, time.add(1));
        self.cpu_core.finish_write(message);
    }
}
