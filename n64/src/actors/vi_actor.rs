use actor_framework::*;
use crate::c_bus::{CBusWrite, CBusRead, ReadFinished, WriteFinished};

use super::{N64Actors, cpu_actor::CpuActor};
use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

pub struct ViActor {
    ctrl: ViCtrl,

    origin: u32,
    width: u16,
    v_intr: u16,
    hsync_width: u8,
    burst_width: u8,
    vsync_width: u8,
    burst_start: u16,
    vsync: u16,
    hsync: u16,
    leap: u8,
    leap_a: u16,
    leap_b: u16,
    h_start: u16,
    h_end: u16,
    v_start: u16,
    v_end: u16,
    v_burst_start: u16,
    v_burst_end: u16,
    x_scale: u16,
    x_offset: u16,
    y_scale: u16,
    y_offset: u16,
}

make_outbox!(
    ViOutbox<N64Actors, ViActor> {
        finish_read: ReadFinished,
        finish_write: WriteFinished,
    }
);

impl Default for ViActor {
    fn default() -> Self {
        Self {
            ctrl: ViCtrl::new(),
            origin: 0,
            width: 0,
            v_intr: 0x3ff,
            hsync_width: 0,
            burst_width: 0,
            vsync_width: 0,
            burst_start: 0,
            vsync: 0,
            hsync: 0,
            leap: 0,
            leap_a: 0,
            leap_b: 0,
            h_start: 0,
            h_end: 0,
            v_start: 0,
            v_end: 0,
            v_burst_start: 0,
            v_burst_end: 0,
            x_scale: 0,
            x_offset: 0,
            y_scale: 0,
            y_offset: 0,
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
                self.ctrl = ViCtrl::from(data);
                println!("VI write VI_CTRL = {:#010x}", data);
                println!("         VI_CTRL = {:?}", self.ctrl);
            }
            0x04 => { // VI_ORIGIN
                println!("VI write VI_ORIGIN = {:#010x}", data);
                self.origin = data & 0x00ff_ffff;
            }
            0x08 => { // VI_WIDTH
                println!("VI write VI_WIDTH = {:#010x}", data);
                self.width = (data & 0xfff) as u16;
            }
            0x0c => { // VI_V_INTR
                println!("VI write VI_V_INTR = {:#010x}", data);
                self.v_intr = (data & 0x3ff) as u16;
            }
            0x10 => { // VI_V_CURRENT
                println!("VI write VI_V_CURRENT = {:#010x}", data);
            }
            0x14 => { // VI_BURST
                println!("VI write VI_BURST = {:#010x}", data);
                let burst = ViBurst::from_bytes(data.to_le_bytes());
                self.hsync_width = burst.hsync_width();
                self.burst_width = burst.burst_width();
                self.vsync_width = burst.vsync_width();
                self.burst_start = burst.burst_start();
            }
            0x18 => { // VI_V_SYNC
                println!("VI write VI_V_SYNC = {:#010x}", data);
                self.vsync = ViVSync::from_bytes(data.to_le_bytes()).vsync();

            }
            0x1c => { // VI_H_SYNC
                println!("VI write VI_H_SYNC = {:#010x}", data);
                let reg = ViHSync::from_bytes(data.to_le_bytes());
                self.hsync = reg.hsync();
                self.leap = reg.leap();
            }
            0x20 => { // VI_H_SYNC_LEAP
                println!("VI write VI_H_SYNC_LEAP = {:#010x}", data);
                let reg = ViHSyncLeap::from_bytes(data.to_le_bytes());
                self.leap_a = reg.leap_a();
                self.leap_b = reg.leap_b();
            }
            0x24 => { // VI_H_VIDEO
                println!("VI write VI_H_VIDEO = {:#010x}", data);
                let reg = ViHVideo::from_bytes(data.to_le_bytes());
                self.h_start = reg.h_start();
                self.h_end = reg.h_end();
            }
            0x28 => { // VI_V_VIDEO
                println!("VI write VI_V_VIDEO = {:#010x}", data);
                let reg = ViVVideo::from_bytes(data.to_le_bytes());
                self.v_end = reg.v_end();
                self.v_start = reg.v_start();
            }
            0x2c => { // VI_V_BURST
                println!("VI write VI_V_BURST = {:#010x}", data);
                let reg = ViVBurst::from_bytes(data.to_le_bytes());
                self.v_burst_start = reg.v_burst_start();
                self.v_burst_end = reg.v_burst_end();
            }
            0x30 => { // VI_X_SCALE
                println!("VI write VI_X_SCALE = {:#010x}", data);
                let reg = ViScale::from_bytes(data.to_le_bytes());
                self.x_offset = reg.offset();
                self.x_scale = reg.scale();
            }
            0x34 => { // VI_Y_SCALE
                println!("VI write VI_Y_SCALE = {:#010x}", data);
                let reg = ViScale::from_bytes(data.to_le_bytes());
                self.y_offset = reg.offset();
                self.y_scale = reg.scale();
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
                println!("VI read VI_CTRL = {:#010x}", u32::from(self.ctrl));
                u32::from(self.ctrl)
            }
            0x04 => { // VI_ORIGIN
                println!("VI read VI_ORIGIN = {:#010x}", self.origin);
                self.origin
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

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct ViCtrl {
    pub pixel_type: PixelType,
    pub gamma_dither: bool,
    pub gamma: bool,
    pub divot: bool,
    pub vbus_clock: bool,
    pub serrate: bool,
    pub test_mode: bool,
    pub aa_mode: AaMode,
    #[skip] __: B1,
    pub kill_we: bool,
    pub pixel_advance: B4,
    pub dedither_filter: bool,
    #[skip] __: B15,
}

impl From<u32> for ViCtrl {
    fn from(data: u32) -> Self {
        Self::from_bytes(data.to_le_bytes())
    }
}

impl From<ViCtrl> for u32 {
    fn from(data: ViCtrl) -> Self {
        u32::from_le_bytes(data.into_bytes())
    }
}

#[derive(BitfieldSpecifier, Debug)]
#[bits = 2]
pub enum PixelType {
    Blank,
    Reserved,
    RGB16,
    RGBA32,
}

#[derive(BitfieldSpecifier, Debug)]
#[bits = 2]
pub enum AaMode {
    Enabled,
    EnabledReducedBandwdith,
    DisabledResambled,
    DisabledExact,
}

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct ViBurst {
    pub hsync_width: B8,
    pub burst_width: B8,
    pub vsync_width: B4,
    pub burst_start: B10,
    #[skip] __: B2,
}

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct ViVSync {
    pub vsync: B10,
    #[skip] __: B22,
}

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct ViHSync {
    pub hsync: B10,
    #[skip] __: B4,
    pub leap: B5,
    #[skip] __: B13,
}


#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct ViHSyncLeap {
    pub leap_a: B12,
    #[skip] __: B4,
    pub leap_b: B12,
    #[skip] __: B4,
}

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct ViHVideo {
    pub h_start: B10,
    #[skip] __: B6,
    pub h_end: B10,
    #[skip] __: B6,
}

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct ViVVideo {
    pub v_start: B10,
    #[skip] __: B6,
    pub v_end: B10,
    #[skip] __: B6,
}

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct ViVBurst {
    pub v_burst_start: B10,
    #[skip] __: B6,
    pub v_burst_end: B10,
    #[skip] __: B6,
}

#[bitfield(bits = 32)]
#[derive(Debug, Copy, Clone)]
pub struct ViScale {
    pub offset: B12,
    #[skip] __: B4,
    pub scale: B12,
    #[skip] __: B4,
}
