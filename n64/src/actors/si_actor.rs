
use actor_framework::{MessagePacket, Time, Actor, Handler, Addr};
use super::{N64Actors, bus_actor::{BusAccept, BusRequest}};


pub struct SiActor {
    buffer: [u32; 16],
    state: SiState,
    burst: bool,
    cpu: bool,
    pif_addr: Addr<super::pif_actor::PifActor, N64Actors>,
    bus_addr: Addr<super::bus_actor::BusActor, N64Actors>,
}

impl Default for SiActor {
    fn default() -> Self {
        SiActor {
            buffer: [0; 16],
            state: SiState::Idle,
            burst: false,
            cpu: false,
            pif_addr: Default::default(),
            bus_addr: Default::default(),
        }
    }
}

impl Actor<N64Actors> for SiActor {
    fn advance(&mut self, limit: Time) -> MessagePacket<N64Actors> {
        MessagePacket::no_message(limit)
    }

    fn advance_to(&mut self, target: Time) {
        let result = self.advance(target);
        assert!(result.is_none());
        assert!(result.time == target);
    }

    fn horizon(&mut self) -> Time {
        return Time::max();
    }
}

impl SiActor {
    fn bus_request(&self, time: Time) -> MessagePacket<N64Actors> {
        self.bus_addr.send(BusRequest::new::<Self>(1), time)
    }
}

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
impl Handler<SiPacket, N64Actors> for SiActor {
    fn recv(&mut self, time: Time, message: SiPacket) -> MessagePacket<N64Actors> {
        let req_time;
        match message {
            SiPacket::Ack => { // PIF ready to receive our write data
                req_time = time.add(1);
            }
            SiPacket::Data4(data) => { // 4 byte read finished
                self.buffer[15] = data;
                req_time = time.add(1 + 4 + 4 * 32);

                // PifActor doesn't take into account the time it takes to send data over the serial link
            }
            SiPacket::Data64(data) => { // 64 byte read finished
                self.buffer = data;
                req_time = time.add(1 + 4 + 4 * 32);
            }
            _ => panic!("Invalid message")
        }
        return self.bus_request(req_time);
    }
}

enum SiState {
    CpuRead,
    CpuWrite,
    DmaRead(u8),
    DmaWrite(u8),
    Idle,
}

impl Handler<BusAccept, N64Actors> for SiActor {
    fn recv(&mut self, time: Time, _: BusAccept) -> MessagePacket<N64Actors> {
        let time = time.add(4 * 32);
        let (state, msg) = match self.state {
            SiState::CpuRead => {
                (SiState::Idle, MessagePacket::no_message(time))
            }
            SiState::DmaRead(1) => {
                (SiState::Idle, MessagePacket::no_message(time))
            }
            SiState::CpuWrite => {
                (SiState::Idle, MessagePacket::no_message(time))
            }
            SiState::DmaWrite(1) => {
                let data_msg = match self.burst {
                    true => SiPacket::Data64(self.buffer),
                    false => SiPacket::Data4(self.buffer[15])
                };
                (SiState::Idle, self.pif_addr.send(data_msg, time))
            }
            SiState::DmaRead(count) => {
                (SiState::DmaRead(count-1), MessagePacket::no_message(time))
            }
            SiState::DmaWrite(count) => {
                (SiState::DmaWrite(count-1), MessagePacket::no_message(time))
            }
            SiState::Idle => {
                unreachable!()
            }
        };
        self.state = state;
        return msg;
    }
}
