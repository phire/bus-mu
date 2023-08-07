

use std::{mem::{MaybeUninit, ManuallyDrop}, any::TypeId};

use crate::{object_map::{ObjectStore, ObjectStoreView}, MakeNamed, Time, Handler, Actor, Channel, SchedulerResult};

pub type ExecuteFn<ActorNames> = for<'a> fn(sender_id: ActorNames, map: &'a mut ObjectStore<ActorNames>, limit: Time) -> SchedulerResult;
pub type ChannelFn<ActorNames, Message> =
    for<'a> fn(
        packet_view: ObjectStoreView<'a, ActorNames, MessagePacket<ActorNames, Message>>,
        limit: Time)
     -> (ObjectStoreView<'a, ActorNames, MessagePacket<ActorNames, Message>>, SchedulerResult);

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
    channel_fn: ChannelFn<ActorNames, Message>,
    //actor_name: ActorNames,
    data: MaybeUninit<ManuallyDrop<Message>>,
}

fn direct_execute2<'a, ActorNames, Sender, Receiver, Message>(_: ActorNames, map: &'a mut ObjectStore<ActorNames>, limit: Time) -> SchedulerResult
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

pub(super) fn receive_for_channel<ActorNames, Receiver, Message>(
    mut packet_view: ObjectStoreView<'_, ActorNames, MessagePacket<ActorNames, Message>>,
    limit: Time
) -> (ObjectStoreView<'_, ActorNames, MessagePacket<ActorNames, Message>>, SchedulerResult)
where
    ActorNames: MakeNamed,
    Receiver: Handler<ActorNames, Message> + Actor<ActorNames>,
{
    let (time, message) = packet_view.run(|p| unsafe { p.take() });

    let receiver = packet_view.get_obj::<Receiver>();

    let result = receiver.actor.recv(&mut receiver.outbox, message, time, limit);
    (packet_view, result)
}


fn new_channel_execute<'a, ActorNames, Sender, Message>(_: ActorNames, map: &'a mut ObjectStore<ActorNames>, limit: Time) -> SchedulerResult
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
    let (channel_fn, time) =
        packet_view.run(|p| (p.channel_fn.clone(), p.time.clone()));

    let (_, result) = (channel_fn)(packet_view, limit);

    let actor = map.get::<Sender>();
    actor.actor.message_delivered(&mut actor.outbox, time);

    result
}

fn null_execute<ActorNames>(_: ActorNames, _: &mut ObjectStore<ActorNames>, _: Time) -> SchedulerResult
where
    ActorNames: MakeNamed,
    //<ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
{
    panic!("Scheduler tried to execute an empty message");
}

fn null_channel<ActorNames, Message>(_packet: ObjectStoreView<'_, ActorNames, MessagePacket<ActorNames, Message>>, _: Time) -> (ObjectStoreView<'_, ActorNames, MessagePacket<ActorNames, Message>>, SchedulerResult)
where
    ActorNames: MakeNamed,
    //<ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
{
    unreachable!()
}

impl<ActorNames> MessagePacketProxy<ActorNames>
where
    ActorNames: MakeNamed,
    //<ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
    //[(); ActorNames::COUNT]: ,
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
    //<ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
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
            execute_fn: direct_execute2::<ActorNames, Sender, Receiver, Message>,
            msg_type: TypeId::of::<Message>(),
            channel_fn: null_channel::<ActorNames, Message>,
            data: MaybeUninit::new(ManuallyDrop::new(data)),
        }
    }

    pub fn from_channel<Sender>(channel: Channel<ActorNames, Message>, time: Time, data: Message) -> Self
    where
        Sender: Actor<ActorNames>,
        <Sender as Actor<ActorNames>>::OutboxType: OutboxSend<ActorNames, Message>,
    {
        Self {
            time,
            // Safety: It is essential that we instantiate the correct execute_fn
            //         template here. It relies on this function for type checking
            execute_fn: new_channel_execute::<ActorNames, Sender, Message>,
            msg_type: TypeId::of::<Message>(),
            channel_fn: channel.channel_fn,
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
            channel_fn: null_channel::<ActorNames, ()>,
            data: MaybeUninit::new(ManuallyDrop::new(())),
        }
    }
}

pub trait Outbox<ActorNames>
where
    ActorNames: MakeNamed,
{
    type Sender;
    fn as_mut(&mut self) -> &mut MessagePacketProxy<ActorNames>;
    fn as_ref(&self) -> &MessagePacketProxy<ActorNames>;
}

pub trait OutboxSend<ActorNames, Message>
where
    ActorNames: MakeNamed,
{
    fn send<Receiver>(&mut self, message: Message, time: Time)
    where
        Receiver: Handler<ActorNames, Message> + Actor<ActorNames>;
    fn send_channel(&mut self, channel: Channel<ActorNames, Message>, message: Message, time: Time);
    fn cancel(&mut self) -> (Time, Message);
    fn as_packet<'a>(&'a mut self) -> Option<&'a mut MessagePacket<ActorNames, Message>>;
}

