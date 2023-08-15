use std::{collections::BinaryHeap, any::TypeId};

use actor_framework::*;
use crate::{c_bus::CBus, d_bus::DBus};

use super::{N64Actors, cpu_actor::CpuActor};

/// This actor represents RCP's internal bus and handles all bus arbitration
/// For now, we do it all synchronously, so this is going to be a huge bottleneck
pub struct BusActor {
    queue: BinaryHeap<BusRequest>,
    commited_time: Time,
    /// A channel to the current Bus owner
    bus_owner: Option<ReturnBusChannel>,
}

impl Default for BusActor {
    fn default() -> Self {
        Self {
            queue: BinaryHeap::new(),
            commited_time: Time::default(),
            // To simplify things, CpuActor starts with the bus resource
            // TODO: Allow actors to pass the bus between each other.
            //       We might need to make resource sharing a native feature of actor_framework and
            //       have the scheduler manage resources. Maybe resources should be full sub-actors
            //       that can be called without going though the scheduler?
            bus_owner: Some(Channel::new::<CpuActor>()),
        }
    }
}

make_outbox!(
    BusOutbox<N64Actors, BusActor> {
        grant_c_bus: Box<BusPair>,
        return_c_bus: ReturnBus,
    }
);


/// Return the borrowed CBus
pub struct ReturnBus {}

pub struct BusPair {
    pub c_bus: CBus,
    pub d_bus: DBus,
}

type GrantBusChannel = Channel<N64Actors, BusActor, Box<BusPair>>;
type ReturnBusChannel = Channel<N64Actors, BusActor, ReturnBus>;

pub struct BusRequest {
    channels: (GrantBusChannel, ReturnBusChannel),
    piority: u16,
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
        N64Actors::BusActor | N64Actors::PifActor | N64Actors::Terminal => { // shouldn't happen
            debug_assert!(false);
            0
        }
    }
}

/// Limitations: There can only be one outstanding bus request per actor
pub fn request_bus<Requestor, Out>(outbox: &mut Out, time: Time) -> SchedulerResult
where
Out: Outbox<N64Actors, Sender = Requestor>,
Out: OutboxSend<N64Actors, BusRequest>,
Requestor: Handler<N64Actors, Box<BusPair>> + Actor<N64Actors> + Handler<N64Actors, ReturnBus>,
{
    let bus_request = BusRequest {
        channels: (Channel::new::<Requestor>(), Channel::new::<Requestor>()),
        piority: piority(Requestor::name()),
    };
    outbox.send::<BusActor>(bus_request, time)
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
    #[inline(always)]
    fn recv(&mut self, outbox: &mut BusOutbox, message: BusRequest, time: Time, _limit: Time) -> SchedulerResult {
        let new_piority = message.piority;
        self.queue.push(message);

        if let Some(bus_owner) = self.bus_owner.take() {
            // Request the bus from the current owner
            outbox.send_channel(bus_owner, ReturnBus {}, time);
        } else if outbox.contains::<BusPair>() && self.commited_time == time {
            // We already accepted a request this cycle, but we might need to change our mind and
            // accept this one if it has a higher piority
            let highest = self.queue.peek().unwrap();

            // Note: All priorities should be unique per sender, and they are only allowed one
            //       outstanding request. So we can just compare priorities
            // TODO: PERF: Should we modify BinaryHeap to tell us if the top element changed?
            if highest.piority == new_piority {
                // This request takes priority, cancel the previous grant
                let (_, bus) = outbox.cancel();
                let (grant, _) = highest.channels.clone();

                outbox.send_channel(grant, bus, time.add(1));
            }
        }
        SchedulerResult::Ok
    }
}

impl Handler<N64Actors, Box<BusPair>> for BusActor {
    #[inline(always)]
    fn recv(&mut self, outbox: &mut BusOutbox, bus: Box<BusPair>, time: Time, _limit: Time) -> SchedulerResult {
        let highest = self.queue.peek().expect("There should be a request in the queue");
        self.commited_time = time;

        let (grant, _) = highest.channels.clone();
        outbox.send_channel(grant, bus, time.add(1))
    }
}

impl Actor<N64Actors> for BusActor {
    type OutboxType = BusOutbox;

    #[inline(always)]
    fn delivering<Message>(&mut self, outbox: &mut BusOutbox, _: &Message, time: Time)
    where Message: 'static
    {
        if TypeId::of::<Message>() == TypeId::of::<Box<BusPair>>() {
            let request = self.queue.pop().unwrap();

            // Increment committed time
            self.commited_time = self.commited_time.add(1);
            let (_, return_channel) = request.channels;

            if self.queue.is_empty() {
                self.bus_owner = Some(return_channel);
            } else {
                // There is another request waiting, immediately ask for the bus back
                outbox.send_channel(return_channel, ReturnBus {}, time.add(1));
            }
        }
    }
}
