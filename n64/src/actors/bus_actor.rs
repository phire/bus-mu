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

make_outbox!(
    BusOutbox<N64Actors, BusActor> {
        bus_accept: BusAccept
    }
);

/// Message for when a bus-request was accepted
pub struct BusAccept {}

pub struct BusRequest {
    channel: Endpoint<N64Actors, BusAccept>,
    piority: u16,
    count: u16,
}

const fn piority(actor: N64Actors) -> u16 {
    match actor {
        // All priorities should be unique
        N64Actors::SiActor => 50, // SI has a high priority because it has no buffer and no way to pause serial transfers
        N64Actors::AiActor => 45, // Guess, needs to be reasonably high, buffer is pretty small
        N64Actors::ViActor => 40, // Guess, needs to be reasonably high, buffer is pretty small
        N64Actors::RdpActor => 5, // Guess
        N64Actors::RspActor => 3, // Guess
        N64Actors::PiActor => 2,
        N64Actors::CpuActor => 1,
        N64Actors::BusActor | N64Actors::PifActor => {debug_assert!(false); 0}, // shouldn't happen
    }
}

impl BusRequest {
    /// Limitations: There can only be one outstanding bus request per actor
    pub fn new<A>(count: u16) -> Self
    where
        A: Handler<N64Actors, BusAccept> + Actor<N64Actors>
    {
        let addr = Addr::<A, N64Actors>::default();
        Self {
            channel: addr.make_channel::<BusAccept>(),
            piority: piority(A::name()),
            count,
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
        Some(self.cmp(other))
    }
}

impl Ord for BusRequest {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.piority.cmp(&other.piority)
    }
}

impl Handler<N64Actors, BusRequest> for BusActor {
    fn recv(&mut self, outbox: &mut BusOutbox, message: BusRequest, time: Time, _limit: Time) -> SchedulerResult {
        if self.queue.is_empty() && self.commited_time < time {
            // There are no outstanding requests, so we can just accept this one
            self.commited_time = time;
            outbox.send_endpoint(message.channel.clone(), BusAccept { }, time.add(1));
            self.queue.push(message);
        } else {
            let new_piority = message.piority;
            self.queue.push(message);

            if self.commited_time == time {
                // We already accepted a request this cycle, but we might need to change our mind and
                // accept this one if it has a higher piority
                let highest = self.queue.peek().unwrap();

                // Note: All priorities should be unique per sender, and they are only allowed one outstanding request
                //       So we can just compare priorities
                if highest.piority == new_piority {
                    let channel = highest.channel.clone();
                    // This request takes priority
                    outbox.send_endpoint(channel, BusAccept { }, time.add(1));
                }
            }
        }
        SchedulerResult::Ok
    }
}

impl Actor<N64Actors> for BusActor {
    type OutboxType = BusOutbox;

    fn message_delivered(&mut self, outbox: &mut BusOutbox, _time: Time) {
        let request = self.queue.pop().unwrap();
        // Increment time to end of delivered request
        self.commited_time = self.commited_time.add(request.count.into());
        drop(request);

        // Process next request
        if let Some(request) = self.queue.peek() {
            let channel = request.channel.clone();
            let accept_time = self.commited_time.add(1);

            outbox.send_endpoint(channel, BusAccept { }, accept_time);
        }
    }
}
