use std::sync::mpsc::{self, Receiver, SyncSender};

pub mod cli;

pub trait EmulationCore: Sync + Send {
    /// The name of the core
    fn name(&self) -> &'static str;

    /// Short name of the core (ideally 2-3 chars)
    fn short_name(&self) -> &'static str;

    // Create an instance of the core
    fn new(&self) -> Result<Box<dyn Instance + Send>, anyhow::Error>;

    /// Create a single-threaded instance of the core.
    /// Override the default implementation if your core doesn't implement Send
    fn new_sync(&self) -> Result<Box<dyn Instance>, anyhow::Error> {
        Ok(self.new()?)
    }

    /// Create a multi-threaded instance of the core.
    /// The default implementation calls `new()` and wraps it with `ThreadAdapter`
    fn new_threadded(&self) -> Result<Box<dyn ThreadedInstance>, anyhow::Error> {
        Ok(Box::new(ThreadAdapter::new(self.new()?)?))
    }

    /// Called while running to draw the core's UI
    #[cfg(feature = "ui")]
    fn ui(&self, ui: &mut egui::Ui) {
        // Default implementation: do nothing
        let _ = ui;
    }

    /// Called while paused to allow the core to draw it's UI
    /// The Paused version of the UI has full access to the instance's state
    #[cfg(feature = "ui")]
    fn paused_ui(&self, instance: &mut dyn Instance, ui: &mut egui::Ui) {
        // Default implementation: fall back to the normal UI
        let _ = instance;
        self.ui(ui);
    }
}

pub trait EmulationCoreCli<GlobalOpts>
where
    GlobalOpts: clap::FromArgMatches + clap::Args,
{
    //type Parser : clap::Parser;

    fn parse_args(&self) -> GlobalOpts;

}

/// Messages sent from the core instance to the UI thread
pub enum UpdateMessage {
    /// Signals to the UI thread that the core has a new frame of video ready to be shown
    Vsync,
    /// The instance has finished syncing with the UI thread
    UiSynced,
}

/// Messages sent from the UI thread when the core instance is running
#[derive(Debug)]
pub enum ControlMessage {
    /// The core instance should pause and return from `Instance::run()`
    Pause,
    /// Sent just before the UI is drawn.
    /// The core instance should update any shared data structures needed to draw the UI and respond
    /// with `UpdateMessage::UiSynced`
    UiSync,
}

/// Synchronous instance of an emulator core
pub trait Instance {
    fn run(
        &mut self,
        control_rx: &mpsc::Receiver<ControlMessage>,
        update: mpsc::SyncSender<UpdateMessage>,
    ) -> Result<(), anyhow::Error>;

    fn as_any(&mut self) -> &mut dyn std::any::Any;
}

/// Status of a threaded instance
#[derive(Debug)]
pub enum Status {
    /// The instance is running
    Running,
    /// The instance is paused
    Paused,
    /// The thread has panicked
    Error,
}

// Asynchronous instance of an emulator core
pub trait ThreadedInstance {
    /// Starts the core (non-blocking)
    fn start(&mut self) -> Result<(), anyhow::Error>;
    /// Pauses the core (blocks until paused)
    fn pause(&mut self) -> Result<(), anyhow::Error>;
    /// Draw the UI
    #[cfg(feature = "ui")]
    fn ui(&self, core: &dyn EmulationCore, ui: &mut egui::Ui);
    /// Draw the paused version of the UI
    #[cfg(feature = "ui")]
    fn paused_ui(&mut self, core: &dyn EmulationCore, ui: &mut egui::Ui);

    /// Get the current status of the instance
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

impl ThreadAdapter {
    pub fn new(instance: Box<dyn Instance + Send>) -> Result<Self, anyhow::Error> {
        // Create all our channels
        let (tx_control, rx_control) = mpsc::sync_channel::<ControlMessage>(1);
        let (tx_update, rx_update) = mpsc::sync_channel::<UpdateMessage>(1);
        let (tx_instance, rx_instance) = mpsc::sync_channel::<Box<dyn Instance + Send>>(1);
        let (tx_instance_return, rx_instance_return) =
            mpsc::sync_channel::<Option<Box<dyn Instance + Send>>>(1);

        // Spawn the thread now.
        let join = std::thread::spawn(move || {
            Self::thread_main(rx_instance, tx_instance_return, rx_control, tx_update)
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
        tx_update: SyncSender<UpdateMessage>,
    ) -> Result<(), anyhow::Error> {
        // We don't want to pay the overhead of starting a thread every time we unpause.
        // Instead, we transfer the instance to and from the thread main loop as needed
        loop {
            let mut instance = rx_instance.recv()?;
            let result = instance.run(&rx_control, tx_update.clone());

            match result {
                Ok(_) => {
                    tx_instance
                        .send(Some(instance))
                        .map_err(|_| anyhow::anyhow!("Channel closed"))?;
                }
                Err(e) => {
                    eprintln!("Instance returned error: {:?}", e);
                    tx_instance
                        .send(None)
                        .map_err(|_| anyhow::anyhow!("Channel closed"))?;
                    return Err(e);
                }
            }
        }
    }
}

impl ThreadedInstance for ThreadAdapter {
    fn start(&mut self) -> Result<(), anyhow::Error> {
        match self.instance.take() {
            Some(instance) => self
                .tx_instance
                .send(instance)
                .map_err(|_| anyhow::anyhow!("Channel closed")),
            None => anyhow::bail!("invalid instance state"),
        }
    }
    fn pause(&mut self) -> Result<(), anyhow::Error> {
        self.tx_control.send(ControlMessage::Pause)?;
        self.instance = self.rx_instance_return.recv()?;
        return match self.instance {
            Some(_) => Ok(()),
            None => {
                self.join
                    .take()
                    .expect("invalid instance state")
                    .join()
                    .unwrap()?;
                Err(anyhow::anyhow!("Instance paniced"))
            }
        };
    }
    #[cfg(feature = "ui")]
    fn paused_ui(&mut self, core: &dyn EmulationCore, ui: &mut egui::Ui) {
        match self.instance {
            Some(ref mut instance) => core.paused_ui(instance.as_mut(), ui),
            None => panic!("Instance running or paniced"),
        }
    }

    #[cfg(feature = "ui")]
    fn ui(&self, core: &dyn EmulationCore, ui: &mut egui::Ui) {
        assert!(self.instance.is_none());
        // Sync with instance thread
        if self.tx_control.send(ControlMessage::UiSync).is_ok() {
            loop {
                match self.rx_update.recv() {
                    Ok(UpdateMessage::UiSynced) => break,
                    Ok(_) => continue, // TODO: We probably should be processing these
                    Err(_) => return,  // Channel closed
                }
            }
        }
        core.ui(ui);
    }

    fn status(&self) -> Status {
        match (&self.instance, &self.join) {
            (Some(_), _) => Status::Paused,
            (None, Some(join)) if !join.is_finished() => Status::Running,
            _ => Status::Error,
        }
    }
}
