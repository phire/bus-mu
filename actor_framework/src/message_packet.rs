

use std::{mem::{MaybeUninit, ManuallyDrop}, any::TypeId};

use crate::{object_map::{ObjectStore, ObjectStoreView}, MakeNamed, Time, Handler, Actor, Endpoint, SchedulerResult, OutboxSend};

pub type ExecuteFn<ActorNames> = for<'a> fn(sender_id: ActorNames, map: &'a mut ObjectStore<ActorNames>, limit: Time) -> SchedulerResult;
pub type EndpointFn<ActorNames, Message> =
    for<'a> fn(
        packet_view: ObjectStoreView<'a, ActorNames, MessagePacket<ActorNames, Message>>,
        limit: Time)
    -> SchedulerResult;

#[derive(Debug)]
#[repr(C)]
pub struct MessagePacketProxy<ActorNames>
where
    ActorNames: MakeNamed,
{

    pub time: Time,
    pub(crate) execute_fn: ExecuteFn<ActorNames>,
    msg_type: TypeId,
}

#[repr(C)]
pub struct MessagePacket<ActorNames, Message>
where
    ActorNames: MakeNamed,
    Message: 'static, // For TypeId
{
    pub time: Time,
    pub(crate) execute_fn: ExecuteFn<ActorNames>,
    msg_type: std::any::TypeId,

    endpoint_fn: EndpointFn<ActorNames, Message>,
    data: MaybeUninit<ManuallyDrop<Message>>,
}

fn direct_execute<'a, ActorNames, Sender, Receiver, Message>(_: ActorNames, map: &'a mut ObjectStore<ActorNames>, limit: Time) -> SchedulerResult
where
    ActorNames: MakeNamed,
    Message: 'static,
    Receiver: Actor<ActorNames> + Handler<ActorNames, Message>,
    Sender: Actor<ActorNames>,
    <Sender as Actor<ActorNames>>::OutboxType: OutboxSend<ActorNames, Message>,
{
    let actor = map.get::<Sender>();
    let packet = actor.outbox.as_packet();
    let (time, message) = {

        // Safety: Type checked in MessagePacket::new
         unsafe { packet.unwrap_unchecked().take() }
    };

    let receiver = map.get::<Receiver>();
    let result = receiver.actor.recv(&mut receiver.outbox, message, time, limit);

    let actor = map.get::<Sender>();
    actor.actor.message_delivered(&mut actor.outbox, time);

    result
}

pub(super) fn receive_for_endpoint<ActorNames, Receiver, Message>(
    mut packet_view: ObjectStoreView<'_, ActorNames, MessagePacket<ActorNames, Message>>,
    limit: Time
) -> SchedulerResult
where
    ActorNames: MakeNamed,
    Receiver: Handler<ActorNames, Message> + Actor<ActorNames>,
{
    let (time, message) = packet_view.run(|p| unsafe { p.take() });

    let receiver = packet_view.get_obj::<Receiver>();
    receiver.actor.recv(&mut receiver.outbox, message, time, limit)
}

fn endpoint_execute<'a, ActorNames, Sender, Message>(_: ActorNames, map: &'a mut ObjectStore<ActorNames>, limit: Time) -> SchedulerResult
where
    ActorNames: MakeNamed,
    Message: 'static,
    Sender: Actor<ActorNames>,
    <Sender as Actor<ActorNames>>::OutboxType: OutboxSend<ActorNames, Message>,
{
    let mut packet_view = map.get_view::<Sender>().map(
        |actor_box| {
            let packet = actor_box.outbox.as_packet();
            unsafe { packet.unwrap_unchecked() }
        }
    );
    let (endpoint_fn, time) =
        packet_view.run(|p| (p.endpoint_fn.clone(), p.time.clone()));

    let result = (endpoint_fn)(packet_view, limit);

    let actor = map.get::<Sender>();
    actor.actor.message_delivered(&mut actor.outbox, time);

    result
}

fn null_execute<ActorNames>(_: ActorNames, _: &mut ObjectStore<ActorNames>, _: Time) -> SchedulerResult
where
    ActorNames: MakeNamed,
{
    panic!("Scheduler tried to execute an empty message");
}

fn null_channel<ActorNames, Message>(_packet: ObjectStoreView<'_, ActorNames, MessagePacket<ActorNames, Message>>, _: Time) ->  SchedulerResult
where
    ActorNames: MakeNamed,
{
    unreachable!()
}

impl<ActorNames> MessagePacketProxy<ActorNames>
where
    ActorNames: MakeNamed,
{
    pub fn is_some(&self) -> bool {
        self.execute_fn != null_execute::<ActorNames>
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
        self.execute_fn != null_execute::<ActorNames>
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
            execute_fn: direct_execute::<ActorNames, Sender, Receiver, Message>,
            msg_type: TypeId::of::<Message>(),
            endpoint_fn: null_channel::<ActorNames, Message>,
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
            execute_fn: endpoint_execute::<ActorNames, Sender, Message>,
            msg_type: TypeId::of::<Message>(),
            endpoint_fn: endpoint.endpoint_fn,
            data: MaybeUninit::new(ManuallyDrop::new(data)),
        }
    }

    pub unsafe fn take<'b>(&'b mut self) -> (Time, Message) {
        //debug_assert!(self.execute_fn.is_some());
        debug_assert!(self.msg_type == TypeId::of::<Message>());

        self.msg_type = TypeId::of::<()>();
        self.execute_fn = null_execute::<ActorNames>;

        let mut time = Time::MAX;
        std::mem::swap(&mut self.time, &mut time);
        let mut data = MaybeUninit::uninit();
        std::mem::swap(&mut self.data, &mut data);

        (time, ManuallyDrop::into_inner(data.assume_init()))
    }
}

impl<'a, ActorNames, Message> Drop for MessagePacket<ActorNames, Message>
where
    ActorNames: MakeNamed,
    Message: 'static,
{
    fn drop(&mut self) {
        assert!(self.msg_type == TypeId::of::<Message>());

        unsafe {
            self.take();
        }
    }
}

impl<ActorNames> Default for MessagePacket<ActorNames, ()>
where
    ActorNames: MakeNamed,
{
    fn default() -> Self {
        Self {
            time: Time::MAX,
            execute_fn: null_execute::<ActorNames>,
            msg_type: TypeId::of::<()>(),
            endpoint_fn: null_channel::<ActorNames, ()>,
            data: MaybeUninit::new(ManuallyDrop::new(())),
        }
    }
}
