
use actor_framework::*;
use super::{N64Actors, cpu_actor::{ReadFinished, CpuRegRead, CpuActor, CpuRegWrite}};

pub struct RspActor {
    outbox: RspOutbox,
    halted: bool,
    dma_busy: bool,
}

make_outbox!(
    RspOutbox<N64Actors, RspActor> {
        cpu: ReadFinished,
\    }
);

impl Default for RspActor {
    fn default() -> Self {
        Self {
            outbox: Default::default(),
            // HWTEST: IPL1 starts with a loop checking this bit, which implies that RSP might not
            //         enter the halted state immediately on reset.
            halted: true,
            dma_busy: false,
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
                // todo: remaining bits
                (self.dma_busy as u32) << 2 |
                (self.halted as u32) << 0
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
        self.outbox.send::<CpuActor>(ReadFinished::word(data), time.add(4));

    }
}
