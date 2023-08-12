
fn no_ui_run(cores: Vec<Box<dyn common::Core>>) -> Result<(), anyhow::Error> {
    let com = cores.iter().next().unwrap().create()?;
    com.control.send(common::ControlMessage::MoveTo(common::State::Run))?;

    loop {
        match com.update.recv() {
            Err(_) => { return com.join.join().unwrap(); }
            _ => {}
        }
    }
}

fn main() {
    let cores : Vec<Box<dyn common::Core>> = vec!(
        n64::new(),
    );

    if cfg!(feature = "ui") {
        ui::run(cores);
    } else {
        no_ui_run(cores).err().map(|e| eprintln!("{:?}", e));
    }
}
