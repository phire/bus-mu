use actor_framework::{Actor, Named};

pub mod cpu_actor;
pub mod pif_actor;
pub mod si_actor;
pub mod bus_actor;
pub mod rsp_actor;
pub mod pi_actor;
pub mod vi_actor;
pub mod ai_actor;

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
    #[named(class(rsp_actor::RspActor))]
    RspActor,
    #[named(class(pi_actor::PiActor))]
    PiActor,
    #[named(class(vi_actor::ViActor))]
    ViActor,
    #[named(class(ai_actor::AiActor))]
    AiActor,
}
