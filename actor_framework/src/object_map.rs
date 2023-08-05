
use std::pin::Pin;

use crate::{MakeNamed, Named, EnumMap, Actor, MessagePacketProxy};

pub struct ObjectStore<E>
    where
        E: MakeNamed,
        [(); E::COUNT]: ,
{
    contents: [Pin<Box<E::Base>>; E::COUNT]
}

impl<E> ObjectStore<E> where
    E: MakeNamed,
    [(); E::COUNT]: ,
{
    pub fn new() -> ObjectStore<E> where usize: From<E>
    {
        ObjectStore { contents: std::array::from_fn(|i| {
            let name = E::from(i);
            let obj = E::make(name);
            // Safety: Make sure that the returned object still reports the same name
            assert!(obj.dyn_name() == name);
            obj
        }) }
    }

    pub fn get<U>(&mut self) -> Pin<&mut U>
        where U: Named<E> + Sized
    {
        let index: usize = U::name().into();
        unsafe {
            // Safety: This is safe, but only if contents[U::id()] is actually a U.
            let obj = &mut self.contents[index];
            let u_obj: &mut Pin<Box<U>> = std::mem::transmute(obj);
            return u_obj.as_mut();
        }
    }
    pub fn get_id(&mut self, id: E) -> Pin<&mut E::Base>
    {
        let index: usize = id.into();
        let obj = self.contents[index].as_mut();
        return obj;
    }
}


pub struct ActorStore<ActorNames>
where
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::Base : Actor<ActorNames>,
    [(); ActorNames::COUNT]: ,
{
    obj_store: ObjectStore<ActorNames>,
   // outbox_offsets: EnumMap<u32, ActorNames>,
    outboxes: EnumMap<*mut MessagePacketProxy<ActorNames>, ActorNames>,
}

impl<ActorNames> ActorStore<ActorNames> where
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::Base : Actor<ActorNames>,
    usize: From<ActorNames>,
    [(); ActorNames::COUNT]: ,
{
    pub fn new() -> ActorStore<ActorNames>
    {
        let mut obj_store = ObjectStore::new();

        let outboxes = EnumMap::from_fn(|actor_id| {
            let outbox = obj_store.get_id(actor_id).get_message();
            let outbox_ptr = unsafe {
                outbox.get_unchecked_mut() as *mut MessagePacketProxy<ActorNames>
            };
            outbox_ptr
        });

        // let min_outbox_size = size_of::<MessagePacketProxy<ActorNames>>();

        // This is an ugly hack to get around the fact that rust doesn't have trait fields
        // We just force all Actors to implement a get_message method that returns a pointer to the
        // outbox which must be at the start of the struct.
        // for actor_id in ActorNames::iter() {
        //     let actor = obj_store.get_id(actor_id);
        //     //let actor_ptr = actor as *mut dyn Actor<ActorNames>;
        //     let actor_size = ActorNames::size_of(actor_id);

        //     unsafe {
        //         let (actor_ptr, _) = core::ptr::addr_of_mut!(*actor.get_unchecked_mut()).to_raw_parts();
        //         let outbox = obj_store.get_id(actor_id).get_message();

        //         let outbox_ptr = core::ptr::addr_of_mut!(*outbox.get_unchecked_mut()) as usize;
        //         let outbox_offset = outbox_ptr.checked_sub(actor_ptr as usize) ;

        //         // Safety: This... is really jank.
        //         //         But if we check the outbox is inside the actor, it should be safe... I think.
        //         match outbox_offset {
        //             Some(offset) if (offset + min_outbox_size) < actor_size => {
        //                 assert!(offset < u32::MAX as usize);
        //                 // Compress offsets to 32bits
        //                 //outbox_offsets[actor_id] = outbox_ptr; // offset as u32;
        //                 outbox_offsets[actor_id] = offset as u32;
        //                 outboxes[actor_id] = outbox;
        //             },
        //             _ => {
        //                 eprintln!("{:x} - {:x} = {:x?}; ActorSize: {:x}", outbox_ptr, actor_ptr as usize, outbox_offset, actor_size);
        //                 panic!("{:?}::get_message() needs to return a refrence to an outbox within the main actor struct - {:x} != {:x}", actor_id, actor_ptr as usize, outbox_ptr as usize);
        //             }
        //         }
        //     }
        // }

        ActorStore {
            obj_store,
            outboxes,
        }
    }

    pub fn obj_store(&mut self) -> &mut ObjectStore<ActorNames> {
        &mut self.obj_store
    }

    pub fn outbox<'a>(&mut self, id: ActorNames) -> Pin<&'a mut MessagePacketProxy<ActorNames>> {
        let outbox = self.outboxes[id];
        unsafe {
            return Pin::new_unchecked(
                outbox.as_mut().unwrap_unchecked()
            );
        }
        // let actor = self.obj_store.get_id(id);
        // let offset = self.outbox_offsets[id];

        // // Safety: This is really jank
        // //         But the checks in new() should make this safe
        // unsafe {
        //     let (actor_ptr, _) = core::ptr::addr_of_mut!(*actor.get_unchecked_mut()).to_raw_parts();
        //     let outbox_ptr = (actor_ptr as usize + offset as usize) as *mut MessagePacketProxy<ActorNames>;
        //     // let outbox_ptr = offset as *mut MessagePacketProxy<ActorNames>;
        //     return std::mem::transmute(outbox_ptr);
        // }
    }
}
