

use std::any::TypeId;

use actor_framework::*;
use anyhow::Context;
use crate::{c_bus::{CBusWrite, CBusRead, ReadFinished, WriteFinished}, d_bus::DBus, N64Config};

use super::{N64Actors, cpu_actor::CpuActor, bus_actor::{BusPair, request_bus, ReturnBus, BusRequest, BusActor}};

pub struct PiActor {
    dram_addr: u32,
    cart_addr: u32,
    wr_len: u32,
    rd_len: u32,
    dma_status: DmaStatus,
    queued_dma_event: Time,
    domains: [PiDomain; 2],
    rom: Vec<u16>,
    bus: Option<Box<BusPair>>,
}

make_outbox!(
    PiOutbox<N64Actors, PiActor> {
        finish_read: ReadFinished,
        finish_write: WriteFinished,
        dma: DmaTransfer,
        bus: BusRequest,
        return_bus: Box<BusPair>,
    }
);

impl ActorInit<N64Actors> for PiActor {
    fn init(config: &N64Config, _: &mut PiOutbox, _: Time) -> Result<Self, anyhow::Error> {
        let rom_path = config.rom.clone().ok_or(anyhow::anyhow!("No rom specified"))?;

        let rom_bytes = std::fs::read(&rom_path)
            .with_context(|| format!("Failed to read rom file: {}", rom_path.display()))?;

        let rom = rom_bytes
            .chunks_exact(2)
            .map(|b| u16::from_be_bytes(b.try_into().unwrap()))
            .collect::<Vec<_>>();

        println!("Loaded rom with {} bytes", rom.len() * 2);

        Ok(Self {
            dram_addr: 0,
            cart_addr: 0,
            wr_len: 0,
            rd_len: 0,
            dma_status: DmaStatus::Idle,
            queued_dma_event: Time::MAX,
            domains: Default::default(),
            rom,
            bus: None,
        })
    }
}

impl PiActor {
    fn read_word(&self, address: u32) -> u32 {
        let address = (address / 2) as usize;
        if address >= self.rom.len() {
            panic!("Read out of bounds: {:#010x}", address);
        }
        (self.rom[address] as u32) << 16 | (self.rom[address + 1] as u32)
    }

    fn read_dword(&self, address: u32) -> u64 {
        let address = (address / 2) as usize;
        if address >= self.rom.len() {
            panic!("Read out of bounds: {:#010x}", address);
        }

        (self.rom[address] as u64) << 48
          | (self.rom[address + 1] as u64) << 32
          | (self.rom[address + 2] as u64) << 16
          | (self.rom[address + 3] as u64)
    }

    fn domain(&self, addr: u32) -> &PiDomain {
        match addr {
            0x0800_0000..=0x0fff_ffff => &self.domains[1], // Cartridge SRAM/FlashRAM (Domain 2)
            _ => &self.domains[0], // Everything else is Domain 1 (or unreachable)
        }
    }

    fn dma_page_bytes(&self) -> u32 {
        let bytes = match self.dma_status {
            DmaStatus::Idle => 0,
            DmaStatus::Writing => self.wr_len + 1,
            DmaStatus::Reading => self.rd_len + 1,
        };

        let domain = self.domain(self.cart_addr);
        domain.clamped_bytes(self.cart_addr, bytes)
    }

    fn dma_event_time(&self, time: Time) -> Time {
        let domain = self.domain(self.cart_addr);
        let bytes = self.dma_page_bytes();

        assert!(bytes % 2 == 0); // TODO, what happens here

        if bytes == 0 {
            Time::MAX
        } else {
            let cycles = domain.calc_cycles(self.cart_addr, bytes as u64 / 2);
            time.add(cycles)
        }
    }

    fn clear_outbox(&mut self, outbox: &mut PiOutbox) {
        if let Some((dma_time, _)) = outbox.try_cancel::<DmaTransfer>() {
            self.queued_dma_event = dma_time;
        }

        assert!(outbox.is_empty());
    }
}

impl Actor<N64Actors> for PiActor {
    type OutboxType = PiOutbox;

    #[inline(always)]
    fn delivering<Message>(&mut self, outbox: &mut PiOutbox, _: &Message, _: Time)
    where
        Message: 'static,
    {
        if TypeId::of::<Message>() != TypeId::of::<DmaTransfer>() && self.queued_dma_event != Time::MAX {
            outbox.send::<Self>(DmaTransfer, self.queued_dma_event);
            self.queued_dma_event = Time::MAX;
        }
    }
}

