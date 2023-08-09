use actor_framework::Named;

pub mod cpu_actor;
pub mod pif_actor;
pub mod si_actor;
pub mod bus_actor;
pub mod rsp_actor;
pub mod pi_actor;
pub mod vi_actor;
pub mod ai_actor;
pub mod rdp_actor;

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
#[derive(Named)]
#[named(base(actor_framework::ActorBox), exit_reason(std::error::Error))]
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
    #[named(class(rdp_actor::RdpActor))]
    RdpActor,
    #[named(class(Dummy1))]
    Dummy1,
    #[named(class(Dummy2))]
    Dummy2,
    #[named(class(Dummy3))]
    Dummy3,
    #[named(class(Dummy4))]
    Dummy4,
    #[named(class(Dummy5))]
    Dummy5,
    #[named(class(Dummy6))]
    Dummy6,
    #[named(class(Dummy7))]
    Dummy7,
    #[named(class(Dummy8))]
    Dummy8,
    #[named(class(Dummy9))]
    Dummy9,
    #[named(class(Dummy10))]
    Dummy10,
    #[named(class(Dummy11))]
    Dummy11,
}


use actor_framework::Actor;

macro_rules! dummy_actor {
    ($name:ident, $outbox:ident) => {
        #[derive(Default)]
        pub struct $name {}

        actor_framework::make_outbox!(
            $outbox<N64Actors, $name> {
                foo: u32,
            }
        );

        impl Actor<N64Actors> for $name {
            type OutboxType = $outbox;
        }
    }
}

dummy_actor!(Dummy1, Outbox1);
dummy_actor!(Dummy2, Outbox2);
dummy_actor!(Dummy3, Outbox3);
dummy_actor!(Dummy4, Outbox4);
dummy_actor!(Dummy5, Outbox5);
dummy_actor!(Dummy6, Outbox6);
dummy_actor!(Dummy7, Outbox7);
dummy_actor!(Dummy8, Outbox8);
dummy_actor!(Dummy9, Outbox9);
dummy_actor!(Dummy10, Outbox10);
dummy_actor!(Dummy11, Outbox11);
