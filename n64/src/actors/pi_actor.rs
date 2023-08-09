

use actor_framework::*;
use super::{N64Actors, cpu_actor::{ReadFinished, CpuRegRead, CpuActor, CpuRegWrite, WriteFinished}};

pub struct PiActor {
    dram_addr: u32,
    cart_addr: u32,
    domains: [PiDomain; 2],
    rom: Vec<u16>,
}

make_outbox!(
    PiOutbox<N64Actors, PiActor> {
        cpu: ReadFinished,
        cpu_w: WriteFinished,
    }
);

impl Default for PiActor {
    fn default() -> Self {
        let rom_bytes = std::fs::read("n64-systemtest.z64").expect("Error loading n64-systemtest.z64");
        let rom = rom_bytes
            .chunks_exact(2)
            .map(|b| u16::from_be_bytes(b.try_into().unwrap()))
            .collect::<Vec<_>>();

        println!("Loaded rom with {} bytes", rom.len() * 2);

        Self {
            dram_addr: 0,
            cart_addr: 0,
            domains: Default::default(),
            rom,
        }
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
}

impl Actor<N64Actors> for PiActor {
    type OutboxType = PiOutbox;
}

impl Handler<N64Actors, CpuRegWrite> for PiActor {
    fn recv(&mut self, outbox: &mut PiOutbox, message: CpuRegWrite, time: Time, _limit: Time) -> SchedulerResult {
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
        outbox.send::<CpuActor>(WriteFinished::word(), time.add(4));

        SchedulerResult::Ok
    }
}

impl Handler<N64Actors, CpuRegRead> for PiActor {
    fn recv(&mut self, outbox: &mut PiOutbox, message: CpuRegRead, time: Time, _limit: Time) -> SchedulerResult {
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
                todo!("PI_STATUS");
                // let data = 0;
                // println!("PI read PI_STATUS = {:#08x}", data);
                // data
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
        outbox.send::<CpuActor>(ReadFinished::word(data), time.add(4));

        SchedulerResult::Ok
    }
}

impl Handler<N64Actors, PiRead> for PiActor {
    #[inline(always)]
    fn recv(&mut self, outbox: &mut PiOutbox, message: PiRead, time: Time, _limit: Time) -> SchedulerResult {
        if message.cart_addr & 0x3 != 0 {
            panic!("unaligned PI read {:#010x}", message.cart_addr)
        }
        let addr = message.cart_addr & 0xffff_fffc;

        let domain;

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
                domain = &self.domains[0];
                data = self.read_word(addr - 0x1000_0000);
                println!("PI read {:#010x} (ROM) = {:#010x}", addr, data);
            }
            0x1fd0_0000..=0x7fff_ffff => { // Domain 1, but no known devices use this range
                unimplemented!("PI read {:#010x}", addr);
            }
            _ => unreachable!(),
        }

        let cycles = domain.calc_cycles(addr, 2);

        outbox.send::<CpuActor>(ReadFinished::word(data), time.add(cycles));

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
}
