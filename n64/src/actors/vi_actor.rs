use actor_framework::*;
use crate::{c_bus::{CBusWrite, CBusRead, ReadFinished, WriteFinished}, vi::{control::*, ViCore}};

use super::{N64Actors, cpu_actor::CpuActor};

pub struct ViActor {
    ctrl: ViCtrl,

    fb_origin: u32,
    fb_width: u16,
    v_intr: u16,
    output_format: OutputFormat,
    dirty: bool,
    vi_core: ViCore,
}

make_outbox!(
    ViOutbox<N64Actors, ViActor> {
        finish_read: ReadFinished,
        finish_write: WriteFinished,
    }
);

impl Default for ViActor {
    fn default() -> Self {

        let (vi_core, _resolver) = crate::vi::new();
        Self {
            ctrl: ViCtrl::new(),
            fb_origin: 0,
            fb_width: 0,
            v_intr: 0x3ff,
            output_format: Default::default(),
            dirty: false,
            vi_core,
        }
    }
}

impl Actor<N64Actors> for ViActor {
    type OutboxType = ViOutbox;
}

impl Handler<N64Actors, CBusWrite> for ViActor {
    fn recv(&mut self, outbox: &mut ViOutbox, message: CBusWrite, time: Time, _limit: Time) -> SchedulerResult {
        let data = message.data;
        match message.address & 0x3c {
            0x00 => { // VI_CTRL
                self.ctrl = ViCtrl::from_bytes(data.to_le_bytes());
                self.output_format.set_control(self.ctrl);
                self.dirty = true;
                println!("VI write VI_CTRL = {:#010x}", data);
                println!("         VI_CTRL = {:?}", self.ctrl);
            }
            0x04 => { // VI_ORIGIN
                println!("VI write VI_ORIGIN = {:#010x}", data);
                self.dirty |= self.fb_origin != data & 0x00ff_ffff;
                self.fb_origin = data & 0x00ff_ffff;
            }
            0x08 => { // VI_WIDTH
                println!("VI write VI_WIDTH = {:#010x}", data);
                self.dirty |= self.fb_width != data as u16 & 0xfff;
                self.fb_width = (data & 0xfff) as u16;
            }
            0x0c => { // VI_V_INTR
                println!("VI write VI_V_INTR = {:#010x}", data);
                self.dirty |= self.v_intr != data as u16 & 0xfff;
                self.v_intr = (data & 0x3ff) as u16;
            }
            0x10 => { // VI_V_CURRENT
                println!("VI write VI_V_CURRENT = {:#010x}", data);
                // TODO: Clear interrupt
            }
            0x14 => { // VI_BURST
                println!("VI write VI_BURST = {:#010x}", data);
                self.output_format.set_burst(ViBurst::from_bytes(data.to_le_bytes()));
                self.dirty = true;
            }
            0x18 => { // VI_V_SYNC
                println!("VI write VI_V_SYNC = {:#010x}", data);
                self.output_format.set_vsync(ViVSync::from_bytes(data.to_le_bytes()));
                self.dirty = true;
            }
            0x1c => { // VI_H_SYNC
                println!("VI write VI_H_SYNC = {:#010x}", data);
                self.output_format.set_hsync(ViHSync::from_bytes(data.to_le_bytes()));
                self.dirty = true;
            }
            0x20 => { // VI_H_SYNC_LEAP
                println!("VI write VI_H_SYNC_LEAP = {:#010x}", data);
                self.output_format.set_hsync_leap(ViHSyncLeap::from_bytes(data.to_le_bytes()));
                self.dirty = true;
            }
            0x24 => { // VI_H_VIDEO
                println!("VI write VI_H_VIDEO = {:#010x}", data);
                self.output_format.set_hvideo(ViHVideo::from_bytes(data.to_le_bytes()));
                self.dirty = true;
            }
            0x28 => { // VI_V_VIDEO
                println!("VI write VI_V_VIDEO = {:#010x}", data);
                self.output_format.set_vvideo(ViVVideo::from_bytes(data.to_le_bytes()));
                self.dirty = true;
            }
            0x2c => { // VI_V_BURST
                println!("VI write VI_V_BURST = {:#010x}", data);
                self.output_format.set_vburst(ViVBurst::from_bytes(data.to_le_bytes()));
                self.dirty = true;
            }
            0x30 => { // VI_X_SCALE
                println!("VI write VI_X_SCALE = {:#010x}", data);
                self.output_format.set_xscale(ViScale::from_bytes(data.to_le_bytes()));
                self.dirty = true;
            }
            0x34 => { // VI_Y_SCALE
                println!("VI write VI_Y_SCALE = {:#010x}", data);
                self.output_format.set_yscale(ViScale::from_bytes(data.to_le_bytes()));
                self.dirty = true;
            }
            0x38 => { // VI_TEST_ADDR
                todo!("VI write VI_TEST_ADDR = {:#010x}", data);
            }
            0x3c => { // VI_STAGED_DATA
                todo!("VI write VI_STAGED_DATA = {:#010x}", data);
            }
            _ => unreachable!()
        }
        outbox.send::<CpuActor>(WriteFinished {}, time.add(1));
        SchedulerResult::Ok
    }
}

impl Handler<N64Actors, CBusRead> for ViActor {
    fn recv(&mut self, outbox: &mut ViOutbox, message: CBusRead, time: Time, _limit: Time) -> SchedulerResult {
        let data = match message.address & 0x3c {
            0x00 => { // VI_CTRL
                let data = u32::from_le_bytes(self.ctrl.into_bytes());
                println!("VI read VI_CTRL = {:#010x}", data);
                data
            }
            0x04 => { // VI_ORIGIN
                println!("VI read VI_ORIGIN = {:#010x}", self.fb_origin);
                self.fb_origin
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
        outbox.send::<CpuActor>(ReadFinished {data}, time.add(1));
        SchedulerResult::Ok
    }
}
