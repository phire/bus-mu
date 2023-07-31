
use actor_framework::*;
use super::{N64Actors, cpu_actor::{ReadFinished, CpuRegRead, CpuActor, CpuRegWrite, WriteFinished}};

pub struct RspActor {
    outbox: RspOutbox,
    halted: bool,
    dma_busy: bool,
    imem: Option<Box<[u32; 512]>>,
    dmem: Option<Box<[u32; 512]>>,
}

make_outbox!(
    RspOutbox<N64Actors, RspActor> {
        cpu: ReadFinished,
        cpu_w: WriteFinished,
        send_mem: TransferMemOwnership,
    }
);

impl Default for RspActor {
    fn default() -> Self {
        Self {
            outbox: Default::default(),
            // HWTEST: IPL1 starts with a loop checking this bit, which implies that RSP might not
            //         enter the halted state immediately on a soft reset.
            halted: true,
            dma_busy: false,
            imem: Some(Box::new([0; 512])),
            dmem: Some(Box::new([0; 512])),
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
        self.outbox.send::<CpuActor>(ReadFinished::word(data), time.add(4));
    }
}

/// Converts 16bit binary ?a?b_?c?d_?e?f_?g?h to 8 bit binary abcd_efgh
fn deinterlave8(mut data: u32) -> u32 {
    data &= 0x5555;
    data = (data | data >> 1) & 0x3333;
    data = (data | data >> 2) & 0x0f0f;
    (data | data >> 4) & 0x00ff
}

impl Handler<CpuRegWrite> for RspActor {
    fn recv(&mut self, message: CpuRegWrite, time: Time, _limit: Time) {
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
        self.outbox.send::<CpuActor>(WriteFinished::word(), time.add(4))
    }
}


pub(super) struct ReqestMemOwnership {}

pub(super) struct TransferMemOwnership {
    pub imem: Box<[u32; 512]>,
    pub dmem: Box<[u32; 512]>,
}

impl Handler<TransferMemOwnership> for RspActor {
    fn recv(&mut self, message: TransferMemOwnership, _time: Time, _limit: Time) {
        self.imem = Some(message.imem);
        self.dmem = Some(message.dmem);

        // TODO: If the RSP is running, we need to continue it
    }
}

impl Handler<ReqestMemOwnership> for RspActor {
    fn recv(&mut self, _message: ReqestMemOwnership, time: Time, _limit: Time) {
        // TODO: calculate timings for when RSP is busy
        // TODO: Handle cases where the RSP DMA is active (which apparently corrupts CPU accesses)
        self.outbox.send::<CpuActor>(TransferMemOwnership {
            imem: self.imem.take().unwrap(),
            dmem: self.dmem.take().unwrap(),
        }, time);
    }
}
