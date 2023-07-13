

use std::marker::PhantomData;
use crate::{object_map::{ObjectStore, MakeNamed}, Handler, Time};

pub(crate) trait MessagePacketInner<Name>
where Name: MakeNamed, [(); Name::COUNT]:,
{
    fn execute(self: Box<Self>, map: &mut ObjectStore<Name>) -> MessagePacket<Name>;
}

pub struct MessagePacket<Name> {
    pub(crate) inner: Option<Box<dyn MessagePacketInner<Name>>>,
    pub time: Time,
}

impl<Name> MessagePacket<Name>
    where Name: MakeNamed, [(); Name::COUNT]:,
{
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

pub struct Channel<M, Name>
    where Name: MakeNamed,
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
