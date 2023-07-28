
/// CpuActor: Emulates the CPU and MI (Mips Interface)

use actor_framework::{Actor, MessagePacket, Time};
use super::N64Actors;

use crate::{vr4300};

#[derive(Default)]
pub struct CpuActor {
    committed_time: Time,
    cpu_overrun: u32,
    cpu_core: vr4300::Core,
    imem: Option<Box<[u32; 512]>>,
    dmem: Option<Box<[u32; 512]>>,
    outstanding_mem_request: Option<vr4300::Reason>,
}

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

impl CpuActor {
}

impl Actor<N64Actors> for CpuActor {
    fn advance(&mut self, limit: Time) -> MessagePacket<N64Actors> {
        if self.outstanding_mem_request.is_some() {
            // We are stalled waiting for a memory request to return
            return MessagePacket::no_message(limit);
        }

        let limit_64: u64 = limit.into();
        println!("CpuActor::advance({})", limit_64);
        let mut commit_time_64: u64 = self.committed_time.into();
        let cycles: u64 = limit_64 - commit_time_64;
        let mut odd = commit_time_64 & 1u64;

        let mut cpu_cycles = to_cpu_time(cycles, odd);
        loop {
            let result = self.cpu_core.run(to_cpu_time(cycles, odd));

            let used_cycles = to_bus_time(result.cycles, odd);
            commit_time_64 += used_cycles;
            self.committed_time = commit_time_64.into();
            println!("core did {} ({}) cycles and returned because {:?}", used_cycles,  result.cycles, result.reason);
            assert!(used_cycles <= cycles);

            return match result.reason {
                vr4300::Reason::Limited => {
                    MessagePacket::no_message(self.committed_time)
                }
                vr4300::Reason::SyncRequest => {
                    assert!(limit.is_resolved());
                    self.cpu_core.set_time(commit_time_64);

                    cpu_cycles -= result.cycles;
                    if cpu_cycles > 0 {
                        odd = commit_time_64 & 1u64;
                        continue;
                    }

                    MessagePacket::no_message(self.committed_time)
                }
                reason => {
                    // Request over C-BUS/D-BUS
                    self.outstanding_mem_request = Some(reason);

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
                            0x0404_0000 | 0x0408_0000 => { // RSP Registers
                                todo!("RSP Regs")
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
                        0x0440_0000 => {
                            todo!("Video Interface")
                        }
                        0x0450_0000 => {
                            todo!("Audio Interface")
                        }
                        0x0460_0000 => {
                            todo!("Peripheral Interface")
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
                            todo!("SI External Bus")
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
        };
    }

    fn advance_to(&mut self, target: Time) {
        let result = self.advance(target);
        assert!(result.is_none());
        assert!(result.time == target);
    }

    fn horizon(&mut self) -> Time {
        if self.outstanding_mem_request.is_some() {
            // An outstanding memory request means the CPU is blocked until we get a return message
            Time::max()
        } else {
            let commited: u64 = self.committed_time.into();
            return (commited + self.cpu_overrun as u64).into();
        }

    }
}

