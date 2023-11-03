
use std::sync::mpsc;

use self::control::OutputFormat;

mod aa_filter;
mod scaler;
pub mod control;

static VI_CLOCK_RATIO : f64 = 62.5 / 48.4;

fn vi_cycles(rcp_time : u64) -> u64 {
    (rcp_time * 484) / 625
}

fn rcp_cycles(vi_time : u64) -> u64 {
    (vi_time * 625).div_ceil(484)
}

#[derive(Debug)]
pub struct ViCore {
    /// The RCP has a cache for 32 line segments, each large enough for two or four pixels.
    /// This is not long enough to hold even a single line of pixels, even with AA disabled, so
    /// VI needs to be continually DMAing data from main memory
    line_segments: [LineSegment; 32],
    h_pos: u64,
    v_pos: u64,
    format: OutputFormat,

    fb_stride: u32,
    fb_origin: u32,

    fb_line_addr: u32,
    fb_addr: u32,

    h_blank: bool,
    v_blank: bool,
    vi_cycles: u64,

    buffer: TransferBuffer,
    flush_tx: mpsc::Sender<TransferBuffer>,
    buffer_rx: mpsc::Receiver<Vec<u8>>,
}

enum FetchType {
    WithParity(u8),
    WithoutParity(u8),
}

// The fetch state machine appears to runs once every 32 dots (128 VI cycles). At least when AA=11
// and 16bit, as there are different bugs that happen when h_start is before 128 dots and before 32 dots.
// It look like if

enum NextEvent {
    VStart, // Start of first visible line
    HStart, // Start of all other visible lines
    VisableStart, // Start of visible area
    Dma(u32, FetchType),  // Dma transfer
    Prefetch(u32, FetchType), // Prefetch DMA
    Never,
}

impl ViCore {
    pub fn run_vstart(&mut self, rcp_cycle: u64) -> (NextEvent, u64) {
        self.v_pos = self.format.v_start as u64;
        self.h_pos = 0;
        self.v_blank = false;
        self.fb_line_addr = self.fb_origin;
        self.fb_addr = self.fb_line_addr;

        self.vi_cycles = vi_cycles(rcp_cycle);

        (NextEvent::Prefetch(self.fb_addr, FetchType::WithoutParity(4)), rcp_cycle+1)
    }

    pub fn run_hstart(&mut self, rcp_cycle: u64) -> (NextEvent, u64) {
        self.h_pos = 0;
        self.fb_line_addr += self.fb_stride;
        self.fb_addr = self.fb_line_addr;

        self.vi_cycles = vi_cycles(rcp_cycle);

        (NextEvent::Prefetch(self.fb_addr, FetchType::WithoutParity(4)), rcp_cycle+1)
    }

    pub fn run_dma(&mut self, rcp_cycle: u64) -> (NextEvent, u64) {
        let vi_cycles = vi_cycles(rcp_cycle);
        self.h_pos += vi_cycles - self.vi_cycles;
        self.vi_cycles = vi_cycles;

        cycles
    }

    pub fn run_prefetch(&mut self, rcp_cycle: u64, data: &[u8]) -> (NextEvent, u64) {
        let vi_cycles = vi_cycles(rcp_cycle);
        self.h_pos += vi_cycles - self.vi_cycles;
        self.vi_cycles = vi_cycles;

        self.buffer.dma_bytes.extend_from_slice(data);

        (NextEvent::Prefetch(self.fb_addr, FetchType::WithoutParity(8)), rcp_cycle+1)
    }

    pub fn run_visablestart(&mut self, cycles: u64) -> (NextEvent, u64) {
        self.h_pos += cycles;

        cycles
    }

    pub fn format_chagned(&mut self, rcp_cycle: u64, new_format: OutputFormat) -> (NextEvent, u64) {
        self.update_counters(rcp_cycle);

        if self.format != new_format {
            self.format = new_format;
            self.flush_buffer();
        }
        self.next_event(rcp_cycle)
    }

