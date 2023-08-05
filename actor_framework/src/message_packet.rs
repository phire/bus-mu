

use std::{mem::{MaybeUninit, ManuallyDrop}, any::TypeId, pin::{Pin, pin}};

use crate::{object_map::ObjectStore, MakeNamed, Time, Handler, Actor, Addr, Channel, SchedulerResult};

type ExecuteFn<ActorNames> = fn(sender_id: ActorNames, map: &mut ObjectStore<ActorNames>, limit: Time) -> SchedulerResult;

#[derive(Debug)]
#[repr(C)]
pub struct MessagePacketProxy<ActorNames>
where
    ActorNames: MakeNamed,
    [(); ActorNames::COUNT]: ,
{

    pub time: Time,
    pub(crate) execute_fn: ExecuteFn<ActorNames>,
    msg_type: TypeId,
}

#[repr(C)]
pub struct MessagePacket<ActorNames, Message>
where
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
    [(); ActorNames::COUNT]: ,
    Message: 'static,
{
    pub time: Time,
    pub(crate) execute_fn: ExecuteFn<ActorNames>,
    msg_type: std::any::TypeId,
    //actor_name: ActorNames,
    data: MaybeUninit<ManuallyDrop<Message>>,
}

fn direct_execute<ActorNames, Sender, Receiver, Message>(_: ActorNames, map: &mut ObjectStore<ActorNames>, limit: Time) -> SchedulerResult
where
    Message: 'static,
    Receiver: Handler<Message> + Actor<ActorNames>,
    Sender: crate::Actor<ActorNames>,
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
    [(); ActorNames::COUNT]: ,
{
    let proxy = map.get::<Sender>().get_message();

    // Safety: this was type checked at compile type in MessagePacket::new
    let (time, message) = unsafe {
        let packet : &mut MessagePacket<ActorNames, Message> = std::mem::transmute(proxy);
        packet.take()
    };

    let receiver = unsafe {
        map.get::<Receiver>().get_unchecked_mut()
    };

    //println!("direct_execute: {:?} {:?}", Receiver::name(), time);

    let result = receiver.recv(message, time, limit);
    map.get::<Sender>().message_delivered(time);

    result
}

fn channel_execute<ActorNames, Receiver, Message>(sender_id: ActorNames, map: &mut ObjectStore<ActorNames>, limit: Time) -> SchedulerResult
where
    Message: 'static,
    Receiver: Handler<Message> + Actor<ActorNames>,
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
    [(); ActorNames::COUNT]: ,
{
    let proxy = map.get_id(sender_id).get_message();

    // Safety: this was type checked at compile type in MessagePacket::new
    let (time, message) = unsafe {
        let packet : &mut MessagePacket<ActorNames, Message> = std::mem::transmute(proxy);
        packet.take()
    };

    let receiver = unsafe {
        map.get::<Receiver>().get_unchecked_mut()
    };

    let result = receiver.recv(message, time, limit);
    map.get_id(sender_id).message_delivered(time);

    result
}

fn null_execute<ActorNames>(_: ActorNames, _: &mut ObjectStore<ActorNames>, _: Time) -> SchedulerResult
where
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
    [(); ActorNames::COUNT]: ,
{
    panic!("Scheduler tried to execute an empty message");
}

