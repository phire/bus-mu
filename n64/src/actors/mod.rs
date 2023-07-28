use actor_framework::{Actor, Named};

pub mod cpu_actor;
pub mod pif_actor;
pub mod si_actor;
pub mod bus_actor;

#[derive(Named, PartialEq, Eq, Copy, Clone, Debug)]
#[named(base(Actor))]
pub enum N64Actors {
    #[named(class(cpu_actor::CpuActor))]
    CpuActor,
    #[named(class(pif_actor::PifActor))]
    PifActor,
    #[named(class(si_actor::SiActor))]
    SiActor,
    #[named(class(bus_actor::BusActor))]
    BusActor,
}
