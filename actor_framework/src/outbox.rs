use crate::{MakeNamed, Handler, Actor, Time, Endpoint, MessagePacket, channel::Channel};


pub trait Outbox<ActorNames>
where
    ActorNames: MakeNamed,
{
    type Sender;
}

pub trait OutboxSend<ActorNames, Message> : Outbox<ActorNames>
where
    ActorNames: MakeNamed,
{
    fn send<Receiver>(&mut self, message: Message, time: Time)
    where
        Receiver: Handler<ActorNames, Message> + Actor<ActorNames>
    ;
    fn send_channel<Sender>(&mut self, channel: Channel<ActorNames, Sender, Message>, message: Message, time: Time)
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
        {
            type Sender = $sender;
            // fn as_mut(&mut self) -> &mut actor_framework::MessagePacketProxy<ActorNames> {
            //     unsafe { std::mem::transmute(self) }
            // }
            // fn as_ref(&self) -> &actor_framework::MessagePacketProxy<ActorNames> {
            //     unsafe { std::mem::transmute(self) }
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
            fn send_channel<Sender>(&mut self, channel: actor_framework::Channel<$name_type, Sender, $field_type>, message: $field_type, time: Time)
            where
                Sender: Actor<$name_type>,
                Self: actor_framework::Outbox<$name_type, Sender=Sender>,
            {
                assert!(self.is_empty());

                self.$field_ident = core::mem::ManuallyDrop::new(
                    actor_framework::MessagePacket::from_channel(channel, message, time));
            }

            // fn send_addr<Receiver>(&mut self, addr: &actor_framework::Addr<Receiver, $name_type>, message: $field_type, time: Time)
            // where
            //     Receiver: actor_framework::Handler<$name_type, $field_type> + actor_framework::Actor<$name_type>,
            // {
            //     self.send::<Receiver>(message, time);
            // }

            fn send_endpoint(&mut self, endpoint: actor_framework::Endpoint<$name_type, $field_type>, message: $field_type, time: Time)
            {
                assert!(self.is_empty());

                self.$field_ident = core::mem::ManuallyDrop::new(
                        actor_framework::MessagePacket::from_endpoint::<
                            <Self as actor_framework::Outbox<$name_type>>::Sender>
                    (endpoint, time, message));
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
