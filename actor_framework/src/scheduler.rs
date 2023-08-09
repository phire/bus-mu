
use crate::{object_map::{ObjectStore, ObjectStoreView}, Time, MakeNamed, Actor, MessagePacket, OutboxSend, Handler, Outbox, EnumMap};

pub struct Scheduler<ActorNames> where
    ActorNames: MakeNamed,
{
    actors: ObjectStore<ActorNames>,
    queue: EnumMap<QueueEntry<ActorNames>, ActorNames>,
    queue_head: Option<ActorNames>,
    count: u64,
    zero_limit_count: u64,
}

impl<ActorNames> Drop for Scheduler<ActorNames>
where
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::StorageType: Default,
{
    fn drop(&mut self) {
        eprintln!("Scheduler ran {} times", self.count);
        eprintln!(" with {} zero limits", self.zero_limit_count);
    }
}

impl<ActorNames> Scheduler<ActorNames> where
    ActorNames: MakeNamed,
 {

    pub fn new() -> Scheduler<ActorNames> {
        let mut scheduler = Scheduler {
            actors: ObjectStore::new(),
            queue: EnumMap::from_fn(|_| QueueEntry { next: None, prev: None }),
            queue_head: None,
            count: 0,
            zero_limit_count: 0,
        };

        // Calculate initial priority queue
        for id in ActorNames::iter() {
            if scheduler.get_time(id) != Time::MAX {
                scheduler.queue_add(id);
            }
        }
        assert!(scheduler.queue_head.is_some(), "No schedulable actors found");

        scheduler
    }


    fn find_next(&mut self) -> (ActorNames, Time, Time) {
        let mut min = Time::MAX;
        let mut min_actor = 0.into();
        let mut limit = Time::MAX;

        // PERF: I'm hoping this should compile down into SIMD optimized branchless code
        //       but currently it compiles down to a chain of conditional-moves
        for actor_id in ActorNames::iter() {
            let time = self.get_time(actor_id).lower_bound();
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
        (execute_fn)(sender_id, self, limit)
    }

    #[cfg(feature = "branchless")]
    fn take_next(&mut self) -> (ActorNames, Time, Time) {
        self.find_next()
    }

    #[cfg(feature = "linked_list")]
    fn take_next(&mut self) -> (ActorNames, Time, Time) {
        self.queue_pop()
    }

    #[inline(never)]
    pub fn run(&mut self) -> Box<dyn std::error::Error> {
        loop {
            let (sender_id, time, limit) = self.take_next();

            // println!("Running actor {:?}. Next is {:?} @ {}", sender_id, next, limit);

            match self.run_inner(sender_id, limit) {
                SchedulerResult::Ok => {
                    // Hot path
                    continue;
                },
                SchedulerResult::ZeroLimit if cfg!(feature = "branchless") => {
                    // There are multiple messages scheduled to be delivered on the same cycle.
                    // And one of the receivers couldn't deal with the zero limit message, so we switch
                    // to a more complex scheduler until the current cycle finishes.
                    self.zero_limit_count += 1;
                    if let Some(exit_reason) = self.run_zero_limit(time, limit) {
                        return exit_reason;
                    }
                },
                SchedulerResult::ZeroLimit => {
                    self.zero_limit_count += 1;
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

struct QueueEntry<ActorNames> where
    ActorNames: MakeNamed,
{
    next: Option<ActorNames>,
    prev: Option<ActorNames>,
}

impl<ActorNames> Scheduler<ActorNames> where
    ActorNames: MakeNamed,
{
    fn queue_pop(&mut self) -> (ActorNames, Time, Time) {
        let sender_id = self.queue_head.take().expect("No schedulable actors found");
        let next = {
            let sender = &mut self.queue[sender_id];
            debug_assert!(sender.prev.is_none(),
                "{:?}'s prev should be None, but is {:?}", sender_id, sender.prev);
            sender.next.take()
        };

        let limit = match next {
            Some(next_id) => {
                debug_assert!(next_id != sender_id);
                self.queue_head = Some(next_id);
                self.queue[next_id].prev = None;
                self.get_time(next_id).lower_bound()
            },
            None => Time::MAX,
        };
        return (sender_id, self.get_time(sender_id), limit);
    }

    #[inline(never)]
    fn queue_add(&mut self, id: ActorNames) {
        let time = self.get_time(id).lower_bound();

        let mut next;
        let mut prev_id;

        match self.queue_head {
            None => {
                self.queue_head = Some(id);
                self.queue[id].prev = None;
                self.queue[id].next = None;
                return;
            },
            Some(next_id) => {
                if self.get_time(next_id).lower_bound() > time {
                    self.queue_head = Some(id);
                    self.queue[id].prev = None;
                    self.queue[id].next = Some(next_id);
                    self.queue[next_id].prev = Some(id);
                    return;
                } else {
                    next = self.queue[next_id].next;
                    prev_id = next_id;
                }
            }
        }

        loop {
            match next {
                None => {
                    self.queue[id].prev = Some(prev_id);
                    self.queue[prev_id].next = Some(id);
                    return;
                }
                Some(next_id) => {
                    let next_time = self.get_time(next_id).lower_bound();
                    let next_actor = &mut self.queue[next_id];
                    if next_time <= time {
                        next = next_actor.next;
                        prev_id = next_id;
                    } else {
                        debug_assert!(next_actor.prev == Some(prev_id), "{:?} != {:?}", next_actor.prev, prev_id);
                        next_actor.prev = Some(id);
                        self.queue[prev_id].next = Some(id);
                        self.queue[id].next = Some(next_id);
                        self.queue[id].prev = Some(prev_id);
                        return;
                    }
                }
            }
        }
    }

    fn queue_remove(&mut self, actor_id: ActorNames) {
        let mut entry = QueueEntry {
            next: None,
            prev: None,
        };
        core::mem::swap(&mut self.queue[actor_id], &mut entry);

        match entry.prev {
            None => self.queue_head = entry.next,
            Some(prev_id) => self.queue[prev_id].next = entry.next,
        }
        match entry.next {
            None => (),
            Some(next_id) => self.queue[next_id].prev = entry.prev,
        }
    }

    fn queue_print(&mut self) {
        let mut next = self.queue_head;
        println!("Queue: (Head = {:?})", next);
        while let Some(next_id) = next {
            println!("    {:?} @ {} ({:?}, {:?})", next_id,
                self.get_time(next_id).lower_bound(),
                self.queue[next_id].prev, self.queue[next_id].next
            );
            next = self.queue[next_id].next;
        }
    }

    fn queue_validate(&mut self) {
        for id in ActorNames::iter() {
            let time = self.get_time(id).lower_bound();
            let entry = &mut self.queue[id];
            let prev = entry.prev;
            if let Some(next_id) = entry.next {
                if self.queue[next_id].prev != Some(id) {
                    self.queue_print();
                    panic!("actor {:?}'s next actor {:?} prev points to {:?} instead of {:?}", id, next_id, self.queue[next_id].prev, Some(id));
                } else if self.get_time(next_id) < time {
                    self.queue_print();
                    panic!("actor {:?}'s next actor {:?} has a lower time bound ({}) than {:?} ({})", id, next_id, self.get_time(next_id), id, time);
                }
            }
            if let Some(prev_id) = prev {
                if self.queue[prev_id].next != Some(id) {
                    self.queue_print();
                    panic!("actor {:?}'s prev actor {:?} next points to {:?} instead of {:?}", id, prev_id, self.queue[prev_id].next, Some(id));
                } else if self.get_time(prev_id) > time {
                    self.queue_print();
                    panic!("actor {:?}'s prev actor {:?} has a higher time bound ({}) than {:?} ({})", id, prev_id, self.get_time(prev_id), id, time);
                }
            }
        }
    }

    fn get_time(&mut self, id: ActorNames) -> Time {
        self.actors.get_base(id).outbox.time
    }

    fn take_message<Sender, Message>(&mut self) -> Option<(Time, Message)>
    where
        Message: 'static, // for TypeId
        Sender: Actor<ActorNames>,
        <Sender as Actor<ActorNames>>::OutboxType: OutboxSend<ActorNames, Message>,
    {
        let actor = self.actors.get::<Sender>();
        actor.outbox.as_packet().and_then(|p| { p.take() })
    }

    fn call_receiver<Receiver, Message>(&mut self, sender_id: ActorNames, msg: Message, time: Time, limit: Time) -> SchedulerResult
    where
        Receiver: Actor<ActorNames> + Handler<ActorNames, Message>,
    {
        let actor = self.actors.get::<Receiver>();
        let before = actor.outbox.time();
        let result = actor.obj.recv(&mut actor.outbox, msg, time, limit);
        let after = actor.outbox.time();

        if cfg!(feature = "linked_list") && before != after {
            if sender_id != Receiver::name() && before != Time::MAX {
                // Receiver already had a message queued
                self.queue_remove(Receiver::name());
            }
            if after != Time::MAX {
                // Receiver has a message queued
                self.queue_add(Receiver::name());
            }
        }

        result
    }

    fn message_delivered<Sender>(&mut self, time: Time, _limit: Time)
    where
        Sender: Actor<ActorNames>,
    {
        let actor = self.actors.get::<Sender>();

        // PERF: When Sender != Receiver, before will always be Time::MAX.
        //       We should make sure the compiler optimizes this case
        let before = actor.outbox.time();
        actor.obj.message_delivered(&mut actor.outbox, time);
        let after = actor.outbox.time();

        if cfg!(feature = "linked_list") && before != after && after != Time::MAX {
            // The Sender send another message, add the Sender back to the queue
            self.queue_add(Sender::name());
        }
    }
}

#[derive(Debug)]
pub enum SchedulerResult
{
    Ok,
    ZeroLimit,
    Exit(Box<dyn std::error::Error>)
}

pub(super) type ExecuteFn<ActorNames> = for<'a> fn(sender_id: ActorNames, scheduler: &'a mut Scheduler<ActorNames>, limit: Time) -> SchedulerResult;

pub(super) fn direct_execute<'a, ActorNames, Sender, Receiver, Message>(_: ActorNames, scheduler: &'a mut Scheduler<ActorNames>, limit: Time) -> SchedulerResult
where
    ActorNames: MakeNamed,
    Message: 'static,
    Receiver: Actor<ActorNames> + Handler<ActorNames, Message>,
    Sender: Actor<ActorNames>,
    <Sender as Actor<ActorNames>>::OutboxType: OutboxSend<ActorNames, Message>,
{
    // Safety: Type checked in MessagePacket::new
    let (time, message) = unsafe {
        scheduler.take_message::<Sender, Message>().unwrap_unchecked()
    };
    let result = scheduler.call_receiver::<Receiver, _>(Sender::name(), message, time, limit);
    scheduler.message_delivered::<Sender>(time, limit);

    result
}

pub(super) fn endpoint_execute<'a, ActorNames, Sender, Message>(_: ActorNames, scheduler: &'a mut Scheduler<ActorNames>, limit: Time) -> SchedulerResult
where
    ActorNames: MakeNamed,
    Message: 'static,
    Sender: Actor<ActorNames>,
    <Sender as Actor<ActorNames>>::OutboxType: OutboxSend<ActorNames, Message>,
{
    let mut packet_view = scheduler.actors.get_view::<Sender>().map(
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
    scheduler.message_delivered::<Sender>(time, limit);

    result
}

pub(super) fn null_execute<ActorNames>(_: ActorNames, _: &mut Scheduler<ActorNames>, _: Time) -> SchedulerResult
where
    ActorNames: MakeNamed,
{
    panic!("Scheduler tried to execute an empty message");
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
    let option = packet_view.run(|p| p.take() );
    // Safety: Type checked in Endpoint::new + MessagePacket::from_endpoint
    let (time, message) = unsafe { option.unwrap_unchecked() };

    let receiver = packet_view.close().get::<Receiver>();
    let _result = receiver.obj.recv(&mut receiver.outbox, message, time, limit);

    todo!("Reschedule receiver if wrote to outbox")
}
