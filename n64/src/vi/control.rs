
use modular_bitfield::{bitfield, specifiers::*, Specifier};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct OutputFormat {
    pub dedither_filter: bool,
    pub aa_mode: AaMode,
    pub divot: bool,
    pub gamma: bool,
    pub gamma_dither: bool,
    pub pixel_type: PixelType,
    pub hsync_width: u8,
    pub burst_width: u8,
    pub vsync_width: u8,
    pub burst_start: u16,
    pub vsync: u16,
    pub hsync: u16,
    pub leap: u8,
    pub leap_a: u16,
    pub leap_b: u16,
    pub h_start: u16,
    pub h_end: u16,
    pub v_start: u16,
    pub v_end: u16,
    pub v_burst_start: u16,
    pub v_burst_end: u16,
    pub x_scale: Fixed2_10,
    pub x_offset: Fixed2_10,
    pub y_scale: Fixed2_10,
    pub y_offset: Fixed2_10,
}

impl OutputFormat {
    pub fn transfers_per_line(&self) -> u32 {
        let dots = self.h_end as f32 - self.h_start as f32;
        let pixels = (dots * f32::from(self.x_scale).ceil()) as u32 + 1;
        let transfers_per_line = match self.pixel_type {
            PixelType::Blank => 0,
            PixelType::Reserved => 0,
            PixelType::Rgb5c3 => pixels.next_multiple_of(4) / 4,
            PixelType::Rgb8a5c3 => pixels.next_multiple_of(2) / 2,
        };

        // HWTEST: How exactly do dedither and AA interact?
        //         Does dedither force AAReducedBandwidth to always fetch 3 lines?
        //         Or does ResampleOnly always fetch 3 lines, even when there is no need.
        let aa_passes = match self.aa_mode {
            AaMode::Disabled => 1,
            AaMode::ResampleOnly if !self.dedither_filter => 1,
            _ => 3,
        };
        let scaler_lines = if self.y_scale.fractional() == 0 { 1 } else { 2 };

        transfers_per_line * aa_passes * scaler_lines
    }

    pub fn dma_bytes_per_line(&self) -> u32 {
        let bytes_per_transfer = match self.pixel_type {
            PixelType::Rgb5c3 => 9,
            PixelType::Rgb8a5c3 => 8,
            _ => 0
        };
        self.transfers_per_line() * bytes_per_transfer
    }

    pub fn active_lines(&self) -> u32 {
        (self.v_end - self.v_start) as u32
    }
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self {
            dedither_filter: false,
            aa_mode: AaMode::Enabled,
            divot: false,
            gamma: false,
            gamma_dither: false,
            pixel_type: PixelType::Blank,
            hsync_width: 0,
            burst_width: 0,
            vsync_width: 0,
            burst_start: 0,
            vsync: 0,
            hsync: 0x7ff,
            leap: 0,
            leap_a: 0,
            leap_b: 0,
            h_start: 0,
            h_end: 0,
            v_start: 0,
            v_end: 0,
            v_burst_start: 0,
            v_burst_end: 0,
            x_scale: Default::default(),
            x_offset: Default::default(),
            y_scale: Default::default(),
            y_offset: Default::default(),
        }
    }
}

impl OutputFormat {
    pub fn set_control(&mut self, ctrl: ViCtrl) {
        self.dedither_filter = ctrl.dedither_filter();
        self.aa_mode = ctrl.aa_mode();
        self.divot = ctrl.divot();
        self.gamma = ctrl.gamma();
        self.gamma_dither = ctrl.gamma_dither();
        self.pixel_type = ctrl.pixel_type();
    }

    pub fn set_burst(&mut self, burst: ViBurst) {
        self.hsync_width = burst.hsync_width();
        self.burst_width = burst.burst_width();
        self.vsync_width = burst.vsync_width();
        self.burst_start = burst.burst_start();
    }

    pub fn set_vsync(&mut self, vsync: ViVSync) {
        self.vsync = vsync.vsync();
    }

    pub fn set_hsync(&mut self, hsync: ViHSync) {
        self.hsync = hsync.hsync();
        self.leap = hsync.leap();
    }

    pub fn set_hsync_leap(&mut self, hsync_leap: ViHSyncLeap) {
        self.leap_a = hsync_leap.leap_a();
        self.leap_b = hsync_leap.leap_b();
    }

    pub fn set_hvideo(&mut self, hvideo: ViHVideo) {
        self.h_start = hvideo.h_start();
        self.h_end = hvideo.h_end();
    }

    pub fn set_vvideo(&mut self, vvideo: ViVVideo) {
        self.v_start = vvideo.v_start();
        self.v_end = vvideo.v_end();
    }

    pub fn set_vburst(&mut self, vburst: ViVBurst) {
        self.v_burst_start = vburst.v_burst_start();
        self.v_burst_end = vburst.v_burst_end();
    }

    pub fn set_xscale(&mut self, xscale: ViScale) {
        self.x_offset = xscale.offset();
        self.x_scale = xscale.scale();
    }

    pub fn set_yscale(&mut self, yscale: ViScale) {
        self.y_offset = yscale.offset();
        self.y_scale = yscale.scale();
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

#[derive(Specifier, Debug, Clone, Copy, PartialEq, Eq)]
#[bits = 2]
pub enum PixelType {
    Blank,
    Reserved,
    Rgb5c3, // 15bit color, 3 bits coverage
    Rgb8a5c3, // 24bit color, 5 bits alpha, 3 bits coverage
}

#[derive(Specifier, Debug, Clone, Copy, PartialEq, Eq)]
#[bits = 2]
pub enum AaMode {
    Enabled,
    EnabledReducedBandwdith,
    ResampleOnly,
    Disabled,
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
    pub offset: Fixed2_10,
    pub scale: Fixed2_10,
}

#[bitfield(bits = 16)]
#[derive(Specifier, Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct Fixed2_10 {
    pub fractional: B10,
    pub integer: B2,
    #[skip] __: B4,
}

impl From<Fixed2_10> for f32 {
    fn from(fixed: Fixed2_10) -> Self {
        fixed.integer() as f32 + fixed.fractional() as f32 / 1024.0
    }
}