use std::any::TypeId;

use actor_framework::{Actor, Handler, OutboxSend, SchedulerResult, Time, TimeQueue};

use crate::c_bus::{CBusRead, CBusWrite};

use super::{
    bus_actor::{request_bus, BusActor, BusPair, BusRequest, ReturnBus},
    cpu_actor::{CpuActor, ReadFinished, WriteFinished},
    pif_actor::PifActor,
    N64Actors,
};

pub struct SiActor {
    buffer: [u32; 16],
    state: SiState,
    next_state: SiState,
    dram_address: u32,
    dma_active: bool,
    error: bool,
    queue: TimeQueue<QueuedMessage>,
    queued_read: Option<u16>,
    bus: Option<Box<BusPair>>,
}

impl Default for SiActor {
    fn default() -> Self {
        SiActor {
            buffer: [0; 16],
            state: SiState::Idle,
            next_state: SiState::Idle,
            dram_address: 0,
            dma_active: false,
            error: false,
            queue: TimeQueue::new(),
            queued_read: None,
            bus: None,
        }
    }
}

actor_framework::make_outbox!(
    SiOutbox<N64Actors, SiActor> {
        bus: BusRequest,
        return_bus: Box<BusPair>,
        si_packet: SiPacket,
        cpu: ReadFinished,
        cpu_write: WriteFinished,
    }
);

enum QueuedMessage {
    SiPacket(SiPacket),
    Bus,
}

impl Actor<N64Actors> for SiActor {
    type OutboxType = SiOutbox;

    #[inline(always)]
    fn delivering<Message>(&mut self, outbox: &mut SiOutbox, _: &Message, _: Time)
    where
        Message: 'static,
    {
        if TypeId::of::<Message>() == TypeId::of::<ReadFinished>()
            || TypeId::of::<Message>() == TypeId::of::<WriteFinished>()
        {
            self.finish_bus();
        }
        if let Some((time, msg)) = self.queue.pop() {
            match msg {
                QueuedMessage::SiPacket(packet) => {
                    outbox.send::<PifActor>(packet, time);
                }
                QueuedMessage::Bus => {
                    self.do_bus(outbox, time);
                }
            }
        }
    }
}

impl SiActor {
    fn pif_read(&mut self, outbox: &mut SiOutbox, pif_addr: u16, time: Time) {
        let mut time64: u64 = time.into();

        // align with 4 cycle boundary
        time64 = (time64 + 3) & !3;
        time64 += 4 * 12; // The command packet is 11 bits long, with an extra start bit

        println!("SI: PIF Read {:08x} at {}", pif_addr, time64);

        self.next_state = SiState::CpuRead;
        self.state = SiState::WaitAck;

        outbox.send::<PifActor>(SiPacket::Read4(pif_addr), time64.into());
    }

    fn req_time(&self, time: Time) -> Time {
        time.add(1 + 4 + 4 * 32)
    }
}

