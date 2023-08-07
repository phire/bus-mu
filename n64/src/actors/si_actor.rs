
use std::pin::Pin;

use actor_framework::{Time, Actor, MessagePacketProxy, Handler, Outbox, OutboxSend, SchedulerResult};

use super::{N64Actors, bus_actor::{BusAccept, BusRequest, BusActor}, cpu_actor::{CpuRegRead, CpuActor, ReadFinished, CpuRegWrite, WriteFinished}, pif_actor::PifActor};

pub struct SiActor {
    outbox: SiOutbox,
    buffer: [u32; 16],
    state: SiState,
    next_state: SiState,
    dram_address: u32,
    dma_active: bool,
    error: bool,
    queued_message: Option<(Time, SiPacket)>,
    queued_read: Option<u16>,
}

impl Default for SiActor {
    fn default() -> Self {
        SiActor {
            outbox: Default::default(),
            buffer: [0; 16],
            state: SiState::Idle,
            next_state: SiState::Idle,
            dram_address: 0,
            dma_active: false,
            error: false,
            queued_message: None,
            queued_read: None,
        }
    }
}

actor_framework::make_outbox!(
    SiOutbox<N64Actors, SiActor> {
        bus: BusRequest,
        si_packet: SiPacket,
        cpu: ReadFinished,
        cpu_write: WriteFinished,
    }
);

impl Actor<N64Actors> for SiActor {
    type OutboxType = SiOutbox;

    fn message_delivered(&mut self, outbox: &mut SiOutbox, time: Time) {
        if let Some((msg_time, message)) = self.queued_message.take() {
            println!("Si: sending queued message {:?} to time {} at {}", message, u64::from(msg_time), time);
            self.outbox.send::<PifActor>(message, msg_time);
        } else if self.dma_active {
            self.bus_request(time)
        }
    }
}

impl SiActor {
    fn bus_request(&mut self, time: Time) {
        self.outbox.send::<BusActor>(BusRequest::new::<Self>(1), time);
    }

    fn pif_read(&mut self, pif_addr: u16, time: Time) {
        let mut time64 : u64 = time.into();

        // align with 4 cycle boundary
        time64 = (time64 + 3) & !3;
        time64 += 4 * 12; // The command packet is 11 bits long, with an extra start bit

        println!("SI: PIF Read {:08x} at {}", pif_addr, time64);

        self.next_state = SiState::CpuRead;
        self.state = SiState::WaitAck;

        self.outbox.send::<PifActor>(SiPacket::Read4(pif_addr), time64.into());
    }
}

impl Handler<N64Actors, CpuRegRead> for SiActor {
    fn recv(&mut self, outbox: &mut SiOutbox, message: CpuRegRead, time: Time, _: Time) -> SchedulerResult {
        let address = message.address;
        match address {
            0x0480_0000..=0x048f_ffff => {
                let data = match address & 0x1c {
                    0x00 => { // SI DRAM address
                        self.dram_address
                    }
                    0x04 => { // SI PIF read64
                        unimplemented!()
                    }
                    0x08 => { // SI PIF write4
                        unimplemented!()
                    }
                    0x0c | 0x1c => { // ???
                        unimplemented!()
                    }
                    0x10 => { // SI PIF write 64
                        unimplemented!()
                    }
                    0x14 => { // SI PIF read 4
                        unimplemented!()
                    }
                    0x18 => { // SI status
                        let dma_busy = match self.state {
                            SiState::Idle => 0,
                            _ => 1,
                        };
                        let io_busy = match  self.state {
                            SiState::CpuRead | SiState::CpuWrite => 1,
                            SiState::WaitAck => match self.next_state {
                                SiState::CpuRead | SiState::CpuWrite => 1,
                                _ => 0,
                            }
                            _ => 0,
                        };
                        let value = 0
                          | dma_busy << 0
                          | io_busy << 1
                          // TODO: read pending << 2
                          | (self.error as u32) << 3
                          // TODO: interrupt pending << 12
                          ;
                        println!("SI: Read SI_STATUS = {:08x}", value);
                        value
                    }
                    _ => unreachable!()
                };
                if self.outbox.contains::<SiPacket>() {
                    self.queued_message = Some(self.outbox.cancel());
                }

                self.outbox.send::<CpuActor>(ReadFinished::word(data), time.add(4));
            }
            0x1fc0_0000..=0x1fc0_07ff => { // PIF ROM/RAM
                let pif_address = (address >> 2) as u16 & 0x1ff;

                match self.state {
                    SiState::Idle => {
                        self.pif_read(pif_address, time);
                    }
                    _ => {
                        // Hardware tests indicate PIF will block reads until the previous write finishes.
                        // thanks Tharo - https://discord.com/channels/205520502922543113/768169699564453910/1137213783564099584
                        //
                        // Though IPL2 gets very paranoid when doing this, sometimes inserting NOPS, sometimes
                        // inserting delay loops and sometimes polling the IO_BUSY status bit... so maybe
                        // there is a potential hardware bug around here. Or there was on a previous silicon version.

                        println!("SI: PIF Read {:08x} Blocked on write", address);
                        self.queued_read = Some(pif_address);
                    }
                }
            }
            0x1fc0_0800..=0x1fcf_ffff => {
                // Reserved SI range... not sure what should happen here
                unimplemented!("Si Reserved range")
            }
            _ => { unreachable!() }
        }
        SchedulerResult::Ok
    }
}

