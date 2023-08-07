use crate::{MakeNamed, Handler, Addr, Actor, message_packet::EndpointFn};


#[derive(Copy)]
pub struct Endpoint<ActorNames, Message>
    where
        ActorNames: MakeNamed,
        Message: 'static
{
    pub(super) endpoint_fn: EndpointFn<ActorNames, Message>,
}

impl<ActorNames, Message> Endpoint<ActorNames, Message>
where
    ActorNames: MakeNamed
{
    pub fn new<Receiver>() -> Endpoint<ActorNames, Message>
    where
        Receiver : Handler<ActorNames, Message> + Actor<ActorNames>,
    {
        Endpoint {
            endpoint_fn: crate::message_packet::receive_for_endpoint::<ActorNames, Receiver, Message>,
        }
    }
}

impl<ActorNames, Message> Clone for Endpoint<ActorNames, Message>
    where ActorNames: MakeNamed,
        //<ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
        //M: 'static,
{
    fn clone(&self) -> Self {
        Endpoint {
            endpoint_fn: self.endpoint_fn,
        }
    }
}

impl<A, ActorNames> Addr<A, ActorNames>
 where ActorNames: MakeNamed,
{
    pub fn make_channel<Message>(&self) -> Endpoint<ActorNames, Message>
    where
        A : Handler<ActorNames, Message> + Actor<ActorNames>,
    {
        Endpoint {
            endpoint_fn: crate::message_packet::receive_for_endpoint::<ActorNames, A, Message>,
        }
    }
}
