#![feature(generic_const_exprs)]

mod messaging;
mod scheduler;
mod time;
mod object_map;

pub use object_map::{Named, MakeNamed};
pub use named_derive::Named;
pub use time::Time;
pub use messaging::MessagePacket;
pub use scheduler::Scheduler;

pub trait Actor<ActorNames> : Named<ActorNames> {
    fn advance(&self, limit: Time) -> messaging::MessagePacket<ActorNames>;
}

trait Handler<M, Name> where Self: Named<Name> {
    fn recv(&mut self, message: M) -> messaging::MessagePacket<Name>;
}
