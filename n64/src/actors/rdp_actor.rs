
use actor_framework::*;

use crate::{N64Actors, c_bus::{CBusWrite, CBusRead, ReadFinished, WriteFinished}};

use super::cpu_actor::CpuActor;

pub struct RdpActor {
    start: u32,
    end: u32,
}

make_outbox!(
    RdpOutbox<N64Actors, RdpActor> {
        finish_read: ReadFinished,
        finish_write: WriteFinished,
    }
);

impl Default for RdpActor {
    fn default() -> Self {
        Self {
            start: 0,
            end: 0,
        }
    }
}

impl Actor<N64Actors> for RdpActor {
    type OutboxType = RdpOutbox;
}

impl Handler<N64Actors, CBusWrite> for RdpActor {
    fn recv(&mut self, outbox: &mut RdpOutbox, message: CBusWrite, time: Time, _limit: Time) -> SchedulerResult {
        let data = message.data;
        match message.address & 0x1c {
            0x00 => { // DP_START
                println!("RDP write DP_START = {:#08x}", data);
                self.start = data & 0x00ff_ffff;
            }
            0x04 => { // DP_END
                println!("RDP write DP_END = {:#08x}", data);
                self.end = data & 0x00ff_ffff;
            }
            0x08 => { // DP_CURRENT
                // read-only
                panic!("RDP write DP_CURRENT = {:#08x}", data);
            }
            0x0c => { // DP_STATUS
                todo!("RDP write DP_STATUS = {:#08x}", data);
            }
            0x10 => { // DPC_CLOCK
                todo!("RDP write DPC_CLOCK = {:#08x}", data);
            }
            0x14 => { // DPC_BUSY
                todo!("RDP write DPC_BUSY = {:#08x}", data);
            }
            0x18 => { // DPC_PIPE_BUSY
                todo!("RDP write DPC_PIPE_BUSY = {:#08x}", data);
            }
            0x1c => { // DPC_TMEM_BUSY
                todo!("RDP write DPC_TMEM_BUSY = {:#08x}", data);
            }
            _ => unreachable!()
        }
        outbox.send::<CpuActor>(WriteFinished {}, time.add(4));
        SchedulerResult::Ok
    }
}

impl Handler<N64Actors, CBusRead> for RdpActor {
    fn recv(&mut self, outbox: &mut RdpOutbox, message: CBusRead, time: Time, _limit: Time) -> SchedulerResult {
        let data = match message.address & 0x1c {
            0x00 => { // DP_START
                println!("RDP read DP_START = {:#010x}", self.start);
                self.start
            }
            0x04 => { // DP_END
                println!("RDP read DP_END = {:#010x}", self.end);
                self.end
            }
            0x08 => { // DP_CURRENT
                todo!("RDP read DP_CURRENT");
            }
            0x0c => { // DP_STATUS
                let status = 0;
                println!("RDP read DP_STATUS = {:#010x}", status);
                status
            }
            0x10 => { // DPC_CLOCK
                todo!("RDP read DPC_CLOCK");
            }
            0x14 => { // DPC_BUSY
                todo!("RDP read DPC_BUSY");
            }
            0x18 => { // DPC_PIPE_BUSY
                todo!("RDP read DPC_PIPE_BUSY");
            }
            0x1c => { // DPC_TMEM_BUSY
                todo!("RDP read DPC_TMEM_BUSY");
            }
            _ => unreachable!()
        };
        outbox.send::<CpuActor>(ReadFinished { data }, time.add(4));
        SchedulerResult::Ok
    }
}
