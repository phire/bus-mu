use std::collections::BinaryHeap;

use actor_framework::*;
use super::N64Actors;

/// This actor represents RCP's internal bus and handles all bus arbitration
/// For now, we do it all synchronously, so this is going to be a huge bottleneck
pub struct BusActor {
    queue: BinaryHeap<BusRequest>,
    commited_time: Time,
}

impl Default for BusActor {
    fn default() -> Self {
        Self {
            queue: BinaryHeap::new(),
            commited_time: Time::default(),
        }
    }
}

/// Message for when a bus-request was accepted
pub struct BusAccept {}

pub struct BusRequest {
    piority: u32,
    count: u32,
    channel: Channel<BusAccept, N64Actors>,
}

const fn piority(actor: N64Actors) -> u32 {
    match actor {
        N64Actors::SiActor => 50, // SI has a high priority because it has no buffer and no way to pause serial transfers
        N64Actors::CpuActor => 0,
        N64Actors::BusActor | N64Actors::PifActor => 0, // shouldn't happen
    }
}

impl BusRequest {
    /// Limitations: There can only be one outstanding bus request per actor
    pub fn new<A>(count: u32) -> Self where A: Actor<N64Actors> + Handler<BusAccept, N64Actors> + 'static {
        let addr = Addr::<A, N64Actors>::default();
        Self {
            piority: piority(A::name()),
            count,
            channel: addr.make_channel::<BusAccept>(),
        }
    }
}

impl PartialEq for BusRequest {
    fn eq(&self, other: &Self) -> bool {
        self.piority == other.piority
    }
}

impl Eq for BusRequest {}

impl PartialOrd for BusRequest {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.piority.partial_cmp(&other.piority)
    }
}

impl Ord for BusRequest {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.piority.cmp(&other.piority)
    }
}

impl Handler<BusRequest, N64Actors> for BusActor {
    fn recv(&mut self, time: Time, message: BusRequest) -> MessagePacket<N64Actors> {
        if self.queue.is_empty() {
            self.commited_time = time;
        }
        self.queue.push(message);
        return MessagePacket::no_message(self.commited_time.add(1));
    }
}


impl Actor<N64Actors> for BusActor {
    fn advance(&mut self, limit: Time) -> MessagePacket<N64Actors> {
        debug_assert!(!self.queue.is_empty());
        let request = self.queue.pop().unwrap();
        let time = self.commited_time.add(1);

        self.commited_time = time.add(request.count.into());
        return request.channel.send(BusAccept {}, time);
    }

    fn advance_to(&mut self, target: Time) {
        debug_assert!(target >= self.commited_time);
    }

    fn horizon(&mut self) -> Time {
        return match self.queue.is_empty() {
            true => Time::max(),
            false => self.commited_time,
        }
    }
}