impl Handler<N64Actors, CBusWrite> for PiActor {
    fn recv(&mut self, outbox: &mut PiOutbox, message: CBusWrite, time: Time, _limit: Time) -> SchedulerResult {
        self.clear_outbox(outbox);

        let data = message.data;
        let n = (message.address >> 3) as usize & 1;
        match message.address & 0x3c {
            0x00 => { // PI_DRAM_ADDR
                println!("PI write PI_DRAM_ADDR = {:#010x}", data);
                self.dram_addr = data & 0x00ff_fffe;
            }
            0x04 => { // PI_CART_ADDR
                println!("PI write PI_CART_ADDR = {:#010x}", data);
                self.cart_addr = data & 0xffff_fffe;
            }
            0x08 => { // PI_RD_LEN
                todo!("PI_RD_LEN")
            }
            0x0c => { // PI_WR_LEN
                println!("PI write PI_WR_LEN = {:#010x}", data);
                self.wr_len = data & 0x00ff_ffff;
                self.dma_status = DmaStatus::Writing;
                self.queued_dma_event = self.dma_event_time(time);
                assert!(self.queued_dma_event != Time::MAX);
                println!("  {} queued dma event at {}", time, self.queued_dma_event)
            }
            0x10 => { // PI_STATUS
                println!("PI write PI_STATUS = {:#010x}", data);
                if data & 0x1 != 0 {
                    println!("  reset dma");
                    self.queued_dma_event = Time::MAX;
                    self.dma_status = DmaStatus::Idle;
                }
                if data & 0x2 != 0 {
                    println!("  clear interrupt");
                }
            }
            0x14 | 0x24 => { // PI_BSD_DOMn_LAT
                println!("PI write PI_BSD_DOM{}_LAT = {:#010x}", n, data);
                self.domains[n].latency = data as u8;
            }
            0x18 | 0x28 => { // PI_BSD_DOM1_PWD
                println!("PI write PI_BSD_DOM{}_PWD = {:#010x}", n, data);
                self.domains[n].pulse_width = data as u8;
            }
            0x1c | 0x2c => { // PI_BSD_DOM1_PGS
                println!("PI write PI_BSD_DOM{}_PGS = {:#010x}", n, data);
                self.domains[n].page_size = data as u8 & 0xf;
            }
            0x20 | 0x30 => { // PI_BSD_DOM1_RLS
                println!("PI write PI_BSD_DOM{}_RLS = {:#010x}", n, data);
                self.domains[n].release = data as u8 & 0x3;
            }
            0x34 | 0x38 | 0x3c => {
                unimplemented!()
            }
            _ => unreachable!(),
        }
        outbox.send::<CpuActor>(WriteFinished {}, time.add(4));

        SchedulerResult::Ok
    }
}

impl Handler<N64Actors, CBusRead> for PiActor {
    fn recv(&mut self, outbox: &mut PiOutbox, message: CBusRead, time: Time, _limit: Time) -> SchedulerResult {
        self.clear_outbox(outbox);

        let n = (message.address >> 3) as usize & 1;
        let data = match message.address & 0x3c {
            0x00 => { // PI_DRAM_ADDR
                println!("PI read PI_DRAM_ADDR = {:#08x}", self.dram_addr);
                self.dram_addr
            }
            0x04 => { // PI_CART_ADDR
                println!("PI read PI_CART_ADDR = {:#010x}", self.cart_addr);
                self.cart_addr
            }
            0x08 => { // PI_RD_LEN
                println!("PI read PI_RD_LEN = 0x7f");
                0x7f // N64brew: "Reading appears to always return `0x7F` (more research required)"
            }
            0x0c => { // PI_WR_LEN
                println!("PI read PI_WR_LEN = 0x7f");
                0x7f // N64brew: "Reading appears to always return `0x7F` (more research required)"
            }
            0x10 => { // PI_STATUS
                let mut data = 0;

                match self.dma_status {
                    DmaStatus::Idle => {}
                    DmaStatus::Writing | DmaStatus::Reading => data |= 0x3, // IO busy, DMA busy
                }

                // TODO: Interrupts

                //println!("PI read PI_STATUS = {:#08x}", data);
                data
            }
            0x14 | 0x24 => { // PI_BSD_DOMn_LAT
                println!("PI read PI_BSD_DOM{}_LAT = {:#010x}", n, self.domains[n].latency);
                self.domains[n].latency as u32
            }
            0x18 | 0x28 => { // PI_BSD_DOMn_PWD
                println!("PI read PI_BSD_DOM{}_PWD = {:#010x}", n, self.domains[n].pulse_width);
                self.domains[n].pulse_width as u32
            }
            0x1c | 0x2c => { // PI_BSD_DOMn_PGS
                println!("PI read PI_BSD_DOM{}_PGS = {:#010x}", n, self.domains[n].page_size);
                self.domains[n].page_size as u32
            }
            0x20 | 0x30 => { // PI_BSD_DOMn_RLS
                println!("PI read PI_BSD_DOM{}_RLS = {:#010x}", n, self.domains[n].release);
                self.domains[n].release as u32
            }
            0x34 | 0x38 | 0x3c => {
                unimplemented!()
            }
            _ => unreachable!(),
        };
        outbox.send::<CpuActor>(ReadFinished { data }, time.add(4));

        SchedulerResult::Ok
    }
}

