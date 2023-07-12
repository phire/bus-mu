

use std::{marker::PhantomData, collections::BinaryHeap, cmp::Reverse};
use crate::object_map::{ObjectStore, Named, MakeNamed, EnumMap};


trait MessagePacketInner<Name>
where Name: MakeNamed, [(); Name::COUNT]:,
{
    fn execute(self: Box<Self>, map: &mut ObjectStore<Name>) -> MessagePacket<Name>;
}

trait Handler<M, Name> where Self: Named<Name> {
    fn recv(&mut self, message: M) -> MessagePacket<Name>;
}


pub struct MessagePacket<Name> {
    inner: Option<Box<dyn MessagePacketInner<Name>>>,
    time: Time,
}

impl<Name> MessagePacket<Name> {
    pub fn no_message() -> MessagePacket<Name> {
        MessagePacket {
            inner: None,
            time: Time {
                cycles: 0,
            },
        }
    }
}

pub struct MessagePacketImpl<A, M, Name> {
    message: M,
    type_id: PhantomData<*const A>,
    name_type: PhantomData<*const Name>,
}

impl<A, M, Name> MessagePacketInner<Name> for MessagePacketImpl<A, M, Name>
     where A: Handler<M, Name>, Name: MakeNamed, [(); Name::COUNT]:
{
    fn execute(self: Box<Self>, map: &mut ObjectStore<Name>) -> MessagePacket<Name> {
        map.get::<A>().recv(self.message)
    }
}

pub struct MessagePacketChannel<M, Name>
where M: 'static,
      Name: MakeNamed,
      [(); Name::COUNT]:
{
    message: M,
    channel_fn: fn (map: &mut ObjectStore<Name>, message: M) -> MessagePacket<Name>,
}

impl<M, Name> MessagePacketInner<Name> for MessagePacketChannel<M, Name>
        where M: 'static,
            Name: MakeNamed,
            [(); Name::COUNT]:
{
    fn execute(self: Box<Self>, map: &mut ObjectStore<Name>) -> MessagePacket<Name> {
        (self.channel_fn)(map, self.message)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Time {
    // TODO: allow lazy times
    cycles: u64
}

impl Default for Time {
    fn default() -> Self {
        Time {
            cycles: 0,
        }
    }
}

fn channel_fn<A, M, Name>(map: &mut ObjectStore<Name>, message: M) -> MessagePacket<Name>
where A: Handler<M, Name>,
      M: 'static,
      Name: MakeNamed,
      [(); Name::COUNT]:
{
    map.get::<A>().recv(message)
}

struct Addr<Actor, Name> {
    actor_type: PhantomData<*const Actor>,
    named_type: PhantomData<*const Name>,
}

impl<A, Name> Addr<A, Name>
 where Name: MakeNamed,
[(); Name::COUNT]:
{
    pub fn send<M>(&self, message: M, time: Time) -> MessagePacket<Name>
    where A: Handler<M, Name> + 'static,
          M: 'static,
    {
        // TODO: Don't box messages
        MessagePacket {
            inner: Some(Box::new(MessagePacketImpl::<A, M, Name>
            {
                message: message,
                type_id: PhantomData::<*const A>,
                name_type: PhantomData::<*const Name>,
            })),
            time,
        }
    }

    pub fn make_channel<M>(&self) -> Channel<M, Name>
    where A : Handler<M, Name> + 'static,
          M: 'static,
    {
        Channel {
            channel_fn: channel_fn::<A, M, Name>,
        }
    }
}

pub struct Channel<M, Name> where Name: MakeNamed,
[(); Name::COUNT]:
{
    channel_fn: fn (map: &mut ObjectStore<Name>, message: M) -> MessagePacket<Name>,
}

impl<M, Name> Channel<M, Name>
    where M: 'static,
    Name: MakeNamed,
    [(); Name::COUNT] :
{
    pub fn send(&self, message: M, time: Time) -> MessagePacket<Name> {
        MessagePacket {
            inner: Some(Box::new(MessagePacketChannel::<M, Name>
            {
                message: message,
                channel_fn: self.channel_fn,
            })),
            time,
        }
    }
}

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

pub trait Actor<ActorNames> : Named<ActorNames> {
    fn advance(&self, limit: Time) -> MessagePacket<ActorNames>;
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
        let mut message = MessagePacket::no_message();

        for actor in ActorNames::iter() {
            self.horizon.push(Entry { time: Time::default(), actor });
        }
        assert!(ActorNames::COUNT > 0);

        loop {
            match message {
                MessagePacket { inner: None, time: _ } => {
                    // Find the actor with the smallest horizon, so we can advance it
                    let next = self.horizon.pop().expect("Error: No actors?");

                    // The next-smallest horizon is how far we can advance
                    let limit = self.horizon.peek().expect("Error: No actors?");

                    message = self.actors.get_id(next.actor).advance(limit.time);
                }
                MessagePacket { inner: Some(m), time } => {
                    match time {
                        time if time == self.min_commited_time => {
                            // We have a message for the current time, deliver it
                            message = m.execute(&mut self.actors);
                        }
                        time if time > self.min_commited_time => {
                            // We have a message for the future, we need to advance all actors to that time
                            for actor in ActorNames::iter() {
                                if self.committed[actor] < time {
                                    let val = self.actors.get_id(actor).advance(time);
                                    assert!(val.inner.is_none());
                                }
                            }
                            message = m.execute(&mut self.actors);
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
