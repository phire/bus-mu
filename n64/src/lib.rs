
pub mod actors;

pub mod cic;
pub mod pif;

pub use actors::N64Actors;

pub type CoreN64 = actor_framework::ActorFrameworkCore<N64Actors>;

pub fn new() -> Box<CoreN64> {
    let mut core = CoreN64::new();
    core.set_name("Nintendo 64");

    Box::new(core)
}
