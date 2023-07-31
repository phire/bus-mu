
use actor_framework::*;
use super::{N64Actors, cpu_actor::{ReadFinished, CpuRegRead, CpuActor, CpuRegWrite, WriteFinished}};

pub struct PiActor {
    outbox: PiOutbox,
}

make_outbox!(
    PiOutbox<N64Actors, PiActor> {
        cpu: ReadFinished,
        cpu_w: WriteFinished,
    }
);

impl Default for PiActor {
    fn default() -> Self {
        Self {
            outbox: Default::default(),
        }
    }
}

impl Actor<N64Actors> for PiActor {
    fn get_message(&mut self) -> &mut MessagePacketProxy<N64Actors> {
        self.outbox.as_mut()
    }

    fn message_delivered(&mut self, _time: Time) {
        // do nothing
    }
}

impl Handler<CpuRegWrite> for PiActor {
    fn recv(&mut self, message: CpuRegWrite, time: Time, _limit: Time) {
        let data = message.data;
        match message.address & 0x3c {
            0x00 => { // PI_DRAM_ADDR
                todo!("PI_DRAM_ADDR")
            }
            0x04 => { // PI_CART_ADDR
                todo!("PI_CART_ADDR")
            }
            0x08 => { // PI_RD_LEN
                todo!("PI_RD_LEN")
            }
            0x0c => { // PI_WR_LEN
                todo!("PI_WR_LEN")
            }
            0x10 => { // PI_STATUS
                println!("PI write PI_STATUS = {:#010x}", data);
                if data & 0x1 != 0 {
                    println!("  reset dma")
                }
                if data & 0x2 != 0 {
                    println!("  clear interrupt")
                }
            }
            0x14 | 0x24 => { // PI_BSD_DOMn_LAT
                todo!("PI_BSD_DOMn_LAT")
            }
            0x18 | 0x28 => { // PI_BSD_DOM1_PWD
                todo!("PI_BSD_DOMn_PWD")
            }
            0x1c | 0x2c => { // PI_BSD_DOM1_PGS
                todo!("PI_BSD_DOMn_PGS")
            }
            0x20 | 0x30 => { // PI_BSD_DOM1_RLS
                todo!("PI_BSD_DOMn_RLS")
            }
            0x34 | 0x38 | 0x3c => {
                unimplemented!()
            }
            _ => unreachable!(),
        }
        self.outbox.send::<CpuActor>(WriteFinished::word(), time.add(4));
    }
}

impl Handler<CpuRegRead> for PiActor {
    fn recv(&mut self, message: CpuRegRead, time: Time, _limit: Time) {
        let data = match message.address & 0x3c {
            0x00 => { // PI_DRAM_ADDR
                todo!("PI_DRAM_ADDR")
            }
            0x04 => { // PI_CART_ADDR
                todo!("PI_CART_ADDR")
            }
            0x08 => { // PI_RD_LEN
                todo!("PI_RD_LEN")
            }
            0x0c => { // PI_WR_LEN
                todo!("PI_WR_LEN")
            }
            0x10 => { // PI_STATUS
                todo!("PI_STATUS");
                // let data = 0;
                // println!("PI read PI_STATUS = {:#010x}", data);
                // data
            }
            0x14 | 0x24 => { // PI_BSD_DOMn_LAT
                todo!("PI_BSD_DOMn_LAT")
            }
            0x18 | 0x28 => { // PI_BSD_DOM1_PWD
                todo!("PI_BSD_DOMn_PWD")
            }
            0x1c | 0x2c => { // PI_BSD_DOM1_PGS
                todo!("PI_BSD_DOMn_PGS")
            }
            0x20 | 0x30 => { // PI_BSD_DOM1_RLS
                todo!("PI_BSD_DOMn_RLS")
            }
            0x34 | 0x38 | 0x3c => {
                unimplemented!()
            }
            _ => unreachable!(),
        };
        self.outbox.send::<CpuActor>(ReadFinished::word(data), time.add(4));
    }
}
