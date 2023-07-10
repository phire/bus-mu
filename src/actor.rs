

use std::{marker::PhantomData};
use crate::object_map::{ObjectMap, Named, MakeNamed};


trait MessagePacketInner<Name>
where Name: MakeNamed, [(); Name::COUNT]:,
{
    fn execute(self: Box<Self>, map: &mut ObjectMap<Name>) -> MessagePacket<Name>;
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
    fn execute(self: Box<Self>, map: &mut ObjectMap<Name>) -> MessagePacket<Name> {
        map.get::<A>().recv(self.message)
    }
}

pub struct MessagePacketChannel<M, Name>
where M: 'static,
      Name: MakeNamed,
      [(); Name::COUNT]:
{
    message: M,
    channel_fn: fn (map: &mut ObjectMap<Name>, message: M) -> MessagePacket<Name>,
}

impl<M, Name> MessagePacketInner<Name> for MessagePacketChannel<M, Name>
        where M: 'static,
            Name: MakeNamed,
            [(); Name::COUNT]:
{
    fn execute(self: Box<Self>, map: &mut ObjectMap<Name>) -> MessagePacket<Name> {
        (self.channel_fn)(map, self.message)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Time {
    // TODO: allow lazy times
    cycles: u64
}

fn channel_fn<A, M, Name>(map: &mut ObjectMap<Name>, message: M) -> MessagePacket<Name>
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
    channel_fn: fn (map: &mut ObjectMap<Name>, message: M) -> MessagePacket<Name>,
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


pub struct Scheduler<ActorNames> where
    ActorNames: MakeNamed,
    usize: From<ActorNames>,
    [(); ActorNames::COUNT]:
{
    actors: ObjectMap<ActorNames>,
    commited_time: Time,
}

impl<ActorNames> Scheduler<ActorNames> where
ActorNames: MakeNamed,
    usize: From<ActorNames>,
    ActorNames: MakeNamed,
    [(); ActorNames::COUNT]:
 {
    pub fn new() -> Scheduler<ActorNames> {
        Scheduler {
            actors: ObjectMap::new(),
            commited_time: Time { cycles: 0 }
        }
    }

    pub fn run(&mut self) {
        let mut message = MessagePacket::no_message();
        loop {
            match message {
                MessagePacket { inner: None, time: _ } => {
                    // Find the actor with the smallest window and advance it's time
                }
                MessagePacket { inner: Some(m), time } => {
                    match time {
                        time if time == self.commited_time => {
                            // We have a message for the current time, deliver it
                            message = m.execute(&mut self.actors);
                        }
                        time if time > self.commited_time => {
                            // We have a message for the future, we need to advance all actors to that time
                            todo!();
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
