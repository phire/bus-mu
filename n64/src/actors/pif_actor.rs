/// PifActor: Emulates the SI (Serial Interface) and the connected PIF


use actor_framework::{Actor, MessagePacket, Time, Handler, Addr};
use super::{N64Actors, si_actor::SiPacket};

pub struct PifActor {
    pif_mem: [u32; 512], // Combined PIF RAN and. Last 16 words are RAM
    state: PifState,
    addr: u16,
    burst: bool,
}

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
            pif_mem,
            state: PifState::WaitCmd,
            addr: 0,
            burst: false,
        }
    }
}

impl Actor<N64Actors> for PifActor {
    fn advance(&mut self, limit: Time) -> MessagePacket<N64Actors> {
        MessagePacket::no_message(limit)
    }

    fn advance_to(&mut self, target: Time) {
        let result = self.advance(target);
        assert!(result.is_none());
        assert!(result.time == target);
    }

    fn horizon(&mut self) -> Time {
        // if let Some((time, _)) = self.queued_response {
        //     return time;
        // }

        return Time::max();
    }
}

impl PifActor {
    fn read(&mut self, time: Time) -> MessagePacket<N64Actors> {
        let si_addr: Addr<super::si_actor::SiActor, N64Actors> = Default::default();

        let addr = (self.addr & 0x1ff) as usize;
        match self.burst {
            false => {
                let data = self.pif_mem[addr];
                return si_addr.send(SiPacket::Data4(data), time);
            }
            true => {
                let data: [u32; 16] = self.pif_mem[addr..(addr + 16)].try_into().unwrap();
                return si_addr.send(SiPacket::Data64(data), time);
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

impl Handler<SiPacket, N64Actors> for PifActor {
    fn recv(&mut self, time: Time, message: SiPacket) -> MessagePacket<N64Actors> {
        let si_addr: Addr<super::si_actor::SiActor, N64Actors> = Default::default();

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
                return si_addr.send(SiPacket::Ack, time.add(4 * 4))
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
                    return si_addr.send(SiPacket::Ack, time);
                }
                SiPacket::Data64(data) => {
                    for (i, d) in data.iter().enumerate() {
                        self.write(*d);
                    }
                    return si_addr.send(SiPacket::Ack, time);
                }
                _ => panic!("Unexpected message"),
            }
        }
    }
}