impl<ActorNames> MessagePacketProxy<ActorNames>
where
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
    [(); ActorNames::COUNT]: ,
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
    <ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
    [(); ActorNames::COUNT]: ,
{
    pub fn is_some(&self) -> bool {
        self.execute_fn != null_execute::<ActorNames>
    }

    pub fn msg_type(&self) -> TypeId {
        self.msg_type
    }

    pub fn new<Sender, Receiver>(time: Time, data: Message) -> Self
    where
        Receiver: Handler<Message> + Actor<ActorNames>,
        Sender: crate::Actor<ActorNames>,
        Message: 'static,
    {
        Self {
            time,
            // Safety: It is essential that we instantiate the correct execute_fn
            //         template here. It relies on this function for type checking
            execute_fn: direct_execute::<ActorNames, Sender, Receiver, Message>,
            msg_type: TypeId::of::<Message>(),
            data: MaybeUninit::new(ManuallyDrop::new(data)),
        }
    }

    pub(crate) fn new_channel<Receiver>(time: Time, data: Message) -> Self
    where
        Receiver: Handler<Message> + Actor<ActorNames>,
        <ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
        Message: 'static,
    {
        Self {
            time,
            // Safety: It is essential that we instantiate the correct execute_fn
            //         template here. It relies on this function for type checking
            execute_fn: channel_execute::<ActorNames, Receiver, Message>,
            msg_type: TypeId::of::<Message>(),
            data: MaybeUninit::new(ManuallyDrop::new(data)),
        }
    }

    pub unsafe fn take(&mut self) -> (Time, Message) {
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

impl<ActorNames, Message> Drop for MessagePacket<ActorNames, Message>
where
    Message: 'static,
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
    [(); ActorNames::COUNT]: ,
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
    <ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
    [(); ActorNames::COUNT]: ,
{
    fn default() -> Self {
        Self {
            time: Time::MAX,
            execute_fn: null_execute::<ActorNames>,
            msg_type: TypeId::of::<()>(),
            data: MaybeUninit::new(ManuallyDrop::new(())),
        }
    }
}

pub trait Outbox<ActorNames>
where
    ActorNames: MakeNamed,
    [(); ActorNames::COUNT]: ,
{
    type Sender;
    fn as_mut(&mut self) -> &mut MessagePacketProxy<ActorNames>;
    fn as_ref(&self) -> &MessagePacketProxy<ActorNames>;
}

pub trait OutboxSend<ActorNames, Message>
where
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
    [(); ActorNames::COUNT]: ,
{
    fn send<Receiver>(&mut self, message: Message, time: Time)
    where
        Receiver: Handler<Message> + Actor<ActorNames>;
    fn send_addr<Receiver>(&mut self, addr: &Addr<Receiver, ActorNames>, message: Message, time: Time)
    where
        Receiver: Handler<Message> + Actor<ActorNames>;
    fn send_channel(&mut self, channel: Channel<Message, ActorNames>, message: Message, time: Time);
    fn cancel(&mut self) -> (Time, Message);
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
        union $name {
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
            [(); ActorNames::COUNT]: ,
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
            [(); <$name_type as actor_framework::MakeNamed>::COUNT]: ,
        {
            fn send<Receiver>(&mut self, message: $field_type, time: Time)
            where
                Receiver: actor_framework::Handler<$field_type> + actor_framework::Actor<$name_type>,
            {
                assert!(self.is_empty());

                self.$field_ident = core::mem::ManuallyDrop::new(actor_framework::MessagePacket::new::<
                    <Self as actor_framework::Outbox<$name_type>>::Sender,
                    Receiver>(time, message));
            }

            fn send_addr<Receiver>(&mut self, addr: &actor_framework::Addr<Receiver, $name_type>, message: $field_type, time: Time)
            where
                Receiver: actor_framework::Handler<$field_type> + actor_framework::Actor<$name_type>,
            {
                self.send::<Receiver>(message, time);
            }

            fn send_channel(&mut self, channel: actor_framework::Channel<$field_type, $name_type>, message: $field_type, time: Time)
            {
                self.$field_ident = core::mem::ManuallyDrop::new(channel.send(message, time));
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
        }
    };
    // Call above rule for every field
    (@impl $name:ident<$name_type:ty>, $i:ident : $t:ty, $($tail:tt)+) => {
        actor_framework::make_outbox!(@impl $name<$name_type>, $i : $t);
        actor_framework::make_outbox!(@impl $name<$name_type>, $($tail)+);
    };
}
