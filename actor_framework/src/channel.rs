use crate::{MakeNamed, Time, MessagePacket, Handler, Addr, Actor, message_packet::ChannelFn, object_map::ObjectStore, SchedulerResult, Outbox, ActorBox, Receiver};

impl<A, ActorNames> Addr<A, ActorNames>
 where ActorNames: MakeNamed,
[(); ActorNames::COUNT]:
{
    pub fn make_channel<Message>(&self) -> Channel<Message, ActorNames>
    where
        A : for<'c, 'd> Receiver<'c, 'd, ActorNames, Message>,
        //<ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
        for<'a, 'b> &'a mut ActorBox<ActorNames, A>: From<&'b mut <ActorNames as MakeNamed>::StorageType>,
    {
        Channel {
            channel_fn: receive_for_channel::<'a, ActorNames, A, Message>,
            //actor_name: A::name(),
        }
    }
}

#[derive(Copy)]
pub struct Channel<M, ActorNames>
    where
        ActorNames: MakeNamed,
        //<ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
        M: 'static,
{
    //channel_fn: fn (time: Time, message: M) -> MessagePacket<ActorNames, M>,
    pub(super) channel_fn: for<'a> fn(packet: &'a mut MessagePacket<ActorNames, M>, map: &'a mut ObjectStore<ActorNames>, limit: Time) -> SchedulerResult,
}

impl<'a, M, ActorNames> Clone for Channel<M, ActorNames>
    where ActorNames: MakeNamed,
        //<ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
        //M: 'static,
{
    fn clone(&self) -> Self {
        Channel {
            channel_fn: self.channel_fn,
        }
    }
}

// fn channel_fn<A, M, Name>(time: Time, message: M) -> MessagePacket<Name, M>
// where A: Handler<Name, M> + Actor<Name>,
//       M: 'static,
//       Name: MakeNamed,
//       <Name as MakeNamed>::Base: crate::Actor<Name>,
//       [(); Name::COUNT]:
// {
//     MessagePacket::new_channel::<A>(time, message)
// }

fn receive_for_channel<'a, ActorNames, Receiver, Message>(
    packet: &'a mut MessagePacket<ActorNames, Message>,
    map: &'a mut ObjectStore<ActorNames>, limit: Time
) -> SchedulerResult
where
    ActorNames: MakeNamed,
    Receiver: for<'c, 'd> crate::Receiver<'c, 'd, ActorNames, Message> + 'a,
    //<ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
    // Message: 'static,
    // Receiver: Handler<ActorNames, Message> + Actor<ActorNames> + 'b,
    // <Receiver as Actor<ActorNames>>::OutboxType: Outbox<ActorNames>,
    &'a mut ActorBox<ActorNames, Receiver>: for<'b> From<&'b mut <ActorNames as MakeNamed>::StorageType>,
{
    let (time, message) = unsafe { packet.take() };

    Receiver::receive(map, message, time, limit)
}
