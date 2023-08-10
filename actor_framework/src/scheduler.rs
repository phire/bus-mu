
use std::usize;

use crate::{object_map::{ObjectStore, ObjectStoreView}, Time, MakeNamed, Actor, MessagePacket, OutboxSend, Handler, Outbox, EnumMap};

// PERF: TODO:
// This is currently a bit of a mess. It implements four different scheduling algorithms.
//
//  - Branchless uses a chain of conditional moves to select the next actor without any branches
//    Problem is that it's complexity grows with total actors, even if the actors do nothing
//  - linked-list uses a double-linked list
//  - cached combines the two above two, using branchless to select between a fixed set of
//    active actors and falling back to the linked list
//  - updatding_cache is cached, except it continually updates the cache as actors are executed
//    so the cache always returns a valid result
//
//  Unfortunately, I had to abandon optimization efforts because my n64 implementation didn't
//  generate complex enough workloads. I'll need to come back once I have multiple cores all
//  access main memory and causing bus conflicts.
//
//  When only the CPU is running:
//   - linked list is the fastest, as messages get inserted at the front of the queue 99.9% of the time
//   - cached with CACHE_SIZE = 2 is slightly slower
//   - updating_cache is slightly slower again
//   - branchless is the slowest, especially as the number of actors gets larger
//
//  I'm expecting that cached might be faster on more complex workloads... though need to test
//
//  Other optimization ideas:
//     Don't access time (and execute_fn?) via indirect loads to the outbox.
//     Instead, we should actually copy those out of the outbox and into the linked list queue or cache
//     Should make both find_next_cached and queue_add faster, and allow us to remove some stupid unsafe code

const CACHE_SIZE: usize = 2;
const UNCACHED: u8 = CACHE_SIZE as u8;

pub struct Scheduler<ActorNames> where
    ActorNames: MakeNamed,
{
    actors: ObjectStore<ActorNames>,
    queue: EnumMap<QueueEntry<ActorNames>, ActorNames>,
    queue_head: Option<ActorNames>,
    is_cached: EnumMap<u8, ActorNames>,
    cached: [ActorNames; CACHE_SIZE],
    cache_limit: Time,
    num_cache_entries: u8,
    count: u64,
    count_cache_inserts: u64,
    count_queue_adds: u64,
    count_queue_removes: u64,
    count_queue_add_complexity: u64,
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
        eprintln!(" {} cache inserts", self.count_cache_inserts);
        let complexity = self.count_queue_add_complexity as f64 / self.count_queue_adds as f64;
        eprintln!(" {} queue adds, complexity {}", self.count_queue_adds, complexity);
        eprintln!(" {} queue removes", self.count_queue_removes);
    }
}

