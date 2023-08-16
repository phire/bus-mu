/// I'm mostly copying this implementation from Ares:
///     Copyright (c) 2004-2021 ares team, Near et al
///     Permission to use, copy, modify, and/or distribute this software for any
///     purpose with or without fee is hereby granted, provided that the above
///     copyright notice and this permission notice appear in all copies.
///
///     THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
///     WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
///     MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
///     ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
///     WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
///     ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
///     OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.


/// This is a quick and dirty HLE implementation of the PIF SM5 core
/// I'm just wanting to get enough so I can finish booting, I'll come back to do SM5 LLE later
/// This doesn't implement joybus at all


use actor_framework::Time;

use super::{Dir, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Init,
    WaitLockout,
    WaitGetChecksum,
    WaitCheckChecksum,
    WaitTerminateBoot,
    Run,
    Error
}

struct InternalRam {
    os_info: [u8; 3],
    cpu_checksum: [u8; 6],
    cic_checksum: [u8; 6],
    boot_timeout: Time,
    _joy_address: [u8; 5],
}

pub trait PifIO {
    fn read(&self, address: u32) -> u8;
    fn write(&mut self, address: u32, value: u8);
    fn rom_lockout(&mut self);
    fn reset_enable(&mut self);

    fn cic_poll(&mut self);
    fn cic_read(&mut self) -> u8;
    fn cic_read_nibble(&mut self) -> u8;
    fn cic_write(&mut self, value: u8);
    fn cic_write_nibble(&mut self, value: u8);
}

pub struct PifHle {
    state: State,
    internal_ram: InternalRam,
}

impl dyn PifIO + '_ {
    fn swap(&mut self, addr: u32, other: &mut u8) {
        let mut data = self.read(addr);
        std::mem::swap(other, &mut data);
        self.write(addr, data)
    }

    fn read_command(&mut self) -> u8 {
        return self.read(0x3f);
    }

    fn write_command(&mut self, value: u8) {
        self.write(0x3f, value);
    }
}

fn descramble(buffer: &mut [u8])
{
    for i in (1..buffer.len()).rev() {
        buffer[i] = buffer[i].wrapping_sub(buffer[i - 1] + 1) & 0xf;
    }
}


impl PifHle {
    pub fn new() -> Self {
        PifHle {
            state: State::Init,
            internal_ram: InternalRam {
                os_info: [0; 3],
                cpu_checksum: [0; 6],
                cic_checksum: [0; 6],
                boot_timeout: Time::MAX,
                _joy_address: [0; 5],
            }
        }
    }

    fn swap_secrets(&mut self, io: &mut dyn PifIO) {
        for i in 0..3 {
            io.swap(0x25 + i, &mut self.internal_ram.os_info[i as usize]);
        }
        for i in 0..6 {
            io.swap(0x32 + i, &mut self.internal_ram.cpu_checksum[i as usize]);
        }
    }

