use crate::{MakeNamed, Handler, Addr, Actor, message_packet::ChannelFn};


#[derive(Copy)]
pub struct Channel<ActorNames, Message>
    where
        ActorNames: MakeNamed,
        Message: 'static
{
    pub(super) channel_fn: ChannelFn<ActorNames, Message>,
}

impl<ActorNames, Message> Channel<ActorNames, Message>
where
    ActorNames: MakeNamed
{
    pub fn new<Receiver>() -> Channel<ActorNames, Message>
    where
        Receiver : Handler<ActorNames, Message> + Actor<ActorNames>,
    {
        Channel {
            channel_fn: crate::message_packet::receive_for_channel::<ActorNames, Receiver, Message>,
        }
    }
}

impl<ActorNames, Message> Clone for Channel<ActorNames, Message>
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

impl<A, ActorNames> Addr<A, ActorNames>
 where ActorNames: MakeNamed,
{
    pub fn make_channel<Message>(&self) -> Channel<ActorNames, Message>
    where
        A : Handler<ActorNames, Message> + Actor<ActorNames>,
    {
        Channel {
            channel_fn: crate::message_packet::receive_for_channel::<ActorNames, A, Message>,
        }
    }
}
