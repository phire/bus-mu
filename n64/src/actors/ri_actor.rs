use actor_framework::*;
use crate::c_bus::{CBusWrite, CBusRead};

use super::{N64Actors, cpu_actor::{ReadFinished, CpuActor, WriteFinished}};

/// RI or Ram Interface is the memory controller for the RCP.
///
/// It handles the low level details of the Rambus interface and acts as a bridge
/// between the internal D-BUS and the external RDRAM.
///
/// It's also responsible for issuing refresh commands to the RDRAM.
pub struct RiActor {

}

make_outbox!(
    RiOutbox<N64Actors, RiActor> {
        cpu: ReadFinished,
        cpu_w: WriteFinished,
    }
);

impl Default for RiActor {
    fn default() -> Self {
        Self {

        }
    }
}

impl Actor<N64Actors> for RiActor {
    type OutboxType = RiOutbox;
}


impl Handler<N64Actors, CBusWrite> for RiActor {
    fn recv(&mut self, outbox: &mut RiOutbox, message: CBusWrite, time: Time, _limit: Time) -> SchedulerResult {
        let data = message.data;
        match message.address & 0x1c {
            0x00 => {
                // RI_MODE
                println!("RI: Write RI_MODE = {:#010x}", data);
            }
            0x04 => {
                // RI_CONFIG
                println!("RI: Write RI_CONFIG = {:#010x}", data);
            }
            0x08 => {
                // RI_CURRENT_LOAD
                println!("RI: Write RI_CURRENT_LOAD = {:#010x}", data);
            }
            0x0c => {
                // RI_SELECT
                println!("RI: Write RI_SELECT = {:#010x}", data);
            }
            0x10 => {
                // RI_REFRESH
                todo!("Write RI_REFRESH = {:#010x}", data);
            }
            0x14 => {
                // RI_LATENCY
                todo!("Write RI_LATENCY = {:#010x}", data);
            }
            0x18 => {
                // RI_ERROR ?
                todo!("Write RI_ERROR = {:#010x}", data);
            }
            0x1c => {
                // RI_BANK_STATUS ?
                todo!("Write RI_BANK_STATUS = {:#010x}", data);
            }
            _ => unreachable!()
        }
        outbox.send::<CpuActor>(WriteFinished::word(), time.add(4))
    }
}


impl Handler<N64Actors, CBusRead> for RiActor {
    fn recv(&mut self, outbox: &mut RiOutbox, message: CBusRead, time: Time, _limit: Time) -> SchedulerResult {
        let data = match message.address & 0x1c {
            0x00 => {
                // RI_MODE
                todo!("Read RI_MODE");
            }
            0x04 => {
                // RI_CONFIG
                todo!("Read RI_CONFIG");
            }
            0x08 => {
                // RI_CURRENT_LOAD
                todo!("Read RI_CURRENT_LOAD");
            }
            0x0c => {
                // RI_SELECT
                // Temporary Hack: a non-zero value, makes IPL3 think RAM has already been initialized
                println!("Read RI_SELECT = {:#010x}", 0x0000_0014);
                0x0000_0014
            }
            0x10 => {
                // RI_REFRESH
                todo!("Read RI_REFRESH");
            }
            0x14 => {
                // RI_LATENCY
                todo!("Read RI_LATENCY");
            }
            0x18 => {
                // RI_ERROR ?
                todo!("Read RI_ERROR");
            }
            0x1c => {
                // RI_BANK_STATUS ?
                todo!("Read RI_BANK_STATUS");
            }
            _ => unreachable!()
        };
        outbox.send::<CpuActor>(ReadFinished::word(data), time.add(4));
        SchedulerResult::Ok
    }
}
