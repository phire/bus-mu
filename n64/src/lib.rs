
pub mod actors;
pub mod vr4300;

pub use actors::N64Actors;

pub type MessagePacket = actor_framework::MessagePacket<N64Actors>;

pub fn new() -> actor_framework::Scheduler<N64Actors> {
    actor_framework::Scheduler::new()
}