#[macro_export]
macro_rules! make_outbox {
    // Main entry point of macro
    (
        // match OutboxName<ActorNames, Sender>
        $name:ident<$name_type:ty, $sender:ty>
        {
            // match One or more fields of `name: MessageType,`
            $( $i:ident : $t:ty ),+
            // match optional trailing commas
            $(,)?
        }
    ) => {
        pub union $name {
            none: core::mem::ManuallyDrop<actor_framework::MessagePacket<$name_type, ()>>,
            $($i : core::mem::ManuallyDrop<actor_framework::MessagePacket<$name_type, $t>>),+
        }

        impl $name {
            fn is_empty(&self) -> bool {
                unsafe { !self.none.is_some() }
            }
            fn msg_type(&self) -> std::any::TypeId {
                unsafe { self.none.msg_type() }
            }
            fn msg_type_name(&self) -> &'static str {
                let msg_type = self.msg_type();

                if msg_type == std::any::TypeId::of::<()>() {
                    "Empty"
                }
                $(else if msg_type == std::any::TypeId::of::<$t>() {
                    std::any::type_name::<$t>()
                })+
                else {
                    unreachable!()
                }
            }
            fn contains<Msg>(&self) -> bool
            where
                Msg: 'static,
            {
                self.msg_type() == std::any::TypeId::of::<Msg>()
            }
        }

        impl core::ops::Drop for $name {
            fn drop(&mut self) {
                let msg_type = self.msg_type();

                if msg_type == std::any::TypeId::of::<()>() {
                    unsafe { core::mem::ManuallyDrop::drop( &mut self.none) };
                }
                $(else if msg_type == std::any::TypeId::of::<$t>() {
                    unsafe { core::mem::ManuallyDrop::drop( &mut self.$i) };
                })+
            }
        }

        impl<ActorNames> actor_framework::Outbox<ActorNames> for $name
        where
            ActorNames: actor_framework::MakeNamed,
            //[(); ActorNames::COUNT]: ,
        {
            type Sender = $sender;
            fn as_mut(&mut self) -> &mut actor_framework::MessagePacketProxy<ActorNames> {
                unsafe { std::mem::transmute(self) }
            }
            fn as_ref(&self) -> &actor_framework::MessagePacketProxy<ActorNames> {
                unsafe { std::mem::transmute(self) }
            }
        }

        impl core::default::Default for $name {
            fn default() -> Self {
                Self {
                    none : Default::default(),
                }
            }
        }

        // Create all OutboxSend<MessageType> traits
        actor_framework::make_outbox!(@impl $name<$name_type>, $($i : $t),+ );
    };
    // Called for every union field to implement an OutboxSend<MessageType> trait
    (

        // match macro internal @impl tag
        @impl
        // match OutboxName<ActorNames>,
        $name:ident<$name_type:ty>,
        // match exactly one field of `name: MessageType`
        $field_ident:ident : $field_type:ty
    ) => {
        impl actor_framework::OutboxSend<$name_type, $field_type> for $name
        where
            $name_type: actor_framework::MakeNamed,
            //[(); <$name_type as actor_framework::MakeNamed>::COUNT]: ,
        {
            fn send<Receiver>(&mut self, message: $field_type, time: Time)
            where
                Receiver: Handler<$name_type, $field_type> + Actor<$name_type>
            {
                assert!(self.is_empty());

                self.$field_ident = core::mem::ManuallyDrop::new(actor_framework::MessagePacket::new::<
                    <Self as actor_framework::Outbox<$name_type>>::Sender,
                    Receiver>(time, message));
            }

            // fn send_addr<Receiver>(&mut self, addr: &actor_framework::Addr<Receiver, $name_type>, message: $field_type, time: Time)
            // where
            //     Receiver: actor_framework::Handler<$name_type, $field_type> + actor_framework::Actor<$name_type>,
            // {
            //     self.send::<Receiver>(message, time);
            // }

            fn send_channel(&mut self, channel: actor_framework::Channel<$name_type, $field_type>, message: $field_type, time: Time)
            {
                self.$field_ident = core::mem::ManuallyDrop::new(
                        actor_framework::MessagePacket::from_channel::<
                            <Self as actor_framework::Outbox<$name_type>>::Sender>
                    (channel, time, message));
            }

            fn cancel(&mut self) -> (Time, $field_type)
            {
                let msg_type = unsafe { self.none.msg_type() };
                if msg_type == std::any::TypeId::of::<$field_type>() {
                    unsafe {
                        return self.$field_ident.take();
                    }
                } else {
                    let typename = std::any::type_name::<$field_type>();
                    panic!("Outbox::cancel - Expected {} but found {:?}", typename, msg_type);
                }
            }
            fn as_packet<'a>(&'a mut self) -> Option<&'a mut actor_framework::MessagePacket<$name_type, $field_type>> {
                let msg_type = unsafe { self.none.msg_type() };
                if msg_type == std::any::TypeId::of::<$field_type>() {
                    unsafe {
                        return Some(&mut self.$field_ident);
                    }
                } else {
                    return None;
                }
            }
        }
    };
    // Call above rule for every field
    (@impl $name:ident<$name_type:ty>, $i:ident : $t:ty, $($tail:tt)+) => {
        actor_framework::make_outbox!(@impl $name<$name_type>, $i : $t);
        actor_framework::make_outbox!(@impl $name<$name_type>, $($tail)+);
    };
}
