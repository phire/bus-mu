
use crate::{object_map::ObjectStore, Time, Actor, MakeNamed};

pub struct Scheduler<ActorNames> where
    ActorNames: MakeNamed,
    usize: From<ActorNames>,
    [(); ActorNames::COUNT]:
{
    actors: ObjectStore<ActorNames>,
}

impl<ActorNames> Scheduler<ActorNames> where
ActorNames: MakeNamed,
    usize: From<ActorNames>,
    <ActorNames as MakeNamed>::Base: Actor<ActorNames>,
    [(); ActorNames::COUNT]:
 {
    pub fn new() -> Scheduler<ActorNames> {
        Scheduler {
            actors: ObjectStore::new(),
        }
    }

    fn find_next(&mut self) -> (ActorNames, Time) {
        let mut min = Time::MAX;
        let mut min_actor = 0.into();
        let mut limit = Time::MAX;

        // PERF: I'm hoping this should compile down into SIMD optimized branchless code
        //       But that might require moving away from trait objects
        for actor in ActorNames::iter() {
            let message = self.actors.get_id(actor).get_message();

            let time = message.time.lower_bound();
            if time < min {
                (limit, min, min_actor) = (min, time, actor);
            } else {
                limit = std::cmp::min(limit, time);
            }
        }
        debug_assert!(min != Time::MAX);
        return (min_actor, limit)
    }

    pub fn run(&mut self) {
        loop {
            let (sender_id, limit) = self.find_next();

            // PERF: Going though this trait object is potentially problematic for performance.
           //        Might need to look into Arbitrary Self Types or another nasty unsafe hack
            let message = &mut self.actors.get_id(sender_id).get_message();
            let execute_fn = message.execute_fn.expect("Execute fn missing");

            (execute_fn)(sender_id, &mut self.actors, limit);
        }
    }
}

