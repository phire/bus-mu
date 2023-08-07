use crate::{MakeNamed, Named, EnumMap, Actor, MessagePacketProxy, ActorBox, Outbox, ActorBoxBase, actor_box::AsBase};

pub struct ObjectStore<E>
    where
        E: MakeNamed,
        //E::StorageType: Default,
{
    storage: E::StorageType,
}

impl<E> ObjectStore<E> where
    E: MakeNamed,
{
    pub fn new() -> ObjectStore<E>
    where
        E::StorageType: Default,
    {
        Self {
            storage: Default::default()
        }
    }

    pub fn get<'a, 'b, U>(&'a mut self) -> &'b mut ActorBox<E, U>
    where
        U: Named<E> + Sized + Actor<E> + 'b,
        <U as Actor<E>>::OutboxType: Outbox<E>,
        &'b mut ActorBox<E, U>: From<&'a mut E::StorageType>,
        //'a: 'b
    {
        //U::from_storage(&mut self.storage)
        From::from(&mut self.storage)
    }

    pub fn get_base<'a>(&'a mut self, id: E) -> &'a ActorBoxBase<E>
    where
        E::StorageType: AsBase<E>,
    {
        self.storage.as_base(id)
    }
}

// pub struct ActorStore<ActorNames>
// where
//     ActorNames: MakeNamed,
//     <ActorNames as MakeNamed>::Base : Actor<ActorNames>,
//     [(); ActorNames::COUNT]: ,
// {
//     obj_store: ObjectStore<ActorNames>,
//    // outbox_offsets: EnumMap<u32, ActorNames>,
//     outboxes: EnumMap<*mut MessagePacketProxy<ActorNames>, ActorNames>,
// }

// impl<ActorNames> ActorStore<ActorNames> where
//     ActorNames: MakeNamed,
//     <ActorNames as MakeNamed>::Base : Actor<ActorNames>,
//     usize: From<ActorNames>,
//     [(); ActorNames::COUNT]: ,
// {
//     pub fn new() -> ActorStore<ActorNames>
//     {
//         let mut obj_store = ObjectStore::new();

//         let outboxes = EnumMap::from_fn(|actor_id| {
//             let outbox = obj_store.get_id(actor_id).get_message();
//             let outbox_ptr = unsafe {
//                 outbox.get_unchecked_mut() as *mut MessagePacketProxy<ActorNames>
//             };
//             outbox_ptr
//         });

//         // let min_outbox_size = size_of::<MessagePacketProxy<ActorNames>>();

//         // This is an ugly hack to get around the fact that rust doesn't have trait fields
//         // We just force all Actors to implement a get_message method that returns a pointer to the
//         // outbox which must be at the start of the struct.
//         // for actor_id in ActorNames::iter() {
//         //     let actor = obj_store.get_id(actor_id);
//         //     //let actor_ptr = actor as *mut dyn Actor<ActorNames>;
//         //     let actor_size = ActorNames::size_of(actor_id);

//         //     unsafe {
//         //         let (actor_ptr, _) = core::ptr::addr_of_mut!(*actor.get_unchecked_mut()).to_raw_parts();
//         //         let outbox = obj_store.get_id(actor_id).get_message();

//         //         let outbox_ptr = core::ptr::addr_of_mut!(*outbox.get_unchecked_mut()) as usize;
//         //         let outbox_offset = outbox_ptr.checked_sub(actor_ptr as usize) ;

//         //         // Safety: This... is really jank.
//         //         //         But if we check the outbox is inside the actor, it should be safe... I think.
//         //         match outbox_offset {
//         //             Some(offset) if (offset + min_outbox_size) < actor_size => {
//         //                 assert!(offset < u32::MAX as usize);
//         //                 // Compress offsets to 32bits
//         //                 //outbox_offsets[actor_id] = outbox_ptr; // offset as u32;
//         //                 outbox_offsets[actor_id] = offset as u32;
//         //                 outboxes[actor_id] = outbox;
//         //             },
//         //             _ => {
//         //                 eprintln!("{:x} - {:x} = {:x?}; ActorSize: {:x}", outbox_ptr, actor_ptr as usize, outbox_offset, actor_size);
//         //                 panic!("{:?}::get_message() needs to return a refrence to an outbox within the main actor struct - {:x} != {:x}", actor_id, actor_ptr as usize, outbox_ptr as usize);
//         //             }
//         //         }
//         //     }
//         // }

//         ActorStore {
//             obj_store,
//             outboxes,
//         }
//     }

//     pub fn obj_store(&mut self) -> &mut ObjectStore<ActorNames> {
//         &mut self.obj_store
//     }

//     pub fn outbox<'a>(&mut self, id: ActorNames) -> Pin<&'a mut MessagePacketProxy<ActorNames>> {
//         let outbox = self.outboxes[id];
//         unsafe {
//             return Pin::new_unchecked(
//                 outbox.as_mut().unwrap_unchecked()
//             );
//         }
//         // let actor = self.obj_store.get_id(id);
//         // let offset = self.outbox_offsets[id];

//         // // Safety: This is really jank
//         // //         But the checks in new() should make this safe
//         // unsafe {
//         //     let (actor_ptr, _) = core::ptr::addr_of_mut!(*actor.get_unchecked_mut()).to_raw_parts();
//         //     let outbox_ptr = (actor_ptr as usize + offset as usize) as *mut MessagePacketProxy<ActorNames>;
//         //     // let outbox_ptr = offset as *mut MessagePacketProxy<ActorNames>;
//         //     return std::mem::transmute(outbox_ptr);
//         // }
//     }
// }
