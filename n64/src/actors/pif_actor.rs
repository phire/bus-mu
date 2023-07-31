/// PifActor: Emulates the SI (Serial Interface) and the connected PIF


use actor_framework::{Actor, Time, Handler, make_outbox, Outbox, OutboxSend};
use super::{N64Actors, si_actor::{SiPacket, SiActor}};

pub struct PifActor {
    outbox: PifOutbox,
    pif_mem: [u32; 512], // Combined PIF RAN and. Last 16 words are RAM
    state: PifState,
    addr: u16,
    burst: bool,
}

make_outbox!(
    PifOutbox<N64Actors, PifActor> {
        si_packet: SiPacket
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

        PifActor {
            outbox: Default::default(),
            pif_mem,
            state: PifState::WaitCmd,
            addr: 0,
            burst: false,
        }
    }
}

impl Actor<N64Actors> for PifActor {
    fn get_message(&mut self) -> &mut actor_framework::MessagePacketProxy<N64Actors> {
        self.outbox.as_mut()
    }

    fn message_delivered(&mut self, time: Time) {
        // Nothing to do?
    }
}

impl PifActor {
    fn read(&mut self, time: Time) {
        let addr = (self.addr & 0x1ff) as usize;
        match self.burst {
            false => {
                let data = self.pif_mem[addr];
                self.outbox.send::<SiActor>(SiPacket::Data4(data), time);
            }
            true => {
                let data: [u32; 16] = self.pif_mem[addr..(addr + 16)].try_into().unwrap();
                self.outbox.send::<SiActor>(SiPacket::Data64(data), time);
            }
        }
    }

    fn write(&mut self, data: u32) {
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
    fn recv(&mut self, message: SiPacket, time: Time, _limit: Time) {
        match self.state {
            PifState::WaitCmd => {
                match message {
                    SiPacket::Read4(addr) => {
                        self.addr = addr;
                        self.state = PifState::WaitAck;
                        self.burst = false;
                    }
                    SiPacket::Read64(addr) => {
                        self.addr = addr;
                        self.state = PifState::WaitAck;
                        self.burst = true;
                    }
                    SiPacket::Write4(addr) => {
                        self.addr = addr;
                        self.state = PifState::WaitData;
                        self.burst = false;
                    }
                    SiPacket::Write64(addr) => {
                        self.addr = addr;
                        self.state = PifState::WaitData;
                        self.burst = true;
                    }
                    _ => panic!("Unexpected message"),
                }
                // HWTEST: UltraPIF inserts a 4 cycle delay here
                return self.outbox.send::<SiActor>(SiPacket::Ack, time.add(4 * 4))
            }
            PifState::WaitAck => match message {
                SiPacket::Ack => {
                    self.state = PifState::WaitCmd;
                    return self.read(time);
                }
                _ => panic!("Unexpected message "),
            }
            PifState::WaitData => match message {
                SiPacket::Data4(data) => {
                    self.write(data);
                    return self.outbox.send::<SiActor>(SiPacket::Ack, time);
                }
                SiPacket::Data64(data) => {
                    for (i, d) in data.iter().enumerate() {
                        self.write(*d);
                    }
                    return self.outbox.send::<SiActor>(SiPacket::Ack, time);
                }
                _ => panic!("Unexpected message"),
            }
        }
    }
}
