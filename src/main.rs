use std::sync::mpsc;


fn no_ui_run(cores: Vec<&dyn common::EmulationCore>) -> Result<(), anyhow::Error> {
    let mut instance = cores.iter().next().unwrap().new_sync()?;
    let (_tx_control, rx_control) = mpsc::channel::<common::ControlMessage>();
    let (tx_update, _rx_update) = mpsc::sync_channel::<common::UpdateMessage>(1);

    instance.run(&rx_control, tx_update)
}

fn main() {
    let cores : Vec<&dyn common::EmulationCore> = vec!(
        &n64::CORE_N64,
    );

    if cfg!(feature = "ui") {
        ui::run(cores);
    } else {
        no_ui_run(cores).err().map(|e| eprintln!("{:?}", e));
    }
}
