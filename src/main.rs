
use std::time::Instant;

fn main() {
    // use multiple runs for stable benchmarks
    let loops = 1;
    for _ in 0..loops {
        let start = Instant::now();

        let result = std::panic::catch_unwind(|| {
            let mut scheduler = n64::new();

            scheduler.run();
        });

        match result {
            Ok(_) => {},
            Err(_) => {
                eprintln!("Scheduler panicked");
            }
        }

        let duration = start.elapsed();
        eprintln!("Execution time {:?}", duration);
    }

}
