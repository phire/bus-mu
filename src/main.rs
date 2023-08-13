use std::sync::mpsc;

#[allow(unused)]
fn run_no_ui(cores: Vec<&dyn common::EmulationCore>) -> Result<(), anyhow::Error> {
    let mut instance = cores.iter().next().unwrap().new_sync()?;
    let (_tx_control, rx_control) = mpsc::channel::<common::ControlMessage>();
    let (tx_update, _rx_update) = mpsc::sync_channel::<common::UpdateMessage>(1);

    instance.run(&rx_control, tx_update)
}

#[cfg(not(feature = "ui"))]
fn run(cores: Vec<&'static dyn common::EmulationCore>) -> Result<(), anyhow::Error> {
    run_no_ui(cores)
}

#[cfg(feature = "ui")]
fn run(cores: Vec<&'static dyn common::EmulationCore>) -> Result<(), anyhow::Error> {
    ui::run(cores)
}

fn main() {
    let cores : Vec<&dyn common::EmulationCore> = vec!(
        &n64::CORE_N64,
    );

    run(cores).err().map(|e| eprintln!("{:?}", e));
}
