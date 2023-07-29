

use crate::{object_map::ObjectStore, MakeNamed, Time};

pub(crate) trait MessagePacketInner<Name> : core::fmt::Debug
where Name: MakeNamed, [(); Name::COUNT]:,
{
    fn execute(self: Box<Self>, map: &mut ObjectStore<Name>, time: Time) -> MessagePacket<Name>;
    fn actor_name(&self) -> Name;
}

#[derive(Debug)]
pub struct MessagePacket<Name> {
    pub time: Time,
    pub(crate) inner: Option<Box<dyn MessagePacketInner<Name>>>,
}

impl<Name> Default for MessagePacket<Name> {
    fn default() -> Self {
        MessagePacket {
            inner: None,
            time: Time::default(),
        }
    }
}

impl<Name> MessagePacket<Name>
    where Name: MakeNamed, [(); Name::COUNT]:,
{
    pub fn no_message(time: Time) -> MessagePacket<Name> {
        MessagePacket {
            inner: None,
            time,
        }
    }

    pub fn is_none(&self) -> bool {
        self.inner.is_none()
    }
}
