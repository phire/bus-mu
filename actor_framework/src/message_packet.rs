

use std::{mem::{MaybeUninit, ManuallyDrop}, any::TypeId};

use crate::{channel::Channel, scheduler, Actor, Endpoint, Handler, MakeNamed, OutboxSend, Scheduler, SchedulerResult, Time};

#[derive(Debug)]
#[repr(C)]
pub struct MessagePacketProxy<ActorNames>
where
    ActorNames: MakeNamed,
{

    pub time: Time,
    pub(crate) execute_fn: scheduler::ExecuteFn<ActorNames>,
    msg_type: TypeId,
}

#[repr(C)]
pub struct MessagePacket<ActorNames, Message>
where
    ActorNames: MakeNamed,
    Message: 'static, // For TypeId
{
    pub time: Time,
    pub(crate) execute_fn: scheduler::ExecuteFn<ActorNames>,
    msg_type: std::any::TypeId,

    pub(crate) endpoint_fn: MaybeUninit<scheduler::EndpointFn<ActorNames, Message>>,
    data: MaybeUninit<ManuallyDrop<Message>>,
}

impl<ActorNames> MessagePacketProxy<ActorNames>
where
    ActorNames: MakeNamed,
{
    pub fn is_some(&self) -> bool {
        !std::ptr::fn_addr_eq(self.execute_fn, scheduler::null_execute_fn::<ActorNames>())
    }
    pub fn msg_type(&self) -> TypeId {
        self.msg_type
    }
}

impl<ActorNames, Message> MessagePacket<ActorNames, Message>
where
    ActorNames: MakeNamed,
    Message: 'static,
{
    pub fn is_some(&self) -> bool {
        !std::ptr::fn_addr_eq(self.execute_fn, scheduler::null_execute_fn::<ActorNames>())
    }

    pub fn msg_type(&self) -> TypeId {
        self.msg_type
    }

    pub fn new<'a, 'b, Sender, Receiver>(time: Time, data: Message) -> Self
    where
        Receiver: Actor<ActorNames> + Handler<ActorNames, Message>,
        Sender: Actor<ActorNames>,
        <Sender as Actor<ActorNames>>::OutboxType: OutboxSend<ActorNames, Message>,
    {
        Self {
            time,
            // Safety: It is essential that we instantiate the correct execute_fn
            //         template here. It relies on this function for type checking
            execute_fn: scheduler::direct_execute::<ActorNames, Sender, Receiver, Message>,
            msg_type: TypeId::of::<Message>(),
            endpoint_fn: MaybeUninit::uninit(),
            data: MaybeUninit::new(ManuallyDrop::new(data)),
        }
    }

    pub fn from_endpoint<Sender>(endpoint: Endpoint<ActorNames, Message>, time: Time, data: Message) -> Self
    where
        Sender: Actor<ActorNames>,
        <Sender as Actor<ActorNames>>::OutboxType: OutboxSend<ActorNames, Message>,
    {
        Self {
            time,
            // Safety: It is essential that we instantiate the correct execute_fn
            //         template here. It relies on this function for type checking
            execute_fn: crate::scheduler::endpoint_execute::<ActorNames, Sender, Message>,
            msg_type: TypeId::of::<Message>(),
            endpoint_fn: MaybeUninit::new(endpoint.endpoint_fn),
            data: MaybeUninit::new(ManuallyDrop::new(data)),
        }
    }

    pub fn from_channel<Sender>(channel: Channel<ActorNames, Sender, Message>, data: Message, time: Time) -> Self
    where
        Sender: Actor<ActorNames>,
    {
        Self {
            time,
            // Safety: Channel::new ensures that the execute_fn is correct
            execute_fn: channel.execute_fn,
            msg_type: TypeId::of::<Message>(),
            endpoint_fn: MaybeUninit::uninit(),
            data: MaybeUninit::new(ManuallyDrop::new(data)),
        }
    }

    pub fn take<'b>(&'b mut self) -> Option<(Time, Message)> {
        if self.msg_type != TypeId::of::<Message>() {
            return None
        }

        self.msg_type = TypeId::of::<()>();
        self.execute_fn = scheduler::null_execute_fn::<ActorNames>();

        let mut time = Time::MAX;
        std::mem::swap(&mut self.time, &mut time);
        let mut data = MaybeUninit::uninit();
        std::mem::swap(&mut self.data, &mut data);

        // Safety: Depends on the msg_type being correct
        let data = unsafe { data.assume_init()};
        Some((time, ManuallyDrop::into_inner(data)))
    }
}

impl<'a, ActorNames, Message> Drop for MessagePacket<ActorNames, Message>
where
    ActorNames: MakeNamed,
    Message: 'static,
{
    fn drop(&mut self) {
        assert!(self.msg_type == TypeId::of::<Message>());

        self.take();
    }
}

impl<ActorNames> Default for MessagePacket<ActorNames, ()>
where
    ActorNames: MakeNamed,
{
    fn default() -> Self {
        Self {
            time: Time::MAX,
            execute_fn: scheduler::null_execute_fn::<ActorNames>(),
            msg_type: TypeId::of::<()>(),
            endpoint_fn: MaybeUninit::uninit(),
            data: MaybeUninit::new(ManuallyDrop::new(())),
        }
    }
}