impl Handler<N64Actors, CpuRegWrite> for SiActor {
    fn recv(&mut self, outbox: &mut SiOutbox, message: CpuRegWrite, time: Time, _: Time) -> SchedulerResult {
        match self.state {
            SiState::Idle => {}
            _ => panic!("SI is busy")
        }

        let address = message.address;
        let data = message.data;
        match address {
            0x0480_0000..=0x048f_ffff => {
                match address & 0x1c {
                    0x00 => { // SI DRAM address
                        self.dram_address = data;
                    }
                    0x04 => { // SI PIF read64
                        self.next_state = SiState::DmaRead(16);
                        unimplemented!()
                    }
                    0x08 => { // SI PIF write4
                        unimplemented!()
                    }
                    0x0c | 0x1c => { // ???
                        unimplemented!()
                    }
                    0x10 => { // SI PIF write 64
                        self.next_state = SiState::DmaWrite(16);
                        unimplemented!()
                    }
                    0x14 => { // SI PIF read 4
                        unimplemented!()
                    }
                    0x18 => { // SI status
                        unimplemented!()
                    }
                    _ => unreachable!()
                };
                self.outbox.send::<CpuActor>(WriteFinished::word(), time.add(4));
            }
            0x1fc0_0000..=0x1fc0_07ff => { // PIF ROM/RAM
                // Accept the write instantly
                self.outbox.send::<CpuActor>(WriteFinished::word(), time.add(4));

                let pif_address = (address >> 2) as u16 & 0x1ff;
                let mut time64 : u64 = time.into();

                // align with 4 cycle boundary
                time64 = (time64 + 3) & !3;
                time64 += 4 * 12; // The command packet is 11 bits long, with an extra start bit

                println!("SI: Write {:08x} = {:08x} at {}", address, data, time64);

                self.next_state = SiState::CpuWrite;
                self.state = SiState::WaitAck;
                self.buffer[15] = data;

                // Queue this message for after we finish telling the cpu it's write finished
                self.queued_message = Some((time64.into(), SiPacket::Write4(pif_address)));
            }
            0x1fc0_0800..=0x1fcf_ffff => {
                // Reserved SI range... not sure what should happen here
                unimplemented!("Si Reserved range")
            }
            _ => unreachable!()
        }

        SchedulerResult::Ok
    }
}

#[derive(Debug)]
pub enum SiPacket {
    Read4(u16),
    Read64(u16),
    Write4(u16),
    Write64(u16),
    Ack,
    Data4(u32),
    Data64([u32; 16]),
    Finish, // Not a real packet, just used to track when the write to PIF finishes
}

// Handle responses from PIF
impl Handler<N64Actors, SiPacket> for SiActor {
    fn recv(&mut self, outbox: &mut SiOutbox, message: SiPacket, time: Time, _limit: Time) -> SchedulerResult {
        let req_time;
        match message {
            SiPacket::Ack => { // PIF ready to receive our write data
                req_time = time.add(4);
                self.state = match self.next_state {
                    SiState::CpuRead => {
                        self.outbox.send::<PifActor>(SiPacket::Ack, req_time);
                        SiState::CpuRead
                    }
                    SiState::CpuWrite => {
                        self.outbox.send::<PifActor>(SiPacket::Data4(self.buffer[15]), req_time);
                        SiState::CpuWrite
                    }
                    SiState::DmaRead(count) => {
                        self.outbox.send::<PifActor>(SiPacket::Ack, req_time);
                        SiState::DmaRead(count)
                    }
                    _ => unimplemented!()
                };
                return SchedulerResult::Ok;
            }
            SiPacket::Data4(data) => { // 4 byte read finished
                self.buffer[15] = data;
                // PifActor delivers it's response instantly.
                // It's upto SiActor to add delays
                req_time = time.add(1 + 4 + 4 * 32);
            }
            SiPacket::Data64(data) => { // 64 byte read finished
                self.buffer = data;
                req_time = time.add(1 + 4 + 4 * 32);
                self.dma_active = true;
            }
            SiPacket::Finish => {
                if let Some(pif_addr) = self.queued_read.take() {
                    self.pif_read(pif_addr, time);
                } else {
                    self.state = SiState::Idle;
                }
                return SchedulerResult::Ok;
            }
            _ => panic!("Invalid message")
        }
        self.bus_request(req_time);

        SchedulerResult::Ok
    }
}

#[derive(Debug)]
enum SiState {
    CpuRead,
    CpuWrite,
    DmaRead(u8),
    DmaWrite(u8),
    Idle,
    WaitAck,
}

impl Handler<N64Actors, BusAccept> for SiActor {
    fn recv(&mut self, outbox: &mut SiOutbox, _: BusAccept, time: Time, _limit: Time) -> SchedulerResult {
        //let time = time.add(4 * 32);
        self.state = match self.state {
            SiState::CpuRead => {
                self.outbox.send::<CpuActor>(
                    ReadFinished::word(self.buffer[15]),
                    time);
                SiState::Idle
            }
            SiState::DmaRead(1) => {
                self.dma_active = false;
                unimplemented!("Write to RDRAM");
                //SiState::Idle
            }
            SiState::CpuWrite => {
                self.outbox.send::<CpuActor>(WriteFinished::word(), time);
                SiState::Idle
            }
            SiState::DmaWrite(1) => {
                unimplemented!("Read from RDRAM");
                // let data_msg = match self.burst {
                //     true => SiPacket::Data64(self.buffer),
                //     false => SiPacket::Data4(self.buffer[15])
                // };
                // self.outbox.send::<PifActor>(data_msg, time);
                // SiState::Idle
            }
            SiState::DmaRead(count) => {
                unimplemented!("Write to RDRAM {}", count);
                //SiState::DmaRead(count-1)
            }
            SiState::DmaWrite(count) => {
                unimplemented!("Read from RDRAM {}", count);
                //SiState::DmaWrite(count-1)
            }
            SiState::Idle | SiState::WaitAck => {
                unreachable!()
            }
        };
        SchedulerResult::Ok
    }
}


