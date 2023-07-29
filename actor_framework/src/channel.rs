use crate::{MakeNamed, object_map::ObjectStore, Time, MessagePacket, message_packet::MessagePacketInner, Handler, Addr};

impl<A, Name> Addr<A, Name>
 where Name: MakeNamed,
[(); Name::COUNT]:
{
    pub fn make_channel<M>(&self) -> Channel<M, Name>
    where A : Handler<M, Name> + 'static,
          M: 'static,
    {
        Channel {
            channel_fn: channel_fn::<A, M, Name>,
            actor_name: A::name(),
        }
    }
}

pub struct Channel<M, Name>
    where Name: MakeNamed,
        [(); Name::COUNT]:
{
    channel_fn: fn (map: &mut ObjectStore<Name>, time: Time, message: M) -> MessagePacket<Name>,
    actor_name: Name,
}

impl<M, Name> Channel<M, Name>
    where M: 'static + core::fmt::Debug,
    Name: MakeNamed,
    [(); Name::COUNT] :
{
    pub fn send(&self, message: M, time: Time) -> MessagePacket<Name> {
        MessagePacket {
            inner: Some(Box::new(ChannelMessage::<M, Name>
            {
                message: message,
                channel_fn: self.channel_fn,
                actor_name: self.actor_name,
            })),
            time,
        }
    }
}

fn channel_fn<A, M, Name>(map: &mut ObjectStore<Name>, time: Time, message: M) -> MessagePacket<Name>
where A: Handler<M, Name>,
      M: 'static,
      Name: MakeNamed,
      [(); Name::COUNT]:
{
    map.get::<A>().recv(time, message)
}


#[derive(Debug)]
pub struct ChannelMessage<M, Name>
where M: 'static,
      Name: MakeNamed,
      [(); Name::COUNT]:
{
    message: M,
    channel_fn: fn (map: &mut ObjectStore<Name>, time: Time, message: M) -> MessagePacket<Name>,
    actor_name: Name,
}

impl<M, Name> MessagePacketInner<Name> for ChannelMessage<M, Name>
        where M: 'static + core::fmt::Debug,
            Name: MakeNamed,
            [(); Name::COUNT]:
{
    fn execute(self: Box<Self>, map: &mut ObjectStore<Name>, time: Time) -> MessagePacket<Name> {
        (self.channel_fn)(map, time, self.message)
    }

    fn actor_name(&self) -> Name {
        self.actor_name
    }
}
