use crate::{MakeNamed, Time, MessagePacket, Handler, Addr, Actor};

impl<A, ActorNames> Addr<A, ActorNames>
 where ActorNames: MakeNamed,
[(); ActorNames::COUNT]:
{
    pub fn make_channel<M>(&self) -> Channel<M, ActorNames>
    where A : Handler<M> + Actor<ActorNames>,
          M: 'static,
          <ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
    {
        Channel {
            channel_fn: channel_fn::<A, M, ActorNames>,
            //actor_name: A::name(),
        }
    }
}

pub struct Channel<M, ActorNames>
    where ActorNames: MakeNamed,
        [(); ActorNames::COUNT]:,
        <ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
        M: 'static,
{
    channel_fn: fn (time: Time, message: M) -> MessagePacket<ActorNames, M>,
    //actor_name: ActorNames,
}

impl<M, ActorNames> Channel<M, ActorNames>
    where M: 'static,// + core::fmt::Debug,
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
    [(); ActorNames::COUNT] :
{
    pub fn send(&self, message: M, time: Time) -> MessagePacket<ActorNames, M> {
        (self.channel_fn)(time, message)
    }
}

fn channel_fn<A, M, Name>(time: Time, message: M) -> MessagePacket<Name, M>
where A: Handler<M> + Actor<Name>,
      M: 'static,
      Name: MakeNamed,
      <Name as MakeNamed>::Base: crate::Actor<Name>,
      [(); Name::COUNT]:
{
    MessagePacket::new_channel::<A>(time, message)
}
