
use actor_framework::*;
use super::{N64Actors, cpu_actor::{CpuReadFinished, CpuRegRead, CpuActor}};

pub struct RspActor {
    outbox: RspOutbox,
}

make_outbox!(
    RspOutbox<N64Actors, RspActor> {
        cpu: CpuReadFinished
    }
);

impl Default for RspActor {
    fn default() -> Self {
        Self {
            outbox: Default::default(),
        }
    }
}

impl Actor<N64Actors> for RspActor {
    fn get_message(&mut self) -> &mut MessagePacketProxy<N64Actors> {
        self.outbox.as_mut()
    }

    fn message_delivered(&mut self, _time: Time) {
        // do nothing
    }
}

impl Handler<CpuRegRead> for RspActor {
    fn recv(&mut self, message: CpuRegRead, time: Time, _limit: Time) {
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
                1 // halted
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
            _ => unreachable!()
        };
        self.outbox.send::<CpuActor>(CpuReadFinished::word(data), time.add(4));
    }
}