impl Handler<N64Actors, PiRead> for PiActor {
    #[inline(always)]
    fn recv(&mut self, outbox: &mut PiOutbox, message: PiRead, time: Time, _limit: Time) -> SchedulerResult {
        assert!(self.dma_status == DmaStatus::Idle);

        if message.cart_addr & 0x3 != 0 {
            panic!("unaligned PI read {:#010x}", message.cart_addr)
        }
        let addr = message.cart_addr & 0xffff_fffc;

        let domain = self.domain(addr);

        let data;

        match addr {
            0x0500_0000..=0x05ff_ffff => { // N64DD I/O registers (Domain 1)
                unimplemented!("PI read {:#010x} (N64DD I/O registers)", addr);
            }
            0x0600_0000..=0x07ff_ffff => { // N64DD IPL ROM (Domain 1)
                unimplemented!("PI read {:#010x} (N64DD IPL ROM)", addr);
            }
            0x0800_0000..=0x0fff_ffff => { // Cartridge SRAM/FlashRAM (Domain 2)
                unimplemented!("PI read {:#010x} (Cartridge SRAM/FlashRAM)", addr);
            }
            0x1000_0000..=0x17ff_ffff => { // Cartridge ROM (Domain 1)
                data = self.read_word(addr - 0x1000_0000);
                println!("PI read {:#010x} (ROM) = {:#010x}", addr, data);
            }
            0x1fd0_0000..=0x7fff_ffff => { // Domain 1, but no known devices use this range
                unimplemented!("PI read {:#010x}", addr);
            }
            _ => unreachable!(),
        }

        let cycles = domain.calc_cycles(addr, 2);

        outbox.send::<CpuActor>(ReadFinished { data }, time.add(cycles));

        SchedulerResult::Ok
    }
}

impl Handler<N64Actors, PiWrite> for PiActor {
    fn recv(&mut self, _outbox: &mut PiOutbox, message: PiWrite, _time: Time, _limit: Time) -> SchedulerResult {
        todo!("PI write {:#010x} = {:#010x}", message.cart_addr, message.data);
    }
}

#[derive(Debug)]
pub struct PiRead {
    cart_addr: u32,
}

impl PiRead {
    pub fn new(cart_addr: u32) -> Self {
        Self {
            cart_addr,
        }
    }
}

#[derive(Debug)]
pub struct PiWrite {
    cart_addr: u32,
    data: u32,
}

