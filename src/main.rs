use std::{sync::mpsc, any::Any};

use n64::CoreN64;
use common::{register_cores, cli::GlobalOpts};

register_cores!(
    CoreN64,
);

fn main() -> Result<(), anyhow::Error> {
    use clap::ValueEnum;
    use clap::Parser;

    let all_cores = Cores::value_variants().iter().map(|core| {
        get_core(*core)
    }).collect();

    let core = FindCore::parse().core;
    let (global_opts, config) = parse_args_with::<GlobalOpts<Cores>>(core);

    let core = global_opts.core.map(|c| get_core(c));

    if global_opts.nogui || cfg!(not(feature = "ui")) {
        if let Some(core) = core {
            run_no_ui(core, config, global_opts)
        } else {
            Err(anyhow::anyhow!("--nogui requires a core"))
        }
    } else {
        #[cfg(feature = "ui")]
        ui::run(core, config, all_cores)
    }
}

fn run_no_ui(core: &dyn common::EmulationCore, config: Box<dyn Any>, _opts: GlobalOpts::<Cores>) -> Result<(), anyhow::Error> {
    let mut instance = core.new_sync(config)?;
    let (_tx_control, rx_control) = mpsc::channel::<common::ControlMessage>();
    let (tx_update, _rx_update) = mpsc::sync_channel::<common::UpdateMessage>(1);

    instance.run(&rx_control, tx_update)
}
