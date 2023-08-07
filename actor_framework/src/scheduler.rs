
use crate::{object_map::{ObjectStore, ObjectStoreView}, Time, MakeNamed, actor_box::AsBase, Actor, MessagePacket, OutboxSend, Handler};

pub struct Scheduler<ActorNames> where
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::StorageType: Default,
    usize: From<ActorNames>,
{
    actors: ObjectStore<ActorNames>,
    count: u64,
    zero_limit_count: u64,
}

impl<ActorNames> Drop for Scheduler<ActorNames>
where
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::StorageType: Default,
    usize: From<ActorNames>,
{
    fn drop(&mut self) {
        eprintln!("Scheduler ran {} times", self.count);
        eprintln!(" with {} zero limits", self.zero_limit_count);
    }
}

impl<ActorNames> Scheduler<ActorNames> where
ActorNames: MakeNamed,
    usize: From<ActorNames>,
    <ActorNames as MakeNamed>::StorageType: Default + AsBase<ActorNames>,
 {
    pub fn new() -> Scheduler<ActorNames> {
        Scheduler {
            actors: ObjectStore::new(),
            count: 0,
            zero_limit_count: 0,
        }
    }

    fn find_next(&mut self) -> (ActorNames, Time, Time) {
        let mut min = Time::MAX;
        let mut min_actor = 0.into();
        let mut limit = Time::MAX;

        // PERF: I'm hoping this should compile down into SIMD optimized branchless code
        //       But that might require moving away from trait objects
        for actor_id in ActorNames::iter() {
            let actor = self.actors.get_base(actor_id);

            let time = actor.outbox.time.lower_bound();
            if time < min {
                (limit, min, min_actor) = (min, time, actor_id);
            } else {
                limit = std::cmp::min(limit, time);
            }
        }
        debug_assert!(min != Time::MAX);
        return (min_actor, min, limit)
    }

    fn run_inner<'a>(&'a mut self, sender_id: ActorNames, limit: Time) -> SchedulerResult {
        self.count += 1;

        let execute_fn = self.actors.get_base(sender_id).outbox.execute_fn;
        (execute_fn)(sender_id, &mut self.actors, limit)
    }

    #[inline(never)]
    pub fn run(&mut self) -> Box<dyn std::error::Error> {
        loop {
            let (sender_id, time, limit) = self.find_next();

            match self.run_inner(sender_id, limit) {
                SchedulerResult::Ok => {
                    // Hot path
                    continue;
                },
                SchedulerResult::ZeroLimit => {
                    // There are multiple messages scheduled to be delivered on the same cycle.
                    // And one of the receivers couldn't deal with the zero limit message, so we switch
                    // to a more complex scheduler until the current cycle finishes.
                    if let Some(exit_reason) = self.run_zero_limit(time, limit) {
                        return exit_reason;
                    }
                    continue;
                },
                SchedulerResult::Exit(reason) => {
                    return reason;
                }
            }
        }
    }

    #[inline(never)]
    pub fn run_zero_limit(&mut self, time: Time, limit: Time) -> Option<Box<dyn std::error::Error>> {
        assert!(time == limit, "Actor incorrectly reported a zero limit");

        // We might need to go though multiple iterations before this settles
        for _ in 0..(ActorNames::COUNT * 3) {
            self.zero_limit_count += 1;
            for actor in ActorNames::iter() {
                let message = self.actors.get_base(actor);

                if message.outbox.time.lower_bound() == time {
                    let result = self.run_inner(actor, time);
                    match result {
                        SchedulerResult::Ok | SchedulerResult::ZeroLimit => {},
                        SchedulerResult::Exit(reason) => {
                            return Some(reason);
                        }
                    }
                }
            }

            let (_, time, limit) = self.find_next();
            if time != limit {
                return None;
            }
        }
        panic!("Zero limit cycle detected");
    }
}

#[derive(Debug)]
pub enum SchedulerResult
{
    Ok,
    ZeroLimit,
    Exit(Box<dyn std::error::Error>)
}

pub(super) type ExecuteFn<ActorNames> = for<'a> fn(sender_id: ActorNames, map: &'a mut ObjectStore<ActorNames>, limit: Time) -> SchedulerResult;

pub(super) fn direct_execute<'a, ActorNames, Sender, Receiver, Message>(_: ActorNames, map: &'a mut ObjectStore<ActorNames>, limit: Time) -> SchedulerResult
where
    ActorNames: MakeNamed,
    Message: 'static,
    Receiver: Actor<ActorNames> + Handler<ActorNames, Message>,
    Sender: Actor<ActorNames>,
    <Sender as Actor<ActorNames>>::OutboxType: OutboxSend<ActorNames, Message>,
{
    let actor = map.get::<Sender>();
    let packet = actor.outbox.as_packet();
    let (time, message) = {

        // Safety: Type checked in MessagePacket::new
         unsafe { packet.unwrap_unchecked().take() }
    };

    let receiver = map.get::<Receiver>();
    let result = receiver.actor.recv(&mut receiver.outbox, message, time, limit);

    let actor = map.get::<Sender>();
    actor.actor.message_delivered(&mut actor.outbox, time);

    result
}

pub(super) fn endpoint_execute<'a, ActorNames, Sender, Message>(_: ActorNames, map: &'a mut ObjectStore<ActorNames>, limit: Time) -> SchedulerResult
where
    ActorNames: MakeNamed,
    Message: 'static,
    Sender: Actor<ActorNames>,
    <Sender as Actor<ActorNames>>::OutboxType: OutboxSend<ActorNames, Message>,
{
    let mut packet_view = map.get_view::<Sender>().map(
        |actor_box| {
            let packet = actor_box.outbox.as_packet();
            // Safety: Type checked in MessagePacket::from_endpoint
            unsafe { packet.unwrap_unchecked() }
        }
    );
    let (endpoint_fn, time) =
        packet_view.run(|p| (p.endpoint_fn.clone(), p.time.clone()));

    // Safety: Type checked in MessagePacket::from_endpoint
    let endpoint_fn = unsafe { endpoint_fn.assume_init() };

    let result = (endpoint_fn)(packet_view, limit);

    let actor = map.get::<Sender>();
    actor.actor.message_delivered(&mut actor.outbox, time);

    result
}

pub(super) type EndpointFn<ActorNames, Message> = for<'a> fn(
    packet_view: ObjectStoreView<'a, ActorNames, MessagePacket<ActorNames, Message>>,
    limit: Time
) -> SchedulerResult;

pub(super) fn receive_for_endpoint<ActorNames, Receiver, Message>(
    mut packet_view: ObjectStoreView<'_, ActorNames, MessagePacket<ActorNames, Message>>,
    limit: Time
) -> SchedulerResult
where
    ActorNames: MakeNamed,
    Receiver: Handler<ActorNames, Message> + Actor<ActorNames>,
{
    let (time, message) = packet_view.run(|p| unsafe { p.take() });

    let receiver = packet_view.get_obj::<Receiver>();
    receiver.actor.recv(&mut receiver.outbox, message, time, limit)
}
