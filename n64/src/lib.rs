
pub mod actors;

pub mod cic;
pub mod pif;
mod c_bus;
mod d_bus;

pub use actors::N64Actors;

pub struct CoreN64 { }

impl common::EmulationCore for CoreN64 {
    fn name(&self) -> &'static str { "Nintendo 64" }

    fn new(&self) -> Result<Box<dyn common::Instance + Send>, anyhow::Error> {
        Ok(Box::new(actor_framework::Instance::<N64Actors>::new()))
    }

    #[cfg(feature = "ui")]
    fn paused_ui(&self, instance: &mut dyn common::Instance, ui: &mut egui::Ui) {
        use actors::cpu_actor::CpuActor;

        let instance = instance.as_any().downcast_mut::<actor_framework::Instance<N64Actors>>().unwrap();

        let cpu_core = &mut instance.actor::<CpuActor>().cpu_core;
        cpu_core.ui(ui);
    }
}

pub static CORE_N64 : CoreN64 = CoreN64 { };
