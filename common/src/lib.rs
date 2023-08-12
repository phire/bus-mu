use std::sync::mpsc;

pub trait CoreInstance {
    fn start(&mut self);
}

#[derive(Debug)]
pub enum State {
    Run,
    Pause,
}

pub enum UpdateMessage {
    MovedTo(State),
    Vsync,
    UiFinished,
}

pub enum ControlMessage {
    MoveTo(State),
    #[cfg(feature = "ui")]
    DoUi(egui::Context),
    Exit,
}

impl<'a> core::fmt::Debug for ControlMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::MoveTo(arg0) => f.debug_tuple("MoveTo").field(arg0).finish(),
            #[cfg(feature = "ui")]
            Self::DoUi(_) => write!(f, "DoUi"),
            Self::Exit => write!(f, "Exit"),
        }
    }
}

pub struct CoreCommunication {
    pub control: mpsc::SyncSender<ControlMessage>,
    pub update: mpsc::Receiver<UpdateMessage>,
    pub join: std::thread::JoinHandle<Result<(), anyhow::Error>>,
}

pub trait Core {
    fn name(&self) -> &'static str;
    fn create(&self) -> Result<CoreCommunication, anyhow::Error>;
}
