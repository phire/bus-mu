/// PifActor: Emulates the SI (Serial Interface) and the connected PIF


use actor_framework::{Actor, Time, Handler, make_outbox, Outbox, OutboxSend, SchedulerResult};
use super::{N64Actors, si_actor::{SiPacket, SiActor}};

use crate::{pif, cic};

pub struct PifActor {
    outbox: PifOutbox,
    pif_mem: [u32; 512], // Combined PIF RAN and. Last 16 words are RAM
    state: PifState,
    addr: u16,
    burst: bool,
    enable_rom: bool,
    pif_core: pif::PifHle,
    pif_time: Time,
    cic_core: cic::CicHle,
}

make_outbox!(
    PifOutbox<N64Actors, PifActor> {
        si_packet: SiPacket,
        hle: PifHleMain,
    }
);

impl Default for PifActor {
    fn default() -> Self {
        let pif_rom = std::fs::read("pifdata.bin").expect("Error reading PIF Rom from pifdata.bin");
        let pif_mem: [u32; 512] = pif_rom
            .chunks_exact(4)
            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
            .collect::<Vec<_>>()
            .try_into()
            .expect("Incorrect PIF Rom size");

        let mut outbox = PifOutbox::default();
        outbox.send::<PifActor>(PifHleMain{}, 100.into());

        PifActor {
            outbox,
            pif_mem,
            state: PifState::WaitCmd,
            addr: 0,
            burst: false,
            enable_rom: true,
            pif_core: pif::PifHle::new(),
            pif_time: 0.into(),
            cic_core: cic::CicHle::new(cic::CIC::Nus6101),
        }
    }
}

impl Actor<N64Actors> for PifActor {
    fn get_message(&mut self) -> &mut actor_framework::MessagePacketProxy<N64Actors> {
        self.outbox.as_mut()
    }

    fn message_delivered(&mut self, _: Time) {
        self.outbox.send::<Self>(PifHleMain{}, self.pif_time);
    }
}

impl PifActor {
    fn read_word(&mut self, addr: usize) -> u32 {
        match addr & 0x1ff {
            0..=495 if self.enable_rom => self.pif_mem[addr],
            496..=511 => self.pif_mem[addr],
            _ => 0, // ROM returns zeros when disabled
        }
    }

    fn read(&mut self, time: Time) {
        match self.burst {
            false => {
                let data = self.read_word(self.addr as usize);
                println!("PIF: Read {:08x} from {:04x}", data, self.addr);
                self.outbox.send::<SiActor>(SiPacket::Data4(data), time);
            }
            true => {
                let data: [u32; 16] = core::array::from_fn(|i| {
                    self.read_word(self.addr as usize + i)
                });
                self.outbox.send::<SiActor>(SiPacket::Data64(data), time);
            }
        }
    }

    fn write(&mut self, data: u32) {
        println!("PIF: RCP Write {:08x} to {:04x}", data, self.addr);
        if self.addr >= (512 - 16) {
            self.pif_mem[self.addr as usize] = data;
        }
        self.addr += 1;
    }
}

enum PifState {
    WaitCmd,
    WaitAck,
    WaitData,
}

