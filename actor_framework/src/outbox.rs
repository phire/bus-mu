use crate::{MakeNamed, Handler, Actor, Time, Endpoint, MessagePacket, channel::Channel, SchedulerResult};


pub trait Outbox<ActorNames>
where
    ActorNames: MakeNamed,
{
    type Sender: Actor<ActorNames>;
    fn time(&self) -> Time;
    //fn as_proxy<'a>(&'a mut self) -> &'a mut MessagePacketProxy<ActorNames>;
}

pub trait OutboxSend<ActorNames, Message>
where
    ActorNames: MakeNamed,
{
    fn send<Receiver>(&mut self, message: Message, time: Time) -> SchedulerResult
    where
        Receiver: Handler<ActorNames, Message> + Actor<ActorNames>
    ;
    fn send_channel<Sender>(&mut self, channel: Channel<ActorNames, Sender, Message>, message: Message, time: Time) -> SchedulerResult
    where
        Sender: Actor<ActorNames>,
        Self: Outbox<ActorNames, Sender=Sender>,
    ;
    fn send_endpoint(&mut self, endpoint: Endpoint<ActorNames, Message>, message: Message, time: Time);
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
            $( $field_ident:ident : $field_type:ty ),*
            // match optional trailing commas
            $(,)?
        }
    ) => {
        pub union $name {
            none: core::mem::ManuallyDrop<actor_framework::MessagePacket<$name_type, ()>>,
            $($field_ident : core::mem::ManuallyDrop<actor_framework::MessagePacket<$name_type, $field_type>>),*
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
                $(else if msg_type == std::any::TypeId::of::<$field_type>() {
                    std::any::type_name::<$field_type>()
                })*
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
                $(else if msg_type == std::any::TypeId::of::<$field_type>() {
                    unsafe { core::mem::ManuallyDrop::drop( &mut self.$field_ident) };
                })*
            }
        }

        impl actor_framework::Outbox<$name_type> for $name
        {
            type Sender = $sender;
            fn time(&self) -> actor_framework::Time {
                unsafe { self.none.time }
            }
            // fn as_proxy<'a>(&'a mut self) -> &'a mut actor_framework::MessagePacketProxy<ActorNames> {
            //     unsafe { core::mem::transmute(&mut self.none) }
            // }
        }

        impl core::default::Default for $name {
            fn default() -> Self {
                Self {
                    none : Default::default(),
                }
            }
        }

        // Create all OutboxSend<MessageType> traits
        $(
            impl actor_framework::OutboxSend<$name_type, $field_type> for $name
            where
                $name_type: actor_framework::MakeNamed,
            {
                #[inline(always)]
                fn send<Receiver>(&mut self, message: $field_type, time: actor_framework::Time) -> SchedulerResult
                where
                    Receiver: actor_framework::Handler<$name_type, $field_type> + actor_framework::Actor<$name_type>
                {
                    assert!(self.is_empty());

                    self.$field_ident = core::mem::ManuallyDrop::new(actor_framework::MessagePacket::new::<
                        <Self as actor_framework::Outbox<$name_type>>::Sender,
                        Receiver>(time, message));
                    SchedulerResult::Ok
                }

                #[inline(always)]
                fn send_channel<Sender>(&mut self, channel: actor_framework::Channel<$name_type, Sender, $field_type>, message: $field_type, time: actor_framework::Time) -> SchedulerResult
                where
                    Sender: actor_framework::Actor<$name_type>,
                    Self: actor_framework::Outbox<$name_type, Sender=Sender>,
                {
                    assert!(self.is_empty());

                    self.$field_ident = core::mem::ManuallyDrop::new(
                        actor_framework::MessagePacket::from_channel(channel, message, time));
                    SchedulerResult::Ok
                }

                #[inline(always)]
                fn send_endpoint(&mut self, endpoint: actor_framework::Endpoint<$name_type, $field_type>, message: $field_type, time: actor_framework::Time)
                {
                    assert!(self.is_empty());

                    self.$field_ident = core::mem::ManuallyDrop::new(
                            actor_framework::MessagePacket::from_endpoint::<
                                <Self as actor_framework::Outbox<$name_type>>::Sender>
                        (endpoint, time, message));
                }

                #[inline(always)]
                fn cancel(&mut self) -> (actor_framework::Time, $field_type)
                {
                    let msg_type = unsafe { self.none.msg_type() };
                    if msg_type == std::any::TypeId::of::<$field_type>() {
                        unsafe {
                            return self.$field_ident.take().unwrap_unchecked();
                        }
                    } else {
                        let typename = std::any::type_name::<$field_type>();
                        panic!("Outbox::cancel - Expected {} but found {:?}", typename, msg_type);
                    }
                }

                #[inline(always)]
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
        )*
    };
}
