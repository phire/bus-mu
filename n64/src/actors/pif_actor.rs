
/// PifActor: Emulates the SI (Serial Interface) and the connected PIF


use actor_framework::{Actor, Time, Handler, make_outbox, OutboxSend, SchedulerResult, ActorInit};
use anyhow::Context;
use super::{N64Actors, si_actor::{SiPacket, SiActor}};

use crate::{pif, cic, N64Config};

pub struct PifActor {
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

impl Actor<N64Actors> for PifActor {
    type OutboxType = PifOutbox;

    #[inline(always)]
    fn delivering<Message>(&mut self, outbox: &mut PifOutbox, _: &Message, _: Time) {
        let time = self.pif_time;
        outbox.send::<Self>(PifHleMain{}, time);
    }
}

impl ActorInit<N64Actors> for PifActor {
    fn init(config: &N64Config, outbox: &mut Self::OutboxType, time: Time) -> Result<PifActor, anyhow::Error> {
        outbox.send::<Self>(PifHleMain{}, time);

        let pif_rom = std::fs::read(&config.pif_data)
            .with_context(|| format!("Failed to read pif_rom from {}", config.pif_data.display()))?;
        let pif_mem: [u32; 512] = pif_rom
            .chunks_exact(4)
            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
            .collect::<Vec<_>>()
            .try_into()
            .expect("Incorrect PIF Rom size");

        Ok(PifActor {
            pif_mem,
            state: PifState::WaitCmd,
            addr: 0,
            burst: false,
            enable_rom: true,
            pif_core: pif::PifHle::new(),
            pif_time: 0.into(),
            cic_core: cic::CicHle::new(cic::CIC::Nus6102),
        })
    }
}

impl PifActor {
    fn read_word(&mut self, addr: usize) -> u32 {
        let offet = addr & 0x1ff;
        match offet {
            0..=495 if self.enable_rom => self.pif_mem[offet],
            496..=511 => self.pif_mem[offet],
            _ => 0, // ROM returns zeros when disabled
        }
    }

    fn read(&mut self, outbox: &mut PifOutbox, time: Time) -> SchedulerResult {
        if self.burst {
            let data: [u32; 16] = core::array::from_fn(|i| {
                self.read_word(self.addr as usize + i)
            });
            outbox.send::<SiActor>(SiPacket::Data64(data), time)
        } else {
            let data = self.read_word(self.addr as usize);
            println!("PIF: Read {:08x} from {:04x}", data, self.addr);
            outbox.send::<SiActor>(SiPacket::Data4(data), time)

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

#[derive(Debug)]
enum PifState {
    WaitCmd,
    WaitAck,
    WaitData,
}

impl Handler<N64Actors, SiPacket> for PifActor {
    fn recv(&mut self, outbox: &mut PifOutbox, message: SiPacket, time: Time, _limit: Time) -> SchedulerResult {
        if outbox.contains::<PifHleMain>() {
            let (_, _msg) : (_, PifHleMain) = outbox.cancel();
        }

        if outbox.contains::<SiPacket>() {
            let (old_time, old_msg) : (_, SiPacket) = outbox.cancel();

            panic!("PIF: {:?} stompted {:?} during {:?} @ {}", message, old_msg, self.state, old_time);
        }

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
                        println!("PIF: Read64 {:04x}", addr);
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

                //let (pif_core, mut io) = PifHleIoProxy::split(self);
                let enable_rom = &mut self.enable_rom;
                let cic_core = &mut self.cic_core;

                let mut io = PifHleIoProxy {
                    pif_mem: &mut self.pif_mem,
                    enable_rom: enable_rom,
                    cic_core: cic_core,
                };
                self.pif_core.interrupt_a(&mut io, dir, size);

                // HWTEST: UltraPIF inserts a 4 cycle delay here
                //         But n64-systembench indicates it's more like 1800 cycles
                //         This is chaotic, caused by how long it takes for the sm5 core to respond
                //         to an interrupt and halt
                outbox.send::<SiActor>(SiPacket::Ack, time.add(450 * 4))
            }
            PifState::WaitAck => match message {
                SiPacket::Ack => {
                    self.state = PifState::WaitCmd;
                    self.read(outbox, time)
                }
                _ => panic!("Unexpected message {:?}", message),
            }
            PifState::WaitData => {
                println!("PIF: Waitdata {:?}", message);
                match message {
                    SiPacket::Data4(data) => {
                        self.write(data);
                    }
                    SiPacket::Data64(data) => {
                        for d in data {
                            self.write(d);
                        }
                    }
                    _ => panic!("Unexpected message {:?}", message),
                }

                self.state = PifState::WaitCmd;
                outbox.send::<SiActor
                >(SiPacket::Finish, time)
            }
        }
    }
}

fn calc_address(address: u32) -> (usize, usize) {
    debug_assert!(address < 0x40, "Address {:x} is out of range", address);
    let word_offset = (512 - 16) + (address >> 2);
    let shift = 24 - (address & 0x3) * 8;
    (word_offset as usize, shift as usize)
}

struct PifHleIoProxy<'a> {
    pif_mem: &'a mut [u32; 512],
    enable_rom: &'a mut bool,
    cic_core: &'a mut cic::CicHle,
}

impl<'a> PifHleIoProxy<'a> {
    fn split(actor: &'a mut PifActor) ->  (&'a mut pif::PifHle, PifHleIoProxy<'a>) {
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

impl Handler<N64Actors, PifHleMain> for PifActor {
    #[inline(always)]
    fn recv(&mut self, _outbox: &mut PifOutbox,  _: PifHleMain, time: Time, _: Time) -> SchedulerResult {
        let (pif_core, mut io) = PifHleIoProxy::split(self);

        self.pif_time = pif_core.main(&mut io, time);

        SchedulerResult::Ok
    }
}
