use actor_framework::*;
use crate::c_bus::{CBusRead, CBusWrite, self, WriteFinished, ReadFinished};

use super::{N64Actors, cpu_actor::CpuActor};

pub struct RspActor {
    halted: bool,
    dma_busy: bool,
    dmem_imem: Option<Box<[u32; 2048]>>,
}

make_outbox!(
    RspOutbox<N64Actors, RspActor> {
        finish_read: ReadFinished,
        finish_write: WriteFinished,
        send_mem: c_bus::Resource,
        request_mem: c_bus::ResourceReturnRequest,
    }
);

impl Default for RspActor {
    fn default() -> Self {
        Self {
            // HWTEST: IPL1 starts with a loop checking this bit, which implies that RSP might not
            //         enter the halted state immediately on a soft reset.
            halted: true,
            dma_busy: false,
            dmem_imem: Some(Box::new([0; 2048])),
        }
    }
}

impl Actor<N64Actors> for RspActor {
    type OutboxType = RspOutbox;
}

impl Handler<N64Actors, CBusRead> for RspActor {
    fn recv(&mut self, outbox: &mut RspOutbox, message: CBusRead, time: Time, _limit: Time) -> SchedulerResult {
        let address = message.address;
        let data = match address {
            0x0404_0000 => { // SP_DMA_SPADDR
                todo!("SP_DMA_SPADDR")
            }
            0x0404_0004 => { // SP_DMA_RAMADDR
                todo!("SP_DMA_RAMADDR")
            }
            0x0404_0008 => { // SP_DMA_RDLEN
                todo!("SP_DMA_RDLEN")
            }
            0x0404_000c => { // SP_DMA_WRLEN
                todo!("SP_DMA_WRLEN")
            }
            0x0404_0010 => { // SP_STATUS
                let data = (self.dma_busy as u32) << 2 |
                    (self.halted as u32) << 0;
                println!("RSP read SP_STATUS = {:#010x}", data);
                // todo: remaining bits
                data
            }
            0x0404_0014 => { // SP_DMA_FULL
                todo!("SP_DMA_FULL")
            }
            0x0404_0018 => { // SP_DMA_BUSY
                self.dma_busy as u32
            }
            0x0404_001c => { // SP_SEMAPHORE
                todo!("SP_SEMAPHORE")
            }
            _ => unimplemented!()
        };
        outbox.send::<CpuActor>(ReadFinished {data}, time.add(4));
        SchedulerResult::Ok
    }
}

/// Converts 16bit binary ?a?b_?c?d_?e?f_?g?h to 8 bit binary abcd_efgh
fn deinterlave8(mut data: u32) -> u32 {
    data &= 0x5555;
    data = (data | data >> 1) & 0x3333;
    data = (data | data >> 2) & 0x0f0f;
    (data | data >> 4) & 0x00ff
}

impl Handler<N64Actors, CBusWrite> for RspActor {
    fn recv(&mut self, outbox: &mut RspOutbox, message: CBusWrite, time: Time, _limit: Time) -> SchedulerResult {
        let data = message.data;
        match message.address {
            0x0404_0000 => { // SP_DMA_SPADDR
                todo!("SP_DMA_SPADDR")
            }
            0x0404_0004 => { // SP_DMA_RAMADDR
                todo!("SP_DMA_RAMADDR")
            }
            0x0404_0008 => { // SP_DMA_RDLEN
                todo!("SP_DMA_RDLEN")
            }
            0x0404_000c => { // SP_DMA_WRLEN
                todo!("SP_DMA_WRLEN")
            }
            0x0404_0010 => { // SP_STATUS
                println!("RSP write SP_STATUS = {:#010x}", data);
                // todo: remaining bits
                if data & 0x0000_0001 != 0 {
                    self.halted = false;
                    println!("  Clear Halt");
                }
                if data & 0x0000_0002 != 0 {
                    self.halted = true;
                    println!("  Set Halt");
                }
                if data & 0x0000_0004 != 0 {
                    println!("  Clear Broke");
                }
                if data & 0x0000_0008 != 0 {
                    println!("  Clear Interrupt");
                }
                if data & 0x0000_0010 != 0 {
                    println!("  Set Interrupt");
                }
                if data & 0x0000_0020 != 0 {
                    println!("  Clear Single Step");
                }
                if data & 0x0000_0040 != 0 {
                    println!("  Set Single Step");
                }
                if data & 0x0000_0080 != 0 {
                    println!("  Clear Intr On Break");
                }
                if data & 0x0000_0100 != 0 {
                    println!("  Set Intr On Break");
                }
                if data & 0x00aa_aa00 != 0 {
                    println!("  Clear Signal {:#02x}", deinterlave8(data >> 9));
                }
                if data & 0x0155_5400 != 0 {
                    println!("  Set Signal {:#02x}", deinterlave8(data >> 10));
                }
            }
            0x0404_0014 => { // SP_DMA_FULL
                todo!("SP_DMA_FULL")
            }
            0x0404_0018 => { // SP_DMA_BUSY
                todo!("SP_DMA_BUSY")
            }
            0x0404_001c => { // SP_SEMAPHORE
                todo!("SP_SEMAPHORE")
            }
            _ => unimplemented!()
        };
        outbox.send::<CpuActor>(WriteFinished {}, time.add(4));
        SchedulerResult::Ok
    }
}

impl Handler<N64Actors, c_bus::Resource> for RspActor {
    fn recv(&mut self, _outbox: &mut RspOutbox, message: c_bus::Resource, _time: Time, _limit: Time) -> SchedulerResult {

        match message {
            c_bus::Resource::RspMem(mem) => {
                self.dmem_imem = Some(mem);
            }
            //_ => panic!("RSP received unexpected resource"),
        }

        // TODO: If the RSP is running, we need to continue it
        SchedulerResult::Ok
    }
}

impl Handler<N64Actors, c_bus::ResourceRequest> for RspActor {
    fn recv(&mut self, outbox: &mut RspOutbox, message: c_bus::ResourceRequest, time: Time, _limit: Time) -> SchedulerResult {
        // TODO: calculate timings for when RSP is busy
        // TODO: Handle cases where the RSP DMA is active (which apparently corrupts CPU accesses)
        match message {
            c_bus::ResourceRequest::RspMem => {
                outbox.send::<CpuActor>(c_bus::Resource::RspMem(self.dmem_imem.take().unwrap()), time);
            }
            //_ => panic!("RSP received unexpected resource request"),
        }
        SchedulerResult::Ok
    }
}
