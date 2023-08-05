
use crate::{object_map::{ObjectStore, ActorStore}, Time, Actor, MakeNamed};

pub struct Scheduler<ActorNames> where
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::Base: Actor<ActorNames>,
    usize: From<ActorNames>,
    [(); ActorNames::COUNT]:
{
    actors: ActorStore<ActorNames>,
    count: u64,
    zero_limit_count: u64,
}

impl<ActorNames> Drop for Scheduler<ActorNames>
where
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::Base: Actor<ActorNames>,
    usize: From<ActorNames>,
    [(); ActorNames::COUNT]:
{
    fn drop(&mut self) {
        eprintln!("Scheduler ran {} times", self.count);
        eprintln!(" with {} zero limits", self.zero_limit_count);
    }
}

impl<ActorNames> Scheduler<ActorNames> where
ActorNames: MakeNamed,
    usize: From<ActorNames>,
    <ActorNames as MakeNamed>::Base: Actor<ActorNames>,
    [(); ActorNames::COUNT]:,
 {
    pub fn new() -> Scheduler<ActorNames> {
        let actors = ActorStore::new();
        Scheduler {
            actors,
            count: 0,
            zero_limit_count: 0,
        }
    }

    fn find_next(&mut self) -> (ActorNames, Time, Time) {
        let mut min = Time::MAX;
        let mut min_actor = 0.into();
        let mut limit = Time::MAX;

        // PERF: I'm hoping this should compile down into SIMD optimized branchless code
        //       But that might require moving away from trait objects
        for actor in ActorNames::iter() {
            let message = self.actors.outbox(actor);

            let time = message.time.lower_bound();
            if time < min {
                (limit, min, min_actor) = (min, time, actor);
            } else {
                limit = std::cmp::min(limit, time);
            }
        }
        debug_assert!(min != Time::MAX);
        return (min_actor, min, limit)
    }

    fn run_inner(&mut self, sender_id: ActorNames, limit: Time) -> SchedulerResult {
        self.count += 1;

         // PERF: Going though this trait object is potentially problematic for performance.
        //        Might need to look into Arbitrary Self Types or another nasty unsafe hack
        let message = self.actors.outbox(sender_id);

        //println!("Running actor {:?} at time {:?}", sender_id, message.time);

        (message.execute_fn)(sender_id, self.actors.obj_store(), limit)
    }

    #[inline(never)]
    pub fn run(&mut self) -> Box<dyn std::error::Error> {
        loop {
            let (sender_id, time, limit) = self.find_next();

            match self.run_inner(sender_id, limit) {
                SchedulerResult::Ok => {
                    // Hot path
                    continue;
                },
                SchedulerResult::ZeroLimit => {
                    // There are multiple messages scheduled to be delivered on the same cycle.
                    // And one of the receivers couldn't deal with the zero limit message, so we switch
                    // to a more complex scheduler until the current cycle finishes.
                    if let Some(exit_reason) = self.run_zero_limit(time, limit) {
                        return exit_reason;
                    }
                    continue;
                },
                SchedulerResult::Exit(reason) => {
                    return reason;
                }
            }
        }
    }

    #[inline(never)]
    pub fn run_zero_limit(&mut self, time: Time, limit: Time) -> Option<Box<dyn std::error::Error>> {
        assert!(time == limit, "Actor incorrectly reported a zero limit");

        // We might need to go though multiple iterations before this settles
        for _ in 0..(ActorNames::COUNT * 3) {
            self.zero_limit_count += 1;
            for actor in ActorNames::iter() {
                let message = self.actors.outbox(actor);

                if message.time.lower_bound() == time {
                    let result = self.run_inner(actor, time);
                    match result {
                        SchedulerResult::Ok | SchedulerResult::ZeroLimit => {},
                        SchedulerResult::Exit(reason) => {
                            return Some(reason);
                        }
                    }
                }
            }

            let (_, time, limit) = self.find_next();
            if time != limit {
                return None;
            }
        }
        panic!("Zero limit cycle detected");
    }
}

#[derive(Debug)]
pub enum SchedulerResult
{
    Ok,
    ZeroLimit,
    Exit(Box<dyn std::error::Error>)
}
