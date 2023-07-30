
use actor_framework::{Time, Actor, MessagePacketProxy, Handler, Outbox, OutboxSend};

use super::{N64Actors, bus_actor::{BusAccept, BusRequest, BusActor}};

pub struct SiActor {
    outbox: SiOutbox,
    buffer: [u32; 16],
    state: SiState,
    burst: bool,
    cpu: bool,
}

impl Default for SiActor {
    fn default() -> Self {
        SiActor {
            outbox: Default::default(),
            buffer: [0; 16],
            state: SiState::Idle,
            burst: false,
            cpu: false,
        }
    }
}

actor_framework::make_outbox!(
    SiOutbox<N64Actors, SiActor> {
        bus: BusRequest,
        si_packet: SiPacket
    }
);

impl Actor<N64Actors> for SiActor {
    fn get_message(&mut self) -> &mut MessagePacketProxy<N64Actors> {
        self.outbox.as_mut()
    }

    fn message_delivered(&mut self, time: &Time) {
        todo!()
    }
}

impl SiActor {
    fn bus_request(&mut self, time: Time) {
        self.outbox.send::<BusActor>(BusRequest::new::<Self>(1), time);
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
impl Handler<SiPacket> for SiActor {
    fn recv(&mut self, message: SiPacket, time: Time, _limit: Time) {
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

impl Handler<BusAccept> for SiActor {
    fn recv(&mut self, _: BusAccept, time: Time, limit: Time) {
        let time = time.add(4 * 32);
        /*
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
        */
    }
}
