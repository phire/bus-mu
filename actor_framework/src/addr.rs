use std::marker::PhantomData;

use crate::{MakeNamed, Handler, MessagePacket, Time, object_map::ObjectStore, Named, message_packet::MessagePacketInner};

pub struct Addr<Actor, Name> {
    actor_type: PhantomData<*const Actor>,
    named_type: PhantomData<*const Name>,
}

impl<Actor, Name> Default for Addr<Actor, Name> {
    fn default() -> Self {
        Self {
            actor_type: PhantomData::<*const Actor>,
            named_type: PhantomData::<*const Name>,
        }
    }
}

impl<A, Name> Addr<A, Name>
 where Name: MakeNamed,
[(); Name::COUNT]:
{
    pub fn send<M>(&self, message: M, time: Time) -> MessagePacket<Name>
    where A: Handler<M, Name> + core::fmt::Debug + 'static,
          M: core::fmt::Debug + 'static,
    {
        // TODO: Don't box messages
        MessagePacket {
            inner: Some(Box::new(DirectMessage::<A, M, Name>
            {
                message: message,
                type_id: PhantomData::<*const A>,
                name_type: PhantomData::<*const Name>,
            })),
            time,
        }
    }
}

#[derive(Debug)]
pub struct DirectMessage<A, M, Name> {
    message: M,
    type_id: PhantomData<*const A>,
    name_type: PhantomData<*const Name>,
}

impl<A, M, Name> MessagePacketInner<Name> for DirectMessage<A, M, Name>
     where A: Handler<M, Name> + core::fmt::Debug,
           M: core::fmt::Debug,
           Name: MakeNamed,
           [(); Name::COUNT]:
{
    fn execute(self: Box<Self>, map: &mut ObjectStore<Name>, time: Time) -> MessagePacket<Name> {
        map.get::<A>().recv(time, self.message)
    }
    fn actor_name(&self) -> Name {
        A::name()
    }
}

trait MakeAddr<Name> where Self: Sized, Name: MakeNamed, [(); Name::COUNT]: {
    fn make_addr() -> Addr<Self, Name>;
}

impl<Name, A> MakeAddr<Name> for A where
    Name: MakeNamed,
    A: Named<Name>,
    [(); Name::COUNT]:
{
    fn make_addr() -> Addr<Self, Name> {
        Addr {
            actor_type: PhantomData::<*const Self>,
            named_type: PhantomData::<*const Name>,
        }
    }
}

