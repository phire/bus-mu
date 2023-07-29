use std::{collections::BinaryHeap, cmp::Reverse};

use crate::{object_map::ObjectStore, Time, Actor, message_packet::MessagePacket, MakeNamed, EnumMap};

pub struct Scheduler<ActorNames> where
    ActorNames: MakeNamed,
    usize: From<ActorNames>,
    [(); ActorNames::COUNT]:
{
    actors: ObjectStore<ActorNames>,
    committed: EnumMap<Time, ActorNames>,
    horizon: BinaryHeap<Entry<ActorNames>>,

    min_commited_time: Time,
}

impl<ActorNames> Scheduler<ActorNames> where
ActorNames: MakeNamed,
    usize: From<ActorNames>,
    <ActorNames as MakeNamed>::Base: Actor<ActorNames>,
    [(); ActorNames::COUNT]:
 {
    pub fn new() -> Scheduler<ActorNames> {
        Scheduler {
            actors: ObjectStore::new(),
            committed: EnumMap::new(),
            horizon: BinaryHeap::default(),
            min_commited_time: Time::default(),
        }
    }

    pub fn run(&mut self) {
        let mut message = MessagePacket::no_message(Time::default());

        for actor in ActorNames::iter() {
            let time = self.actors.get_id(actor).horizon();
            self.horizon.push(Entry { time, actor });
        }
        assert!(ActorNames::COUNT > 0);

        loop {
            match message {
                MessagePacket { inner: None, time: _ } => {
                    // Find the actor with the smallest horizon, so we can advance it
                    let next = self.horizon.pop().expect("Error: No actors?");

                    // The next-smallest horizon is how far we can advance
                    let limit = self.horizon.peek().expect("Error: No actors?");

                    let actor = self.actors.get_id(next.actor);

                    message = actor.advance(limit.time);

                    // update horizon
                    self.horizon.push(Entry { time: actor.horizon(), actor: next.actor });
                }
                MessagePacket { inner: Some(m), time } => {
                    match time {
                        time if time == self.min_commited_time => {
                            // We have a message for the current time, deliver it
                            message = m.execute(&mut self.actors, time);
                        }
                        time if time > self.min_commited_time => {
                            // We have a message for the future, we need to advance all actors to that time
                            for actor in ActorNames::iter() {
                                if self.committed[actor] < time {
                                    let val = self.actors.get_id(actor).advance(time);
                                    assert!(val.inner.is_none());
                                }
                            }
                            message = m.execute(&mut self.actors, time);
                        }
                        _ => {
                            panic!("Message sent to the past")
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
struct Entry<ActorNames> {
    time: Time,
    actor: ActorNames,
}

impl<ActorNames> PartialEq for Entry<ActorNames> {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl<ActorNames> Eq for Entry<ActorNames> {}

impl<ActorNames> PartialOrd for Entry<ActorNames> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Reverse(self.time).partial_cmp(&Reverse(other.time))
    }
}

impl <ActorNames> Ord for Entry<ActorNames> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        Reverse(self.time).cmp(&Reverse(other.time))
    }
}
