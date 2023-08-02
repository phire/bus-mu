

/// This is a quick and dirty HLE implementation of the CIC SM5 core
/// I'm just wanting to get enough so I can finish booting, I'll come back to do SM5 LLE later
///
/// I'm mostly copying the implementation from Ares:
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

use super::CIC;

#[derive(Debug)]
pub struct Fifo(Vec<u8>);

impl Fifo {
    pub fn write(&mut self, val: u8) {
        self.0.push(val & 1);
    }
    pub fn read(&mut self) -> u8 {
        let val = self.0[0];
        self.0.remove(0);
        val
    }
    pub fn write_nibble(&mut self, val: u8) {
        self.write(val >> 3);
        self.write(val >> 2);
        self.write(val >> 1);
        self.write(val);
    }
    pub fn read_nibble(&mut self) -> u8 {
        self.read() << 3 | self.read() << 2 | self.read() << 1 | self.read()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

pub struct CicHle {
    state: State,
    region: Region,
    seed: u8,
    checksum: u64,
    challenge_algo: ChallengeAlgo,
    is_dd64: bool,

    pub fifo: Fifo,
}

#[derive(Debug)]
enum State {
    BootRegion, BootSeed, BootChecksum, Run, Challenge, Dead
}
enum Region { NTSC, PAL }
enum ChallengeAlgo { DummyChallenge, RealChallenge }

fn scramble(buffer: &mut [u8])
{
    for i in 1..buffer.len() {
        buffer[i] = (buffer[i] + buffer[i - 1] + 1) & 0xf;
    }
}

impl CicHle {
    pub fn new(cic_model: CIC) -> Self {
        let (region, seed, checksum) =  match cic_model {
            CIC::Nus6101 => (Region::NTSC, 0x3d, 0x45cc73ee317a),
            CIC::Nus6102 => (Region::NTSC, 0x3d, 0xa536c0f1d859),
            CIC::Nus6103 => (Region::NTSC, 0x78, 0x586fd4709867),
            CIC::Nus6105 => (Region::NTSC, 0x91, 0x8618a45bc2d3),
            CIC::Nus6106 => (Region::NTSC, 0x85, 0x2bbad4e6eb74),
            CIC::Nus7101 => (Region::PAL,  0x3d, 0xa536c0f1d859),
            CIC::Nus7102 => (Region::PAL,  0x3d, 0x44160ec5d9af),
            CIC::Nus7103 => (Region::PAL,  0x78, 0x586fd4709867),
            CIC::Nus7105 => (Region::PAL,  0x91, 0x8618a45bc2d3),
            CIC::Nus7106 => (Region::PAL,  0x85, 0x2bbad4e6eb74),
            CIC::Nus8303 => (Region::NTSC, 0xdd, 0x32b294e2ab90),
            CIC::Nus8401 => (Region::NTSC, 0xdd, 0x6ee8d9e84970),
            CIC::Nus5167 => (Region::NTSC, 0xdd, 0x083c6c77e0b1),
            CIC::NusDDUS => (Region::NTSC, 0xde, 0x05ba2ef0a5f1),
        };

        let challenge_algo = match cic_model {
            CIC::Nus6105 | CIC::Nus7105 => ChallengeAlgo::RealChallenge,
            _ => ChallengeAlgo::DummyChallenge
        };

        let is_dd64 = match cic_model {
            CIC::Nus8303 | CIC::Nus8401 | CIC::NusDDUS => true,
            _ => false
        };

        CicHle {
            state: State::BootRegion,
            region,
            seed,
            checksum,
            challenge_algo,
            is_dd64,

            fifo: Fifo(Vec::new()),
        }
    }

    pub fn poll(&mut self) {
        println!("CIC HLE: {:?}", self.state);
        match self.state {
            State::BootRegion => {
                self.fifo.write(self.is_dd64 as u8);
                self.fifo.write(match self.region { Region::NTSC => 0, Region::PAL => 1 });
                self.fifo.write(0);
                self.fifo.write(1);
                self.state = State::BootSeed;
            }
            State::BootSeed => {
                let mut buf = [
                    0xb,
                    0x5,
                    self.seed >> 4,
                    self.seed & 0xf,
                    self.seed >> 4,
                    self.seed & 0xf,
                ];
                for _ in 0..2 {
                    scramble(&mut buf)
                }
                for b in buf.iter() {
                    self.fifo.write_nibble(*b);
                }
                self.state = State::BootChecksum;
            }
            State::BootChecksum => {
                let mut buf = [0; 16];
                buf[0] = 0x4; // true random
                buf[1] = 0x7; // true random
                buf[2] = 0xa; // true random
                buf[3] = 0x1; // true random

                for i in 0..12 {
                    buf[i + 4] = ((self.checksum >> (44-4*i)) as u8) & 0xf ;
                }
                for _ in 0..4 {
                    scramble(&mut buf)
                }
                for b in buf.iter() {
                    self.fifo.write_nibble(*b);
                }
                self.state = State::Run;
            }
            State::Run if (self.fifo.len() >= 2) => {
                let cmd = self.fifo.read() << 1 | self.fifo.read();
                match cmd {
                    0b00 => self.cmd_compare(),
                    0b01 => self.cmd_die(),
                    0b10 => self.cmd_challenge(),
                    0b11 => self.cmd_reset(),
                    _ => unreachable!()
                }
            }
            State::Challenge => {
                self.cmd_challenge();
            }
            _ => {}
        }
        println!("  now: {:?}", self.state);
        println!("  fifo: {:?}", self.fifo);
    }

    fn challenge(&self, data: [u8; 30]) -> [u8; 30] {
        match self.challenge_algo {
            ChallengeAlgo::DummyChallenge => {
                return data.map(|n| !n);
            }
            ChallengeAlgo::RealChallenge => {
                // CIC-NUS-6105 anti-piracy challenge
                let lut = [
                    0x4, 0x7, 0xa, 0x7, 0xe, 0x5, 0xe, 0x1,
                    0xc, 0xf, 0x8, 0xf, 0x6, 0x3, 0x6, 0x9,
                    0x4, 0x1, 0xa, 0x7, 0xe, 0x5, 0xe, 0x1,
                    0xc, 0x9, 0x8, 0x5, 0x6, 0x3, 0xc, 0x9,
                ];

                let mut key = 0xb;
                let mut sel = 0;
                let mut mem = data;

                for address in 0..30 {
                    let data = (key + 5 * mem[address]) & 0xf;
                    mem[address] = data;

                    key = lut[(sel << 4 | data) as usize];
                    let mut modifier = key >> 3 != 0;
                    let mut mag = key & 7;
                    if modifier { mag = !mag & 0x3 }
                    if mag % 3 != 1 { modifier = !modifier }
                    sel = match data {
                        0x1 | 0x9 if sel == 1 => 1,
                        0xb | 0xe if sel == 1 => 0,
                        _ => modifier as u8,
                    }
                }

                return mem;
            }
        }
    }

    fn cmd_compare(&mut self) {
        // ares doesn't implement this?
    }

    fn cmd_challenge(&mut self) {
        match self.state {
            State::Run => {
                self.fifo.write_nibble(0xa);
                self.fifo.write_nibble(0xa);
                self.state = State::Challenge;
            }
            _ => {}
        }
        if self.fifo.len() == 30*4 {
            let data: [u8; 30] = std::array::from_fn(|_| self.fifo.read_nibble());
            for n in self.challenge(data) {
                self.fifo.write_nibble(n);
            }
            self.state = State::Run;
        }
    }
    fn cmd_die(&mut self) {
        println!("CIC: die");
        self.state = State::Dead;
    }
    fn cmd_reset(&mut self) {
        unimplemented!("CIC: reset")
    }

}