impl PiWrite {
    pub fn new(cart_addr: u32, data: u32) -> Self {
        Self {
            cart_addr,
            data,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DmaStatus {
    Idle,
    Reading,
    Writing,
}

struct DmaTransfer;

impl PiActor {

    fn do_write(&mut self, d_bus: &mut DBus) -> u64 {

        let mut mask = !0u64;
        let mut src_addr = self.cart_addr;

        let domain = self.domain(src_addr);
        let bytes = domain.clamped_bytes(src_addr, self.wr_len + 1);

        let mut dram_addr = self.dram_addr;

        //println!("PI write {:#010x} -> {:#010x} ({} bytes)", src_addr, dram_addr, bytes);

        let misalignment = dram_addr & 0x7;
        let aligned_bytes = if misalignment != 0 {
            // First transfer is misaligned in dram and needs a mask
            mask = !0u64 >> (misalignment * 8);
            dram_addr -= misalignment;
            src_addr -= misalignment;
            bytes + misalignment
        } else {
            bytes
        };

        // The PI has a 128 bytes of buffer, enough to do a 16 transfer burst
        let aligned_bytes = aligned_bytes.min(128);
        let transfer_count = u64::from((aligned_bytes + 7) / 8);

        let mut remaining_bytes = aligned_bytes;

        while remaining_bytes >= 8 {
            let data = self.read_dword(src_addr - 0x1000_0000);
            if mask != !0u64 {
                d_bus.write_qword_masked(dram_addr, data, mask);
                mask = !0u64;
            } else {
                d_bus.write_qword(dram_addr, data);
            }

            dram_addr += 8;
            src_addr += 8;
            remaining_bytes -= 8;
        }

        if remaining_bytes != 0 {
            let data = self.read_dword(src_addr - 0x1000_0000);
            // Last transfer is less than 8 bytes and needs a mask
            mask &= !0u64 << ((8 - remaining_bytes) * 8);

            d_bus.write_qword_masked(dram_addr, data, mask);
            dram_addr += remaining_bytes;
            src_addr += remaining_bytes;
        }

        self.dram_addr = dram_addr;
        self.cart_addr = src_addr;

        if self.wr_len <= bytes {
            // DMA finished
            self.wr_len = 0;
            self.dma_status = DmaStatus::Idle;
        } else {
            self.wr_len -= bytes;
        }

        transfer_count
    }

    fn do_dma(&mut self, outbox: &mut PiOutbox, d_bus: &mut DBus, time: Time) -> SchedulerResult {
        let transfer_count = match self.dma_status {
            DmaStatus::Writing => self.do_write(d_bus),
            DmaStatus::Reading => todo!("DRAM -> PI transfer"),
            DmaStatus::Idle => unreachable!(),
        };

        if self.dma_status != DmaStatus::Idle {
            let next_time = self.dma_event_time(time.add(transfer_count));
            outbox.send::<Self>(DmaTransfer, next_time);
        }

        SchedulerResult::Ok
    }
}

impl Handler<N64Actors, DmaTransfer> for PiActor {
    fn recv(&mut self, outbox: &mut PiOutbox, _: DmaTransfer, time: Time, _limit: Time) -> SchedulerResult {
        match self.bus.take() {
            Some(mut bus) => {
                let result = self.do_dma(outbox, &mut bus.d_bus, time);
                self.bus = Some(bus);
                result
            }
            None => request_bus(outbox, time),
        }
    }
}

impl Handler<N64Actors, Box<BusPair>> for PiActor {
    fn recv(&mut self, outbox: &mut PiOutbox, mut bus: Box<BusPair>, time: Time, _: Time) -> SchedulerResult
    {
        let result = self.do_dma(outbox, &mut bus.d_bus, time);
        self.bus = Some(bus);

        result
    }
}

impl Handler<N64Actors, ReturnBus> for PiActor {
    fn recv(&mut self, outbox: &mut PiOutbox, _: ReturnBus, time: Time, _: Time) -> SchedulerResult {
        self.clear_outbox(outbox);

        outbox.send::<BusActor>(self.bus.take().unwrap(), time)
    }
}

#[derive(Debug, Default)]
struct PiDomain {
    latency: u8,
    pulse_width: u8,
    page_size: u8,
    release: u8,
}

impl PiDomain {
    fn calc_cycles(&self, addr: u32, hwords: u64) -> u64 {
        let offset = (addr as u64 / 2) % (self.page_size as u64 + 1);
        let pages = (hwords + offset) / (self.page_size as u64 + 1);

        let page_cycles = 14 + (self.latency as u64 + 1);
        let word_cycles = (self.pulse_width as u64 + 1) + (self.release as u64 + 1);

        page_cycles * pages + word_cycles * hwords
    }

    fn clamped_bytes(&self, addr: u32, bytes: u32) -> u32 {
        let page_size = (self.page_size as u32 + 1) << 1;
        let offset = addr % page_size;

        (bytes + offset).min(page_size) - offset
    }
}
