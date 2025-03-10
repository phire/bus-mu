

use actor_framework::*;
use crate::c_bus::{CBusWrite, CBusRead, ReadFinished, WriteFinished};

use super::{N64Actors, cpu_actor::CpuActor};

pub struct AiActor {
    dram_addr: u32,
    length: u32,
    dma_enable: bool,
}

make_outbox!(
    AiOutbox<N64Actors, AiActor> {
        cpu: ReadFinished,
        cpu_w: WriteFinished,
    }
);

impl Default for AiActor {
    fn default() -> Self {
        Self {
            dram_addr: 0,
            length: 0,
            dma_enable: false,
        }
    }
}

impl Actor<N64Actors> for AiActor {
    type OutboxType = AiOutbox;
}

impl Handler<N64Actors, CBusWrite> for AiActor {
    fn recv(&mut self, outbox: &mut AiOutbox, message: CBusWrite, time: Time, _limit: Time) -> SchedulerResult {
        let data = message.data;
        match message.address & 0x1c {
            0x00 => { // AI_DRAM_ADDR
                println!("AI_DRAM_ADDR = {:#010x}", data);
                self.dram_addr = data & 0x00ff_fff8;
            }
            0x04 => { // AI_LENGTH
                println!("AI_LENGTH = {:#010x}", data);
                self.length = data & 0x0003_fff8;
            }
            0x08 => { // AI_CONTROL
                println!("AI_CONTROL = {:#010x}", data);
                self.dma_enable = data & 0x1 != 0;
            }
            0x0c => { // AI_STATUS
                todo!("AI_STATUS = {:#010x}", data);
            }
            0x10 => { // AI_DACRATE
                todo!("AI_DACRATE = {:#010x}", data);
            }
            0x14 => { // AI_BITRATE
                todo!("AI_BITRATE = {:#010x}", data);
            }
            0x1c => { // unknown
                todo!("AI unknown = {:#010x}", data);
            }
            _ => unreachable!()
        }
        outbox.send::<CpuActor>(WriteFinished {}, time.add(4))
    }
}

impl Handler<N64Actors, CBusRead> for AiActor {
    fn recv(&mut self, outbox: &mut AiOutbox, message: CBusRead, time: Time, _limit: Time) -> SchedulerResult {
        let data = match message.address & 0x1c {
            0x00 => { // AI_DRAM_ADDR
                println!("read AI_DRAM_ADDR");
                self.dram_addr
            }
            0x04 => { // AI_LENGTH
                println!("read AI_LENGTH");
                self.length
            }
            0x08 => { // AI_CONTROL
                println!("read AI_CONTROL");
                // write only, returns length
                self.length
            }
            0x0c => { // AI_STATUS
                todo!("AI_STATUS");
            }
            0x10 => { // AI_DACRATE
                println!("read AI_DACRATE");
                // write only, returns length
                self.length
            }
            0x14 => { // AI_BITRATE
                println!("read AI_BITRATE");
                // write only, returns length
                self.length
            }
            0x1c => { // unknown
                todo!("AI unknown");
            }
            _ => unreachable!()
        };
        outbox.send::<CpuActor>(ReadFinished { data }, time.add(4))
    }
}