impl Handler<N64Actors, CBusRead> for SiActor {
    #[inline(always)]
    fn recv(
        &mut self,
        outbox: &mut SiOutbox,
        message: CBusRead,
        time: Time,
        _: Time,
    ) -> SchedulerResult {
        let address = message.address;
        match address {
            0x0480_0000..=0x048f_ffff => {
                let data = match address & 0x1c {
                    0x00 => {
                        // SI DRAM address
                        self.dram_address
                    }
                    0x04 => {
                        // SI PIF read64
                        unimplemented!()
                    }
                    0x08 => {
                        // SI PIF write4
                        unimplemented!()
                    }
                    0x0c | 0x1c => {
                        // ???
                        unimplemented!()
                    }
                    0x10 => {
                        // SI PIF write 64
                        unimplemented!()
                    }
                    0x14 => {
                        // SI PIF read 4
                        unimplemented!()
                    }
                    0x18 => {
                        // SI status
                        let dma_busy = match self.state {
                            SiState::Idle => 0,
                            _ => 1,
                        };
                        let io_busy = match self.state {
                            SiState::CpuRead | SiState::CpuWrite => 1,
                            SiState::WaitAck => match self.next_state {
                                SiState::CpuRead | SiState::CpuWrite => 1,
                                _ => 0,
                            },
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
                    _ => unreachable!(),
                };
                if outbox.contains::<SiPacket>() {
                    let (time, packet) = outbox.cancel();
                    self.queue.push(time, QueuedMessage::SiPacket(packet));
                }

                outbox.send::<CpuActor>(ReadFinished::word(data), time.add(4));
            }
            0x1fc0_0000..=0x1fc0_07ff => {
                // PIF ROM/RAM
                let pif_address = (address >> 2) as u16 & 0x1ff;

                match self.state {
                    SiState::Idle => {
                        self.pif_read(outbox, pif_address, time);
                    }
                    _ => {
                        // HWTEST: Apparently doing this too soon will return the latched value instead
                        println!("SI: Reading while not idle {:08x}", address);
                        self.queued_read = Some(pif_address);
                    }
                }
            }
            0x1fc0_0800..=0x1fcf_ffff => {
                // Reserved SI range... not sure what should happen here
                unimplemented!("Si Reserved range")
            }
            _ => {
                unreachable!()
            }
        }
        SchedulerResult::Ok
    }
}

impl Handler<N64Actors, CBusWrite> for SiActor {
    fn recv(
        &mut self,
        outbox: &mut SiOutbox,
        message: CBusWrite,
        time: Time,
        _: Time,
    ) -> SchedulerResult {
        match self.state {
            SiState::Idle => {}
            _ => panic!("SI is busy"),
        }

        let address = message.address;
        let data = message.data;
        match address {
            0x0480_0000..=0x048f_ffff => {
                match address & 0x1c {
                    0x00 => {
                        // SI DRAM address
                        self.dram_address = data;
                    }
                    0x04 => {
                        // SI PIF read64
                        self.next_state = SiState::DmaRead(16);
                        unimplemented!()
                    }
                    0x08 => {
                        // SI PIF write4
                        unimplemented!()
                    }
                    0x0c | 0x1c => {
                        // ???
                        unimplemented!()
                    }
                    0x10 => {
                        // SI PIF write 64
                        self.next_state = SiState::DmaWrite(16);
                        unimplemented!()
                    }
                    0x14 => {
                        // SI PIF read 4
                        unimplemented!()
                    }
                    0x18 => {
                        // SI status
                        unimplemented!()
                    }
                    _ => unreachable!(),
                };
                outbox.send::<CpuActor>(WriteFinished::word(), time.add(4));
            }
            0x1fc0_0000..=0x1fc0_07ff => {
                // PIF ROM/RAM
                // Accept the write instantly
                outbox.send::<CpuActor>(WriteFinished::word(), time.add(4));

                let pif_address = (address >> 2) as u16 & 0x1ff;
                let mut time64: u64 = time.into();

                // align with 4 cycle boundary
                time64 = (time64 + 3) & !3;
                time64 += 4 * 12; // The command packet is 11 bits long, with an extra start bit

                println!("SI: Write {:08x} = {:08x} at {}", address, data, time64);

                self.next_state = SiState::CpuWrite;
                self.state = SiState::WaitAck;
                self.buffer[15] = data;

                // Queue this message for after we finish telling the cpu it's write finished
                self.queue.push(
                    time64.into(),
                    QueuedMessage::SiPacket(SiPacket::Write4(pif_address)),
                );
            }
            0x1fc0_0800..=0x1fcf_ffff => {
                // Reserved SI range... not sure what should happen here
                unimplemented!("Si Reserved range")
            }
            _ => unreachable!(),
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
    fn recv(
        &mut self,
        outbox: &mut SiOutbox,
        message: SiPacket,
        time: Time,
        _limit: Time,
    ) -> SchedulerResult {
        let req_time;
        match message {
            SiPacket::Ack => {
                // PIF ready to receive our write data
                req_time = time.add(4);
                self.state = match self.next_state {
                    SiState::CpuRead => {
                        outbox.send::<PifActor>(SiPacket::Ack, req_time);
                        SiState::CpuRead
                    }
                    SiState::CpuWrite => {
                        outbox.send::<PifActor>(SiPacket::Data4(self.buffer[15]), req_time);
                        SiState::CpuWrite
                    }
                    SiState::DmaRead(count) => {
                        outbox.send::<PifActor>(SiPacket::Ack, req_time);
                        SiState::DmaRead(count)
                    }
                    _ => unimplemented!(),
                };
                return SchedulerResult::Ok;
            }
            SiPacket::Finish => {
                if let Some(addr) = self.queued_read.take() {
                    println!("SI: Queued read {:04x}", addr);
                    self.pif_read(outbox, addr, time);
                } else {
                    self.state = SiState::Idle;
                }
                return SchedulerResult::Ok;
            }
            SiPacket::Data4(data) => {
                // 4 byte read finished
                self.buffer[15] = data;
                // PifActor delivers it's response instantly.
                // It's upto SiActor to add delays
                req_time = self.req_time(time);
            }
            SiPacket::Data64(data) => {
                // 64 byte read finished
                self.buffer = data;
                req_time = self.req_time(time);
                self.dma_active = true;
            }
            _ => panic!("Invalid message"),
        }
        self.do_bus(outbox, req_time)
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

impl Handler<N64Actors, Box<BusPair>> for SiActor {
    fn recv(
        &mut self,
        outbox: &mut SiOutbox,
        bus: Box<BusPair>,
        time: Time,
        _limit: Time,
    ) -> SchedulerResult {
        self.bus = Some(bus);
        self.do_bus(outbox, time)
    }
}

impl SiActor {
    fn do_bus(&self, outbox: &mut SiOutbox, time: Time) -> SchedulerResult {
        let _ = match self.bus.as_ref() {
            Some(bus) => bus,
            None => return request_bus(outbox, time),
        };

        match self.state {
            SiState::CpuRead => {
                let data = self.buffer[15];
                outbox.send::<CpuActor>(ReadFinished::word(data), time)
            }
            SiState::DmaRead(1) => {
                unimplemented!("Write to RDRAM");
            }
            SiState::CpuWrite => outbox.send::<CpuActor>(WriteFinished::word(), time),
            SiState::DmaWrite(1) => {
                unimplemented!("Read from RDRAM");
                // let data_msg = match self.burst {
                //     true => SiPacket::Data64(self.buffer),
                //     false => SiPacket::Data4(self.buffer[15])
                // };
                // outbox.send::<PifActor>(data_msg, time);
            }
            SiState::DmaRead(count) => {
                unimplemented!("Write to RDRAM {}", count);
            }
            SiState::DmaWrite(count) => {
                unimplemented!("Read from RDRAM {}", count);
            },
            SiState::Idle | SiState::WaitAck => SchedulerResult::Ok,
        }
    }

    fn finish_bus(&mut self) {
        self.state = match self.state {
            SiState::CpuRead => SiState::Idle,
            SiState::DmaRead(1) => {
                unimplemented!("Write to RDRAM");
                //SiState::Idle
            }
            SiState::CpuWrite => {
                //if self.queued_read.is_some() {
                //    SiState::QueuedRead
                //} else {
                    SiState::Idle
                //}
            }
            SiState::DmaWrite(1) => {
                // TODO: Handle queued read?
                unimplemented!("Read from RDRAM");
            }
            SiState::DmaRead(count) => {
                unimplemented!("Write to RDRAM {}", count);
                //SiState::DmaRead(count-1)
            }
            SiState::DmaWrite(count) => {
                unimplemented!("Read from RDRAM {}", count);
                //SiState::DmaWrite(count-1)
            }
            SiState::Idle => SiState::Idle,
            SiState::WaitAck => SiState::WaitAck,
        };
    }
}

impl Handler<N64Actors, ReturnBus> for SiActor {
    fn recv(
        &mut self,
        outbox: &mut SiOutbox,
        _: ReturnBus,
        time: Time,
        _limit: Time,
    ) -> SchedulerResult {
        if outbox.contains::<ReadFinished>() {
            let (time, _) : (Time, ReadFinished) = outbox.cancel();
            self.queue.push(time, QueuedMessage::Bus);
        }
        if outbox.contains::<WriteFinished>() {
            let (time, _) : (Time, WriteFinished) = outbox.cancel();
            self.queue.push(time, QueuedMessage::Bus);
        } else if outbox.contains::<SiPacket>() {
            let (time, packet) = outbox.cancel();
            self.queue.push(time, QueuedMessage::SiPacket(packet));
        }
        outbox.send::<BusActor>(self.bus.take().unwrap(), time)
    }
}
