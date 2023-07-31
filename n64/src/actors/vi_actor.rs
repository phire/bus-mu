
use actor_framework::*;
use super::{N64Actors, cpu_actor::{ReadFinished, CpuRegRead, CpuActor, CpuRegWrite, WriteFinished}};

pub struct ViActor {
    outbox: ViOutbox,
}

make_outbox!(
    ViOutbox<N64Actors, ViActor> {
        cpu: ReadFinished,
        cpu_w: WriteFinished,
    }
);

impl Default for ViActor {
    fn default() -> Self {
        Self {
            outbox: Default::default(),
        }
    }
}

impl Actor<N64Actors> for ViActor {
    fn get_message(&mut self) -> &mut MessagePacketProxy<N64Actors> {
        self.outbox.as_mut()
    }

    fn message_delivered(&mut self, _time: Time) {
        // do nothing
    }
}

impl Handler<CpuRegWrite> for ViActor {
    fn recv(&mut self, message: CpuRegWrite, time: Time, _limit: Time) {
        let data = message.data;
        match message.address & 0x3c {
            0x00 => { // VI_CTRL
                todo!("VI write VI_CTRL = {:#010x}", data);
            }
            0x04 => { // VI_ORIGIN
                todo!("VI write VI_ORIGIN = {:#010x}", data);
            }
            0x08 => { // VI_WIDTH
                todo!("VI write VI_WIDTH = {:#010x}", data);
            }
            0x0c => { // VI_V_INTR
                println!("VI write VI_V_INTR = {:#010x}", data);
            }
            0x10 => { // VI_V_CURRENT
                println!("VI write VI_V_CURRENT = {:#010x}", data);
            }
            0x14 => { // VI_BURST
                todo!("VI write VI_BURST = {:#010x}", data);
            }
            0x18 => { // VI_V_SYNC
                todo!("VI write VI_V_SYNC = {:#010x}", data);
            }
            0x1c => { // VI_H_SYNC
                todo!("VI write VI_H_SYNC = {:#010x}", data);
            }
            0x20 => { // VI_H_SYNC_LEAP
                todo!("VI write VI_H_SYNC_LEAP = {:#010x}", data);
            }
            0x24 => { // VI_H_VIDEO
                println!("VI write VI_H_VIDEO = {:#010x}", data);
            }
            0x28 => { // VI_V_VIDEO
                todo!("VI write VI_V_VIDEO = {:#010x}", data);
            }
            0x2c => { // VI_V_BURST
                todo!("VI write VI_V_BURST = {:#010x}", data);
            }
            0x30 => { // VI_X_SCALE
                todo!("VI write VI_X_SCALE = {:#010x}", data);
            }
            0x34 => { // VI_Y_SCALE
                todo!("VI write VI_Y_SCALE = {:#010x}", data);
            }
            0x38 => { // VI_TEST_ADDR
                todo!("VI write VI_TEST_ADDR = {:#010x}", data);
            }
            0x3c => { // VI_STAGED_DATA
                todo!("VI write VI_STAGED_DATA = {:#010x}", data);
            }
            _ => unreachable!()
        }
        self.outbox.send::<CpuActor>(WriteFinished::word(), time.add(4));
    }
}

impl Handler<CpuRegRead> for ViActor {
    fn recv(&mut self, message: CpuRegRead, time: Time, _limit: Time) {
        let data = match message.address & 0x3c {
            0x00 => { // VI_CTRL
                todo!("VI read VI_CTRL");
            }
            0x04 => { // VI_ORIGIN
                todo!("VI read VI_ORIGIN");
            }
            0x08 => { // VI_WIDTH
                todo!("VI read VI_WIDTH");
            }
            0x0c => { // VI_V_INTR
                todo!("VI read VI_V_INTR");
            }
            0x10 => { // VI_V_CURRENT
                todo!("VI read VI_V_CURRENT");
            }
            0x14 => { // VI_BURST
                todo!("VI read VI_BURST");
            }
            0x18 => { // VI_V_SYNC
                todo!("VI read VI_V_SYNC");
            }
            0x1c => { // VI_H_SYNC
                todo!("VI read VI_H_SYNC");
            }
            0x20 => { // VI_H_SYNC_LEAP
                todo!("VI read VI_H_SYNC_LEAP");
            }
            0x24 => { // VI_H_VIDEO
                todo!("VI read VI_H_VIDEO");
            }
            0x28 => { // VI_V_VIDEO
                todo!("VI read VI_V_VIDEO");
            }
            0x2c => { // VI_V_BURST
                todo!("VI read VI_V_BURST");
            }
            0x30 => { // VI_X_SCALE
                todo!("VI read VI_X_SCALE");
            }
            0x34 => { // VI_Y_SCALE
                todo!("VI read VI_Y_SCALE");
            }
            0x38 => { // VI_TEST_ADDR
                todo!("VI read VI_TEST_ADDR");
            }
            0x3c => { // VI_STAGED_DATA
                todo!("VI read VI_STAGED_DATA");
            }
            _ => unreachable!()
        };
        self.outbox.send::<CpuActor>(ReadFinished::word(data), time.add(4));
    }
}