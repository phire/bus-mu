use std::sync::mpsc::{self, SyncSender, Receiver};

pub trait EmulationCore {
    fn name(&self) -> &'static str;
    fn new_send(&self) -> Result<Box<dyn Instance + Send>, anyhow::Error>;

    fn new(&self) -> Result<Box<dyn Instance>, anyhow::Error> {
        Ok(self.new_send()?)
    }
    fn new_threadded(&self) -> Result<Box<dyn ThreaddedInstance>, anyhow::Error> {
        Ok(Box::new(ThreadAdapter::new(self.new_send()?)?))
    }

    #[cfg(feature = "ui")]
    fn paused_ui(&self, _instance: &mut dyn Instance, _ctx : egui::Context) { }
}

pub enum UpdateMessage {
    Vsync,
    UiSynced,
}

#[derive(Debug)]
pub enum ControlMessage {
    Pause,
    UiSync,
}

/// Synchronous instance of an emulator core
pub trait Instance : Send {
    fn run(&mut self,
        control_rx: &mpsc::Receiver<ControlMessage>,
        update: mpsc::SyncSender<UpdateMessage>
    ) -> Result<(), anyhow::Error>;

    fn as_any(&mut self) -> &mut dyn std::any::Any;
}

#[derive(Debug)]
pub enum Status {
    Running,
    Paused,
    Error,
}

// Asynchronous instance of an emulator core
pub trait ThreaddedInstance {
    fn start(&mut self) -> Result<(), anyhow::Error>;
    fn pause(&mut self) -> Result<(), anyhow::Error>;
    fn paused_ui(&mut self, core: &dyn EmulationCore, ctx : egui::Context);
    fn status(&self) -> Status;
}

/// Takes a raw synchronous Instance and wraps it in a thread
///
/// The instance gets moved to the thread when running and then back to the parent thread when paused.
/// This allows the UI code to have full access to the instance while paused without crossing thread
/// boundaries.
pub struct ThreadAdapter {
    instance: Option<Box<dyn Instance + Send>>,
    tx_control: SyncSender<ControlMessage>,
    rx_update: Receiver<UpdateMessage>,
    tx_instance: SyncSender<Box<dyn Instance + Send>>,
    rx_instance_return: Receiver<Option<Box<dyn Instance + Send>>>,
    join: Option<std::thread::JoinHandle<Result<(), anyhow::Error>>>,
}

impl ThreadAdapter
{
    pub fn new(instance: Box<dyn Instance + Send>) -> Result<Self, anyhow::Error> {
        // Create all our channels
        let (tx_control, rx_control) = mpsc::sync_channel::<ControlMessage>(1);
        let (tx_update, rx_update) = mpsc::sync_channel::<UpdateMessage>(1);
        let (tx_instance, rx_instance) = mpsc::sync_channel::<Box<dyn Instance + Send>>(1);
        let (tx_instance_return, rx_instance_return) = mpsc::sync_channel::<Option<Box<dyn Instance + Send>>>(1);

        // Span the thread
        let join = std::thread::spawn(move || {
                Self::thread_main(
                    rx_instance,
                    tx_instance_return,
                    rx_control,
                    tx_update)
        });

        Ok(Self {
            instance: Some(instance),
            tx_control,
            rx_update,
            tx_instance,
            rx_instance_return,
            join: Some(join),
        })
    }

    fn thread_main<'b>(
        rx_instance: Receiver<Box<dyn Instance + Send>>,
        tx_instance: SyncSender<Option<Box<dyn Instance + Send>>>,
        rx_control: Receiver<ControlMessage>,
        tx_update: SyncSender<UpdateMessage>
    ) -> Result<(), anyhow::Error> {
        loop {
            let mut instance = rx_instance.recv()?;
            let result = instance.run(&rx_control, tx_update.clone());

            match result {
                Ok(_) => {
                    tx_instance.send(None).map_err(|_| anyhow::anyhow!("Channel closed"))?;
                    return Ok(());
                },
                Err(e) => {
                    eprintln!("Instance returned error: {:?}", e);
                }
            }
        }
    }
}

impl ThreaddedInstance for ThreadAdapter
{
    fn start(&mut self) -> Result<(), anyhow::Error> {
        match self.instance.take() {
            Some(instance) =>
                self.tx_instance.send(instance).map_err(|_| anyhow::anyhow!("Channel closed")),
            None => anyhow::bail!("invalid instance state"),
        }
    }
    fn pause(&mut self) -> Result<(), anyhow::Error> {
        self.tx_control.send(ControlMessage::Pause)?;
        self.instance = self.rx_instance_return.recv()?;
        return match self.instance {
            Some(_) => Ok(()),
            None => {
                self.join.take().expect("invalid instance state").join().unwrap()?;
                Err(anyhow::anyhow!("Instance paniced"))
            },
        }
    }
    fn paused_ui(&mut self, core: &dyn EmulationCore, ctx : egui::Context)
    {
        match self.instance {
            Some(ref mut instance) => core.paused_ui(instance.as_mut(), ctx),
            None => panic!("Instance running or paniced")
        }
    }

    fn status(&self) -> Status {
        if self.instance.is_some() {
            Status::Paused
        } else if self.join.is_some() {
            Status::Paused
        } else {
            Status::Error
        }
    }
}