impl<ActorNames> Scheduler<ActorNames> where
    ActorNames: MakeNamed,
 {
    const EMPTY_CACHE: ActorNames = ActorNames::TERMINAL;

    pub fn new() -> Scheduler<ActorNames> {
        let mut scheduler = Scheduler {
            actors: ObjectStore::new(),
            queue: EnumMap::from_fn(|_| QueueEntry { next: None, prev: None }),
            queue_head: None,
            is_cached: EnumMap::from_fn(|_| UNCACHED),
            cached: [Self::EMPTY_CACHE; CACHE_SIZE],
            cache_limit: Time::MAX,
            num_cache_entries: 0,
            count: 0,
            count_cache_inserts: 0,
            count_queue_adds: 0,
            count_queue_removes: 0,
            count_queue_add_complexity: 0,
            zero_limit_count: 0,
        };

        assert!(CACHE_SIZE < core::cmp::max(ActorNames::COUNT, 254));

        // Calculate initial priority queue
        for id in ActorNames::iter() {
            let time = scheduler.get_time(id);
            if time != Time::MAX {
                scheduler.queue_add(id, time);
            }
        }
        assert!(scheduler.queue_head.is_some(), "No schedulable actors found");

        if cfg!(feature = "cached") {
            scheduler.cached = std::array::from_fn(|idx| {
                let (id, _, limit) = scheduler.queue_pop();
                if let Some(id) = id {
                    scheduler.cache_limit = limit;
                    scheduler.is_cached[id] = idx as u8;
                    scheduler.num_cache_entries += 1;
                    id
                } else {
                    Self::EMPTY_CACHE
                }
            });
        }

        scheduler
    }

    #[cfg(feature = "branchless")]
    fn take_next(&mut self) -> (ActorNames, Time, Time) {
        let mut min = Time::MAX;
        let mut min_actor = 0.into();
        let mut limit = Time::MAX;

        // This compiles down to a chain of conditional-moves
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

    #[allow(dead_code)]
    fn find_next_cached(&mut self) -> (Option<ActorNames>, Time, Time) {
        let mut min = self.cache_limit;
        let mut min_actor: Option<ActorNames> = None;
        let mut limit = Time::MAX;

        // This compiles down to a chain of conditional-moves
        for actor_id in self.cached {
            let time = self.get_time(actor_id).lower_bound();
            if time < min {
                (limit, min, min_actor) = (min, time, Some(actor_id));
            } else {
                limit = std::cmp::min(limit, time);
            }
        }
        return (min_actor, min, limit)
    }

    fn cache_remove(&mut self, id: ActorNames, time: Time) -> usize {
        debug_assert!(self.is_cached[id] != UNCACHED, "Trying to uncache Actor {:?} that's not cached", id);
        debug_assert!(time == self.get_time(id), "Time mismatch");
        let cache_slot = self.is_cached[id] as usize;
        self.cached[cache_slot] = Self::EMPTY_CACHE;
        self.is_cached[id] = UNCACHED;
        self.num_cache_entries -= 1;
        if time != Time::MAX {
            self.queue_add(id, time);
        }
        cache_slot
    }

    fn cache_replace(&mut self, slot: usize, id: ActorNames) {
        match self.cached[slot] {
            id if id == Self::EMPTY_CACHE => { },
            replaced_id => {
                let time = self.get_time(replaced_id);
                debug_assert!(time == Time::MAX, "Trying to replace cached Actor that's scheduled");
                self.is_cached[replaced_id] = UNCACHED;
                self.num_cache_entries -= 1;
            }
        }
        self.cached[slot] = id;
        self.is_cached[id] = slot as u8;
        self.num_cache_entries += 1;
    }

    #[inline(never)]
    fn cache_insert(&mut self, insert_id: ActorNames) {
        if self.num_cache_entries as usize == CACHE_SIZE {
            // cache is full, replace the entry with the highest time
            self.cached.clone().iter()
                .enumerate()
                .max_by(|&(_, &a), &(_, &b)| {
                    self.get_time(a).cmp(&self.get_time(b))
                }
                )
                .and_then(|(slot, &max_id)| {
                    self.is_cached[max_id] = UNCACHED;
                    self.num_cache_entries -= 1;
                    match self.get_time(max_id) {
                        Time::MAX => { }
                        time => self.queue_add(max_id, time)
                    }
                    Some(slot)
                })
        } else {
            // There is at least one empty slot
            self.cached.iter()
                .enumerate()
                .find_map(|(slot, id)| {
                    if *id == Self::EMPTY_CACHE { Some(slot) } else { None }
                })
        }.map(|slot| {
            self.cached[slot] = insert_id;
            self.is_cached[insert_id] = slot as u8;
            self.num_cache_entries += 1;
            self.count_cache_inserts += 1;
        })
        .unwrap();
    }

    fn run_inner<'a>(&'a mut self, sender_id: ActorNames, limit: Time) -> SchedulerResult {
        self.count += 1;

        let execute_fn = self.actors.get_base(sender_id).outbox.execute_fn;
        (execute_fn)(sender_id, self, limit)
    }

    #[cfg(feature = "cached")]
    //#[inline(never)]
    fn take_next(&mut self) -> (ActorNames, Time, Time) {
        let (next, time, limit) = self.find_next_cached();
        if cfg!(feature = "updating_cache") {
            // The "updating_cache" fully updates the cache in `fn execute_message`
            // to make it always return a valid result
            (next.unwrap(), time, limit)
        } else if next.is_some() && time != Time::MAX {
            (next.unwrap(), time, limit)
        } else {
            let (next, time, limit) = self.queue_pop();
            self.cache_insert(next.unwrap());
            (next.unwrap(), time, limit)
        }
    }

    #[cfg(all(feature = "linked_list", not(feature = "cached")))]
    fn take_next(&mut self) -> (ActorNames, Time, Time) {
        let (next, time, limit) = self.queue_pop();
        assert!(next.is_some() && time != Time::MAX, "next: {:?}, time: {:?}", next, time);
        (next.unwrap(), time, limit)
    }

    #[inline(never)]
    pub fn run(&mut self) -> Box<dyn std::error::Error> {
        loop {
            let (sender_id, time, limit) = self.take_next();
            //println!("Running actor {:?}. Next @ {}", sender_id, limit);

            match self.run_inner(sender_id, limit) {
                SchedulerResult::Ok => {
                    // Hot path
                    continue;
                },
                SchedulerResult::ZeroLimit if cfg!(any(feature = "branchless", feature = "cached")) => {
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

            let (_, time, limit) = self.take_next();
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
    fn queue_pop(&mut self) -> (Option<ActorNames>, Time, Time) {
        let sender_id = match self.queue_head.take() {
            Some(id) => id,
            None => {
                return (None, Time::MAX, Time::MAX);
            }
        };

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
        return (Some(sender_id), self.get_time(sender_id), limit);
    }

    #[inline(never)]
    fn queue_add(&mut self, id: ActorNames, time: Time) {
        debug_assert!(time == self.get_time(id), "Time mismatch");
        self.count_queue_adds += 1;
        self.count_queue_add_complexity += 1;

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
            self.count_queue_add_complexity += 1;
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
        self.count_queue_removes += 1;
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    fn get_time(&self, id: ActorNames) -> Time {
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

    fn execute_message<Sender, Receiver, Message>(&mut self, msg: Message, time: Time, limit: Time) -> SchedulerResult
    where
        Sender: Actor<ActorNames>,
        Receiver: Actor<ActorNames> + Handler<ActorNames, Message>,
    {
        let receiver = self.actors.get::<Receiver>();

        let before = receiver.outbox.time();
        let result = receiver.obj.recv(&mut receiver.outbox, msg, time, limit);
        let after = receiver.outbox.time();

        let sender = self.actors.get::<Sender>();
        let before_delivered = sender.outbox.time();
        sender.obj.message_delivered(&mut sender.outbox, time);
        let after_delivered = sender.outbox.time();

        if cfg!(feature = "cached") {
            if self.is_cached[Receiver::name()] != UNCACHED {
                return result;
            }
            // First, make sure the queue is in a valid state
            if before != after && self.is_cached[Receiver::name()] == UNCACHED && before != Time::MAX {
                self.queue_remove(Receiver::name());

                // receiver might have been the cache limit, so update it
                if cfg!(feature = "updating_cache")  {
                    self.cache_limit = match self.queue_head {
                        Some(head_id) => self.get_time(head_id),
                        None => Time::MAX,
                    }
                }
            }

            let empty_slot = if before_delivered != after_delivered {
                Some(self.is_cached[Sender::name()] as usize)
            } else {
                if cfg!(feature = "updating_cache") && after_delivered > self.cache_limit {
                    // Sender needs to leave the cache
                    Some(self.cache_remove(Sender::name(), time))
                } else {
                    None
                }
            };

            if before != after {
                match self.is_cached[Receiver::name()] {
                    UNCACHED => {
                        if after < self.cache_limit {
                            match empty_slot {
                                Some(slot) => {
                                    // We can take sender's position in the cache
                                    self.cache_replace(slot, Receiver::name());
                                }
                                None => {
                                    self.cache_insert(Receiver::name());
                                }
                            }
                        } else {
                            // Put receiver back into the queue
                            self.queue_add(Receiver::name(), after);
                        }
                    }
                    _ => { } // Receiver is already in the cache
                }
            }
        }
        else if cfg!(feature = "linked_list") {
            if before != after && before != Time::MAX {
                // Receiver already had a message queued, but it's replacing it
                self.queue_remove(Receiver::name());
            }
            // PERF: before_delivered will always be Time::MAX (because of take_message) so we could
            //       use `after_delivered != Time::MAX`, but llvm doesn't seem to detect that as dead
            //       code, so this generally produces better code
            if before_delivered != after_delivered {
                debug_assert!(after_delivered != Time::MAX);
                self.queue_add(Sender::name(), after_delivered);
            }
            if before != after && after != Time::MAX {
                // Receiver has a message queued
                self.queue_add(Receiver::name(), after);
            }
        }

        result
    }

    fn execute_message_self<Receiver, Message>(&mut self, msg: Message, time: Time, limit: Time) -> SchedulerResult
    where
        Receiver: Actor<ActorNames> + Handler<ActorNames, Message>,
    {
        let actor = self.actors.get::<Receiver>();

        let before = actor.outbox.time();

        let result = actor.obj.recv(&mut actor.outbox, msg, time, limit);
        actor.obj.message_delivered(&mut actor.outbox, time);
        let after = actor.outbox.time();

        if cfg!(feature = "cached") {
            debug_assert!(self.is_cached[Receiver::name()] != UNCACHED);
            if cfg!(feature = "updating_cache") && before != after && after >= self.cache_limit {
                 // remove from cache
                 self.cache_remove(Receiver::name(), after);
            }
        }
        else if cfg!(feature = "linked_list") && before != after {
            // The main scheduler loop already popped us from the queue
            self.queue_add(Receiver::name(), after);
        }

        result
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

    //println!("{:?} -> {:?} @ ({})", Sender::name(), Receiver::name(), time);

    if Receiver::name() == Sender::name() {
        scheduler.execute_message_self::<Receiver, Message>(message, time, limit)
    } else {
        scheduler.execute_message::<Sender, Receiver, _>(message, time, limit)
    }
}

pub(super) fn endpoint_execute<'a, ActorNames, Sender, Message>(_: ActorNames, _scheduler: &'a mut Scheduler<ActorNames>, _limit: Time) -> SchedulerResult
where
    ActorNames: MakeNamed,
    Message: 'static,
    Sender: Actor<ActorNames>,
    <Sender as Actor<ActorNames>>::OutboxType: OutboxSend<ActorNames, Message>,
{
    todo!();
    // let mut packet_view = scheduler.actors.get_view::<Sender>().map(
    //     |actor_box| {
    //         let packet = actor_box.outbox.as_packet();
    //         // Safety: Type checked in MessagePacket::from_endpoint
    //         unsafe { packet.unwrap_unchecked() }
    //     }
    // );
    // let (endpoint_fn, time) =
    //     packet_view.run(|p| (p.endpoint_fn.clone(), p.time.clone()));

    // // Safety: Type checked in MessagePacket::from_endpoint
    // let endpoint_fn = unsafe { endpoint_fn.assume_init() };

    // let result = (endpoint_fn)(packet_view, limit);


    // result
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