impl Handler<SiPacket> for PifActor {
    fn recv(&mut self, message: SiPacket, time: Time, _limit: Time) -> SchedulerResult {
        match self.state {
            PifState::WaitCmd => {

                let (dir, size) = match message {
                    SiPacket::Read4(addr) => {
                        self.addr = addr;
                        self.state = PifState::WaitAck;
                        self.burst = false;
                        (pif::Dir::Read, pif::Size::Size4)
                    }
                    SiPacket::Read64(addr) => {
                        self.addr = addr;
                        self.state = PifState::WaitAck;
                        self.burst = true;
                        (pif::Dir::Read, pif::Size::Size64)
                    }
                    SiPacket::Write4(addr) => {
                        println!("PIF: Write4 {:04x}", addr);
                        self.addr = addr;
                        self.state = PifState::WaitData;
                        self.burst = false;
                        (pif::Dir::Write, pif::Size::Size4)
                    }
                    SiPacket::Write64(addr) => {
                        self.addr = addr;
                        self.state = PifState::WaitData;
                        self.burst = true;
                        (pif::Dir::Write, pif::Size::Size64)
                    }
                    _ => panic!("Unexpected message"),
                };

                let (pif_core, mut io) = PifHleIoProxy::split(self);
                pif_core.interrupt_a(&mut io, dir, size);


                // HWTEST: UltraPIF inserts a 4 cycle delay here
                //         But n64-systembench indicates it's more like 1800 cycles
                //         This is chaotic, caused by how long it takes for the sm5 core to respond
                //         to an interrupt and halt
                self.outbox.send::<SiActor>(SiPacket::Ack, time.add(450 * 4))
            }
            PifState::WaitAck => match message {
                SiPacket::Ack => {
                    self.state = PifState::WaitCmd;
                    self.read(time);
                }
                _ => panic!("Unexpected message {:?}", message),
            }
            PifState::WaitData => {

                println!("PIF: Waitdata {:?}", message);
                match message {
                SiPacket::Data4(data) => {
                    self.write(data);
                    self.outbox.send::<SiActor>(SiPacket::Finish, time);
                }
                SiPacket::Data64(data) => {
                    for d in data {
                        self.write(d);
                    }
                    self.outbox.send::<SiActor>(SiPacket::Finish, time);
                }
                _ => panic!("Unexpected message {:?}", message),
            }
            }
        }

        SchedulerResult::Ok
    }
}

fn calc_address(address: u32) -> (usize, usize) {
    debug_assert!(address < 0x40, "Address {:x} is out of range", address);
    let word_offset = (512 - 16) + (address >> 2);
    let shift = 31 - (address & 0x3) * 8;
    (word_offset as usize, shift as usize)
}

struct PifHleIoProxy<'a> {
    pif_mem: &'a mut [u32; 512],
    enable_rom: &'a mut bool,
    cic_core: &'a mut cic::CicHle,
}

impl<'a> PifHleIoProxy<'a> {
    fn split(actor: &'a mut PifActor) ->  (&mut pif::PifHle, Self) {
        let io = PifHleIoProxy {
            pif_mem: &mut actor.pif_mem,
            enable_rom: &mut actor.enable_rom,
            cic_core: &mut actor.cic_core,
        };
        (&mut actor.pif_core, io)
    }
}

impl pif::PifIO for PifHleIoProxy<'_> {
    fn read(&self, address: u32) -> u8 {
        let (offset, shift) = calc_address(address);

        (self.pif_mem[offset] >> shift) as u8
    }

    fn write(&mut self, address: u32, value: u8) {
        let (offset, shift) = calc_address(address);
        let mask = !(0xff << shift);
        self.pif_mem[offset] &= mask;
        self.pif_mem[offset] |= (value as u32) << shift;
    }

    fn rom_lockout(&mut self) {
        *self.enable_rom = false;
    }

    fn reset_enable(&mut self) {
        // TODO
    }

    fn cic_read(&mut self) -> u8 {
        self.cic_core.fifo.read()
    }

    fn cic_read_nibble(&mut self) -> u8 {
        self.cic_core.fifo.read_nibble()
    }

    fn cic_write(&mut self, value: u8) {
        self.cic_core.fifo.write(value);
    }

    fn cic_write_nibble(&mut self, value: u8) {
        self.cic_core.fifo.write_nibble(value);
    }

    fn cic_poll(&mut self) {
        self.cic_core.poll();
    }
}

struct PifHleMain {}

impl Handler<PifHleMain> for PifActor {
    fn recv(&mut self, _: PifHleMain, time: Time, _: Time) -> SchedulerResult {
        let (pif_core, mut io) = PifHleIoProxy::split(self);

        self.pif_time = pif_core.main(&mut io, time);

        SchedulerResult::Ok
    }
}
