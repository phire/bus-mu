use actor_framework::{Actor, Named};

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
}


/*

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum N64Actors {
    CpuActor,
    PifActor,
    SiActor,
    BusActor,
    RspActor,
    PiActor,
    ViActor,
    AiActor,
    RdpActor,
}


impl actor_framework :: Named < N64Actors > for cpu_actor :: CpuActor
{
 #[inline(always)] fn name() -> N64Actors { N64Actors :: CpuActor }
 #[inline(always)] fn dyn_name(& self) -> N64Actors
 { N64Actors :: CpuActor } #[inline(always)] fn from_storage < 'a, 'b >
 (storage : & 'a mut N64ActorsStorage) -> & 'b mut actor_framework ::
 ActorBox < N64Actors, Self > where 'a: 'b { & mut storage.CpuActor }
} impl < 'a > From < & 'a mut N64ActorsStorage > for & 'a mut actor_framework
:: ActorBox < N64Actors, cpu_actor :: CpuActor >
{
 #[inline(always)] fn from(storage : & 'a mut N64ActorsStorage) -> Self
 { & mut storage.CpuActor }
} impl actor_framework :: Named < N64Actors > for pif_actor :: PifActor
{
 #[inline(always)] fn name() -> N64Actors { N64Actors :: PifActor }
 #[inline(always)] fn dyn_name(& self) -> N64Actors
 { N64Actors :: PifActor } #[inline(always)] fn from_storage < 'a, 'b>
 (storage : & 'a mut N64ActorsStorage) -> & 'a mut actor_framework ::
 ActorBox < N64Actors, Self > { & mut storage.PifActor }
} impl < 'a > From < & 'a mut N64ActorsStorage > for & 'a mut actor_framework
:: ActorBox < N64Actors, pif_actor :: PifActor >
{
 #[inline(always)] fn from(storage : & 'a mut N64ActorsStorage) -> Self
 { & mut storage.PifActor }
}

impl actor_framework :: Named < N64Actors > for si_actor :: SiActor
{
 #[inline(always)] fn name() -> N64Actors { N64Actors :: SiActor }
 #[inline(always)] fn dyn_name(& self) -> N64Actors
 { N64Actors :: SiActor }

    // fn from_storage<'a>(storage: &'a mut E::StorageType) -> &'a mut ActorBox<E, Self>
    // where
    // E: MakeNamed,
    // Self: Actor<E> + Sized;

     #[inline(always)]
    fn from_storage<'a>(storage : &'static mut <N64Actors as actor_framework::MakeNamed>::StorageType) -> &'a mut actor_framework::ActorBox<N64Actors, Self>
    where
        N64Actors: actor_framework::MakeNamed,
        Self: Actor<N64Actors> + Sized
     {
        &mut storage.SiActor
    }
} impl < 'a > From < & 'a mut N64ActorsStorage > for & 'a mut actor_framework
:: ActorBox < N64Actors, si_actor :: SiActor >
{
 #[inline(always)] fn from(storage : & 'a mut N64ActorsStorage) -> Self
 { & mut storage.SiActor }
} impl actor_framework :: Named < N64Actors > for bus_actor :: BusActor
{
 #[inline(always)] fn name() -> N64Actors { N64Actors :: BusActor }
 #[inline(always)] fn dyn_name(& self) -> N64Actors
 { N64Actors :: BusActor } #[inline(always)] fn from_storage < 'a >
 (storage : & 'a mut N64ActorsStorage) -> & 'a mut actor_framework ::
 ActorBox < N64Actors, Self > { & mut storage.BusActor }
} impl < 'a > From < & 'a mut N64ActorsStorage > for & 'a mut actor_framework
:: ActorBox < N64Actors, bus_actor :: BusActor >
{
 #[inline(always)] fn from(storage : & 'a mut N64ActorsStorage) -> Self
 { & mut storage.BusActor }
} impl actor_framework :: Named < N64Actors > for rsp_actor :: RspActor
{
 #[inline(always)] fn name() -> N64Actors { N64Actors :: RspActor }
 #[inline(always)] fn dyn_name(& self) -> N64Actors
 { N64Actors :: RspActor } #[inline(always)] fn from_storage < 'a >
 (storage : & 'a mut N64ActorsStorage) -> & 'a mut actor_framework ::
 ActorBox < N64Actors, Self > { & mut storage.RspActor }
} impl < 'a > From < & 'a mut N64ActorsStorage > for & 'a mut actor_framework
:: ActorBox < N64Actors, rsp_actor :: RspActor >
{
 #[inline(always)] fn from(storage : & 'a mut N64ActorsStorage) -> Self
 { & mut storage.RspActor }
} impl actor_framework :: Named < N64Actors > for pi_actor :: PiActor
{
 #[inline(always)] fn name() -> N64Actors { N64Actors :: PiActor }
 #[inline(always)] fn dyn_name(& self) -> N64Actors
 { N64Actors :: PiActor } #[inline(always)] fn from_storage < 'a >
 (storage : & 'a mut N64ActorsStorage) -> & 'a mut actor_framework ::
 ActorBox < N64Actors, Self > { & mut storage.PiActor }
} impl < 'a > From < & 'a mut N64ActorsStorage > for & 'a mut actor_framework
:: ActorBox < N64Actors, pi_actor :: PiActor >
{
 #[inline(always)] fn from(storage : & 'a mut N64ActorsStorage) -> Self
 { & mut storage.PiActor }
} impl actor_framework :: Named < N64Actors > for vi_actor :: ViActor
{
 #[inline(always)] fn name() -> N64Actors { N64Actors :: ViActor }
 #[inline(always)] fn dyn_name(& self) -> N64Actors
 { N64Actors :: ViActor } #[inline(always)] fn from_storage < 'a >
 (storage : & 'a mut N64ActorsStorage) -> & 'a mut actor_framework ::
 ActorBox < N64Actors, Self > { & mut storage.ViActor }
} impl < 'a > From < & 'a mut N64ActorsStorage > for & 'a mut actor_framework
:: ActorBox < N64Actors, vi_actor :: ViActor >
{
 #[inline(always)] fn from(storage : & 'a mut N64ActorsStorage) -> Self
 { & mut storage.ViActor }
} impl actor_framework :: Named < N64Actors > for ai_actor :: AiActor
{
 #[inline(always)] fn name() -> N64Actors { N64Actors :: AiActor }
 #[inline(always)] fn dyn_name(& self) -> N64Actors
 { N64Actors :: AiActor } #[inline(always)] fn from_storage < 'a >
 (storage : & 'a mut N64ActorsStorage) -> & 'a mut actor_framework ::
 ActorBox < N64Actors, Self > { & mut storage.AiActor }
} impl < 'a > From < & 'a mut N64ActorsStorage > for & 'a mut actor_framework
:: ActorBox < N64Actors, ai_actor :: AiActor >
{
 #[inline(always)] fn from(storage : & 'a mut N64ActorsStorage) -> Self
 { & mut storage.AiActor }
} impl actor_framework :: Named < N64Actors > for rdp_actor :: RdpActor
{
 #[inline(always)] fn name() -> N64Actors { N64Actors :: RdpActor }
 #[inline(always)] fn dyn_name(& self) -> N64Actors
 { N64Actors :: RdpActor } #[inline(always)] fn from_storage < 'a >
 (storage : & 'a mut N64ActorsStorage) -> & 'a mut actor_framework ::
 ActorBox < N64Actors, Self > { & mut storage.RdpActor }
} impl < 'a > From < & 'a mut N64ActorsStorage > for & 'a mut actor_framework
:: ActorBox < N64Actors, rdp_actor :: RdpActor >
{
 #[inline(always)] fn from(storage : & 'a mut N64ActorsStorage) -> Self
 { & mut storage.RdpActor }
} impl actor_framework :: MakeNamed for N64Actors
{
 const COUNT : usize = 9usize ; type Base < A > = actor_framework ::
 ActorBox < N64Actors, A > where A : actor_framework :: Actor < N64Actors >
 ; type ExitReason = Box < dyn std :: error :: Error > ; type StorageType =
 N64ActorsStorage ; fn size_of(id : Self) -> usize
 {
     match id
     {
         N64Actors :: CpuActor => core :: mem :: size_of :: < cpu_actor ::
         CpuActor > (), N64Actors :: PifActor => core :: mem :: size_of ::
         < pif_actor :: PifActor > (), N64Actors :: SiActor => core :: mem
         :: size_of :: < si_actor :: SiActor > (), N64Actors :: BusActor =>
         core :: mem :: size_of :: < bus_actor :: BusActor > (), N64Actors
         :: RspActor => core :: mem :: size_of :: < rsp_actor :: RspActor >
         (), N64Actors :: PiActor => core :: mem :: size_of :: < pi_actor
         :: PiActor > (), N64Actors :: ViActor => core :: mem :: size_of ::
         < vi_actor :: ViActor > (), N64Actors :: AiActor => core :: mem ::
         size_of :: < ai_actor :: AiActor > (), N64Actors :: RdpActor =>
         core :: mem :: size_of :: < rdp_actor :: RdpActor > (),
     }
 }
} struct N64ActorsStorage
{
 CpuActor : actor_framework :: ActorBox < N64Actors, cpu_actor :: CpuActor
 >, PifActor : actor_framework :: ActorBox < N64Actors, pif_actor ::
 PifActor >, SiActor : actor_framework :: ActorBox < N64Actors, si_actor ::
 SiActor >, BusActor : actor_framework :: ActorBox < N64Actors, bus_actor
 :: BusActor >, RspActor : actor_framework :: ActorBox < N64Actors,
 rsp_actor :: RspActor >, PiActor : actor_framework :: ActorBox <
 N64Actors, pi_actor :: PiActor >, ViActor : actor_framework :: ActorBox <
 N64Actors, vi_actor :: ViActor >, AiActor : actor_framework :: ActorBox <
 N64Actors, ai_actor :: AiActor >, RdpActor : actor_framework :: ActorBox <
 N64Actors, rdp_actor :: RdpActor >,
} impl actor_framework :: AsBase < N64Actors > for N64ActorsStorage
{
 fn as_base(& self, id : N64Actors) -> & actor_framework :: ActorBoxBase <
 N64Actors >
 {
    unsafe{
     match id
     {
         N64Actors :: CpuActor => std :: mem :: transmute(& self.CpuActor),
         N64Actors :: PifActor => std :: mem :: transmute(& self.PifActor),
         N64Actors :: SiActor => std :: mem :: transmute(& self.SiActor),
         N64Actors :: BusActor => std :: mem :: transmute(& self.BusActor),
         N64Actors :: RspActor => std :: mem :: transmute(& self.RspActor),
         N64Actors :: PiActor => std :: mem :: transmute(& self.PiActor),
         N64Actors :: ViActor => std :: mem :: transmute(& self.ViActor),
         N64Actors :: AiActor => std :: mem :: transmute(& self.AiActor),
         N64Actors :: RdpActor => std :: mem :: transmute(& self.RdpActor),
     }
    }
 }
} impl Default for N64ActorsStorage
{
 fn default() -> Self
 {
     Self
     {
         CpuActor : Default :: default(), PifActor : Default :: default(),
         SiActor : Default :: default(), BusActor : Default :: default(),
         RspActor : Default :: default(), PiActor : Default :: default(),
         ViActor : Default :: default(), AiActor : Default :: default(),
         RdpActor : Default :: default(),
     }
 }
} impl From < N64Actors > for usize
{ #[inline(always)] fn from(id : N64Actors) -> usize { id as usize } } impl
From < usize > for N64Actors
{
 #[inline(always)] fn from(id : usize) -> N64Actors
 {
     match id
     {
         0usize => cpu_actor :: CpuActor :: name(), 1usize => pif_actor ::
         PifActor :: name(), 2usize => si_actor :: SiActor :: name(),
         3usize => bus_actor :: BusActor :: name(), 4usize => rsp_actor ::
         RspActor :: name(), 5usize => pi_actor :: PiActor :: name(),
         6usize => vi_actor :: ViActor :: name(), 7usize => ai_actor ::
         AiActor :: name(), 8usize => rdp_actor :: RdpActor :: name(), _ =>
         { panic! ("invalid id") ; }
     }
 }
}

*/