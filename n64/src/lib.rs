
pub mod actors;

pub mod cic;
pub mod pif;

pub use actors::N64Actors;

pub struct CoreN64 { }

impl common::EmulationCore for CoreN64 {
    fn name(&self) -> &'static str { "Nintendo 64" }

    fn new_send(&self) -> Result<Box<dyn common::Instance + Send>, anyhow::Error> {
        Ok(Box::new(actor_framework::Instance::<N64Actors>::new()))
    }
}

pub static CORE_N64 : CoreN64 = CoreN64 { };
