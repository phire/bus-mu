
pub mod actors;

pub mod cic;
pub mod pif;
pub mod vi;
mod c_bus;
mod d_bus;

use std::{path::PathBuf, any::Any};

pub use actors::N64Actors;
use clap::{Parser, FromArgMatches, Args};

pub struct CoreN64;

impl common::EmulationCore for CoreN64 {
    fn name(&self) -> &'static str { "Nintendo 64" }
    fn short_name(&self) -> &'static str { "n64" }

    fn new(&self, config: Box<dyn Any>) -> Result<Box<dyn common::Instance + Send>, anyhow::Error> {
        let config = config.downcast::<N64Config>().unwrap();
        Ok(Box::new(actor_framework::Instance::<N64Actors>::new(*config)?))
    }

    #[cfg(feature = "ui")]
    fn paused_ui(&self, instance: &mut dyn common::Instance, ui: &mut egui::Ui) {
        use actors::cpu_actor::CpuActor;

        let instance = instance.as_any().downcast_mut::<actor_framework::Instance<N64Actors>>().unwrap();

        let cpu_core = &mut instance.actor::<CpuActor>().cpu_core;
        cpu_core.ui(ui);
    }
}

impl<GlobalOpts> common::EmulationCoreCli<GlobalOpts> for CoreN64
where
    GlobalOpts: FromArgMatches + Args,
{
    fn parse_args(&self) -> (GlobalOpts, Box<dyn Any>)
    {
        let cli = Cli::<GlobalOpts>::parse();
        (cli.global_opts, Box::new(cli.n64_config))
    }
}

#[derive(Debug, Args, Clone)]
pub struct N64Config {
    rom: Option<PathBuf>,

    #[arg(long, default_value = "pifdata.bin")]
    #[clap(next_help_heading = "N64 Core Options")]
    pif_data: PathBuf,
}

#[derive(Debug, Parser)]
struct Cli<GlobalOpts>
where
    GlobalOpts: FromArgMatches + Args
{
    #[clap(flatten)]
    n64_config: N64Config,

    #[clap(flatten)]
    global_opts: GlobalOpts,
}