    fn update_counters(&mut self, rcp_cycle: u64) {
        let cycles_per_line = self.format.hsync as u64;
        let cycles_to_end_of_line = cycles_per_line - self.h_pos;

        let mut vi_diff = vi_cycles(rcp_cycle) - self.vi_cycles;
        if vi_diff > cycles_to_end_of_line {
            self.h_pos += vi_diff;
        } else {
            vi_diff -= cycles_to_end_of_line;
            self.h_pos = vi_diff % cycles_per_line;
            let halfline_pos = (1 + self.v_pos + vi_diff / cycles_per_line) * 2;
            self.v_pos = 1 + self.v_pos + (vi_diff / cycles_per_line);
        }

        // Check for v_pos wrap
        let halflines_per_frame = self.format.vsync as u64;
        let current_halfline = self.h_pos / (cycles_per_line / 2); // Either 0 or 1
        let halfline_pos = current_halfline + self.v_pos * 2;
        if halfline_pos > halflines_per_frame {
            self.v_pos = (halfline_pos % halflines_per_frame) / 2;

            // TODO: Handle LEAP for PAL
        }
    }

    fn next_event(&self, rcp_cycle: u64) -> (NextEvent, u64) {
        let vi_cycle = vi_cycles(rcp_cycle);
        if self.format.pixel_type == control::PixelType::Blank {
            return (NextEvent::Never, u64::MAX);
        }
        if self.v_blank {
            (NextEvent::VStart, rcp_cycles(vi_cycle + self.next_vstart()))
        } else {
            if self.h_blank {
                let cycles = (self.format.h_start as u64).checked_sub(self.h_pos)
                    .unwrap_or_else(|| self.next_line());
                (NextEvent::HStart, rcp_cycles(vi_cycle + cycles))
            } else {
                todo!("Format changed outside blanking")
            }
        }
    }

    fn next_line(&self) -> u64 {
        (self.format.hsync as u64).checked_sub(self.h_pos)
            .unwrap_or_else(|| { 0x1000 - self.h_pos }) // counter should eventually wrap
    }

    fn next_vstart(&self) -> u64 {
        let halfline_cycles = self.format.hsync as u64 / 2;
        let current_halfline = self.h_pos / halfline_cycles // Either 0 or 1
            + self.v_pos * 2;

        let halflines = (self.format.v_start as u64).checked_sub(current_halfline)
            .unwrap_or_else(|| {
                self.format.v_start as u64 +
                    (self.format.vsync as u64).checked_sub(current_halfline)
                        .unwrap_or_else(|| 0x400 - current_halfline) // counter should eventually wrap
            });
        halflines * halfline_cycles + self.next_line()
    }

    fn flush_buffer(&mut self) {
        let mut buffer = TransferBuffer {
            format: self.format,
            v_pos: self.v_pos as u16,
            h_pos: self.h_pos as u16,
            dma_bytes: self.get_byte_buffer(),
        };
        core::mem::swap(&mut buffer, &mut self.buffer);
        self.flush_tx.send(buffer).unwrap();
    }

    fn get_byte_buffer(&self) -> Vec<u8> {
        // Try to reuse empty buffers when possible, otherwise allocate a new buffer
        let buffer = match self.buffer_rx.try_recv() {
            Ok(buffer) => buffer,
            Err(mpsc::TryRecvError::Empty) => Vec::new(),
            Err(mpsc::TryRecvError::Disconnected) => panic!("VI buffer channel disconnected"),
        };

        buffer
    }
}

pub struct ViResolver {
    flush_rx: mpsc::Receiver<TransferBuffer>,
    buffer_tx: mpsc::Sender<Vec<u8>>,
}

pub fn new() -> (ViCore, ViResolver) {
    let (flush_tx, flush_rx) = mpsc::channel();
    let (buffer_tx, buffer_rx) = mpsc::channel();

    let core = ViCore {
        line_segments: Default::default(),
        h_pos: 0,
        v_pos: 0,
        vi_cycles: 0,
        format: Default::default(),
        buffer: TransferBuffer {
            format: Default::default(),
            h_pos: 0,
            v_pos: 0,
            dma_bytes: Vec::new(),
        },
        flush_tx,
        buffer_rx,
        h_blank: true,
        v_blank: true,
    };

    let resolver = ViResolver {
        flush_rx,
        buffer_tx,
    };
    (core, resolver)
}

/// Each line segment is 9 bytes, so can hold 2 32bit pixels, or 4 18bit pixels.
#[derive(Debug, Default)]
pub struct LineSegment {
    tag: u32, // Presumably there is some tag
    data: [u8; 9],
}

/// The output buffer only contains the raw data DMAed out of memory, and any configuration needed
/// reconstruct a final image for display.
///
/// This limits the amount of work needed on the main emulation thread, just the DMA copy.
#[derive(Debug)]
pub struct TransferBuffer {
    format: OutputFormat,
    h_pos: u16,
    v_pos: u16,
    dma_bytes: Vec<u8>,
}
