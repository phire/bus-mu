
use actor_framework::{Time, Actor, MessagePacketProxy, Handler, Outbox, OutboxSend, SchedulerResult};

use super::{N64Actors, bus_actor::{BusAccept, BusRequest, BusActor}, cpu_actor::{CpuRegRead, CpuActor, ReadFinished, CpuRegWrite}, pif_actor::PifActor};

pub struct SiActor {
    outbox: SiOutbox,
    buffer: [u32; 16],
    state: SiState,
    dram_address: u32,
    dma_active: bool,
}

impl Default for SiActor {
    fn default() -> Self {
        SiActor {
            outbox: Default::default(),
            buffer: [0; 16],
            state: SiState::Idle,
            dram_address: 0,
            dma_active: false,
        }
    }
}

actor_framework::make_outbox!(
    SiOutbox<N64Actors, SiActor> {
        bus: BusRequest,
        si_packet: SiPacket,
        cpu: ReadFinished
    }
);

impl Actor<N64Actors> for SiActor {
    fn get_message(&mut self) -> &mut MessagePacketProxy<N64Actors> {
        self.outbox.as_mut()
    }

    fn message_delivered(&mut self, time: Time) {
        if self.dma_active {
            self.bus_request(time)
        }
    }
}

impl SiActor {
    fn bus_request(&mut self, time: Time) {
        self.outbox.send::<BusActor>(BusRequest::new::<Self>(1), time);
    }
}

impl Handler<CpuRegRead> for SiActor {
    fn recv(&mut self, message: CpuRegRead, time: Time, limit: Time) -> SchedulerResult {
        match self.state {
            SiState::Idle => {}
            // HWTEST: What should happen when SI is busy?
            //         N64brew suggests bus conflicts.
            _ => panic!("SI is busy")
        }

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
                        unimplemented!()
                    }
                    _ => unreachable!()
                };
                self.outbox.send::<CpuActor>(ReadFinished::word(data), time.add(4));
            }
            0x1fc0_0000..=0x1fc0_07ff => { // PIF ROM/RAM
                let pif_address = (address >> 2) as u16 & 0x1ff;
                let mut time64 : u64 = time.into();

                // align with 4 cycle boundary
                time64 = (time64 + 3) & !3;
                time64 += 4 * 12; // The command packet is 11 bits long, with an extra start bit

                println!("PIF Read {:08x} at {}", address, time64);
                self.state = SiState::CpuRead;

                self.outbox.send::<PifActor>(SiPacket::Read4(pif_address), time64.into());
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

impl Handler<CpuRegWrite> for SiActor {
    fn recv(&mut self, message: CpuRegWrite, time: Time, limit: Time) -> SchedulerResult {
        unimplemented!("SiActor::CpuRegWrite")
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
}

// Handle responses from PIF
impl Handler<SiPacket> for SiActor {
    fn recv(&mut self, message: SiPacket, time: Time, _limit: Time) -> SchedulerResult {
        let req_time;
        match message {
            SiPacket::Ack => { // PIF ready to receive our write data
                req_time = time.add(4);
                match self.state {
                    SiState::CpuRead | SiState::DmaRead(_) => {
                        self.outbox.send::<PifActor>(SiPacket::Ack, req_time);
                        return SchedulerResult::Ok;
                    }
                    _ => {}
                }
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
            _ => panic!("Invalid message")
        }
        self.bus_request(req_time);

        SchedulerResult::Ok
    }
}

enum SiState {
    CpuRead,
    CpuWrite,
    DmaRead(u8),
    DmaWrite(u8),
    Idle,
}

impl Handler<BusAccept> for SiActor {
    fn recv(&mut self, _: BusAccept, time: Time, _limit: Time) -> SchedulerResult {
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
                unimplemented!("Tell cpu write finished");
                //SiState::Idle
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
                unimplemented!("Write to RDRAM");
                //SiState::DmaRead(count-1)
            }
            SiState::DmaWrite(count) => {
                unimplemented!("Read from RDRAM");
                //SiState::DmaWrite(count-1)
            }
            SiState::Idle => {
                unreachable!()
            }
        };
        SchedulerResult::Ok
    }
}