    pub fn main(&mut self, io: &mut dyn PifIO, time: Time) -> Time{
        let next_time = time.add(13653);
        println!("PIF: main {:?} cmd: {:02x} @ {}", self.state, io.read_command(), time);
        let initial_state = self.state;

        match self.state {

            State::Init  => {
                io.cic_poll();
                let hello = io.cic_read_nibble();
                if hello & 0x3 != 0x1 {
                    println!("PIF: invalid CIC hello: {:x}", hello);
                    self.state = State::Error;
                    return next_time;
                }

                // TODO: region check
                let is_dd  = hello & 0x4 != 0;
                let os_info = 0
                    | 1 << 2 // "version" bit (unknown, always set)
                    | (is_dd as u8) << 3; // 64dd

                io.cic_poll();
                let mut buf = std::array::from_fn::<u8, 6, _>(|_| io.cic_read_nibble());
                for _ in 0..2 {
                    descramble(&mut buf);
                }

                self.internal_ram.os_info[0] = buf[0] << 4 | os_info;
                self.internal_ram.os_info[1] = buf[2] << 4 | buf[3];
                self.internal_ram.os_info[2] = buf[4] << 4 | buf[5];
                assert!(self.internal_ram.os_info[2] == 0x3f);
                self.swap_secrets(io);  //show osinfo+seeds in external memory

                io.write_command(0x00);
                self.state = State::WaitLockout;
            }
            State::WaitLockout => {
                if io.read_command() & 0x10 != 0 {
                    io.rom_lockout();
                    // TODO: Joy init
                    self.state = State::WaitGetChecksum;
                }
            }
            State::WaitGetChecksum => {
                let current_command = io.read_command();
                if current_command & 0x20 != 0 {
                    self.swap_secrets(io);
                    io.write_command(current_command | 0x80);
                    println!("   CPU checksum: {:x?}", self.internal_ram.cpu_checksum);
                    self.state = State::WaitCheckChecksum;
                }
            }
            State::WaitCheckChecksum => {
                if io.read_command() & 0x40 != 0 {
                    if  true { // only on cold boot
                        io.cic_poll();
                        let mut buf = std::array::from_fn::<u8, 16, _>(|_| io.cic_read_nibble());
                        for _ in 0..4 {
                            descramble(&mut buf);
                        }
                        for i in 0..6 {
                            let hi = buf[i*2 + 4];
                            let lo = buf[i*2 + 5];
                            self.internal_ram.cic_checksum[i] = hi << 4 | lo;
                        }
                        self.internal_ram.os_info[0] |= 0x01; // warm boot (NMI) flag (ready in case a reset is made in the future)
                    }

                    if self.internal_ram.cic_checksum != self.internal_ram.cpu_checksum {
                        println!("PIF: invalid CIC checksum");
                        println!("   CPU checksum: {:x?}", self.internal_ram.cpu_checksum);
                        println!("   CIC checksum: {:x?}", self.internal_ram.cic_checksum);
                        // TODO: Uncomment once IPL2 checksum is correct
                        //self.state = State::Error;
                        //return next_time;
                    } else {
                        println!("PIF: CIC checksum OK");
                    }
                    self.internal_ram.cpu_checksum = [0; 6];

                    self.state = State::WaitTerminateBoot;
                    self.internal_ram.boot_timeout = time.add(6 * (250000000 / 4));  //6 seconds
                }
            }
            State::WaitTerminateBoot => {
                if io.read_command() & 0x08 != 0 {
                    io.reset_enable();
                    io.write_command(0x00);
                    self.state = State::Run;
                }
                else if self.internal_ram.boot_timeout < time {
                    println!("Boot timeout: CPU has not sent the boot termination command with 5 seconds. Halting the CPU");
                    self.state = State::Error;
                }

            }
            State::Run => {
                // do nothing
            }

            State::Error => todo!("Reset the CPu"),
        }
        if initial_state != self.state {
            println!("PIF: state {:?} -> {:?}", initial_state, self.state);
        }

        return next_time;
    }

    fn challenge(io: &mut dyn PifIO) {
        io.cic_write(1); io.cic_write(0); // challenge command
        io.cic_poll();

        io.cic_read_nibble(); // ignore timeout value returned by CIC (we simulate instant response)
        io.cic_read_nibble(); // timeout high nibble

        for address in 0..15 {
            let data = io.read(0x30 + address);
            io.cic_write_nibble(data >> 4);
            io.cic_write_nibble(data & 0xf);
        }
        io.cic_poll();

        io.cic_read(); // ignore start bit
        for address in 0..15 {
            let mut data = 0;
            data |= io.cic_read_nibble() << 4;
            data |= io.cic_read_nibble() << 0;
            io.write(0x30 + address, data);
        }
    }

    pub fn interrupt_a(&mut self, io: &mut dyn PifIO, dir: super::Dir, size: super::Size) {
        match dir {
            Dir::Read => {
                match size {
                    Size::Size64 => {
                        if io.read_command() & 0x02 != 0 {
                            Self::challenge(io);
                        }
                        else {
                            todo!("Poll joybus")
                        }
                    }
                    Size::Size4 => { }
                }
            }
            Dir::Write => {
                let cmd = io.read_command();
                if cmd & 0x01 != 0 {
                    io.write_command(cmd & !0x01);
                    todo!("configure joybus");
                }
            }
        }
    }
}
