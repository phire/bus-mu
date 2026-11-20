use std::{any::Any, cell::RefCell, rc::Rc, sync::Arc};

#[cfg(feature = "dx")]
use dioxus::core::Element;

type Sender<T> = tokio::sync::mpsc::Sender<T>;
type Receiver<T> = tokio::sync::mpsc::Receiver<T>;

pub mod cli;
pub mod util;

pub trait EmulationCore: Sync + Send {
    /// The name of the core
    fn name(&self) -> &'static str;

    /// Short name of the core (ideally 2-3 chars)
    fn short_name(&self) -> &'static str;

    // Create an instance of the core
    fn new(&self, config: Arc<dyn Any>) -> Result<Box<dyn Instance + Send>, anyhow::Error>;

    /// Create a single-threaded instance of the core.
    /// Override the default implementation if your core doesn't implement Send
    fn new_sync(&self, config: Arc<dyn Any>) -> Result<Box<dyn Instance>, anyhow::Error> {
        Ok(self.new(config)?)
    }

    /// Create a multi-threaded instance of the core.
    /// The default implementation calls `new()` and wraps it with `ThreadAdapter`
    fn new_threaded(&self, config: Arc<dyn Any>) -> Result<(Rc<RefCell<ThreadAdapter>>, ThreadUpdates), anyhow::Error> {
        ThreadAdapter::new(self.new(config)?)
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

    #[cfg(feature = "dx")]
    fn ui(&self, instance: Option<&dyn Instance>) -> Element;
}

pub trait EmulationCoreCli<GlobalOpts>
where
    GlobalOpts: clap::FromArgMatches + clap::Args,
{
    //type Parser : clap::Parser;

    fn parse_args(&self, args: Vec<String>) -> (GlobalOpts, Arc<dyn Any + Sync + Send>);

}

/// Messages sent from the core instance to the UI thread
pub enum UpdateMessage {
    /// Signals to the UI thread that the core has a new frame of video ready to be shown
    Vsync,
    /// The instance has finished syncing with the UI thread
    UiSynced,
    /// The instance has started running
    Running,
    /// The instance has paused
    Paused(Box<dyn Instance>),
    /// The instance has encountered an error
    Error(anyhow::Error, Box<dyn Instance>),
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
pub trait Instance: Send {
    fn run(
        &mut self,
        control_rx: &mut Receiver<ControlMessage>,
        update: Sender<UpdateMessage>,
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

/// Takes a raw synchronous Instance and wraps it in a thread
///
/// The instance gets moved to the thread when running and then back to the parent thread when paused.
/// This allows the UI code to have full access to the instance while paused without crossing thread
/// boundaries.
pub struct ThreadAdapter {
    instance: Option<Box<dyn Instance>>,
    tx_control: Sender<ControlMessage>,
    tx_instance: Sender<Box<dyn Instance>>,
    join: Option<std::thread::JoinHandle<Result<(), anyhow::Error>>>,
}

pub struct ThreadUpdates {
    rx_update: Receiver<UpdateMessage>,
    adapter: Rc<RefCell<ThreadAdapter>>,
}

impl ThreadUpdates {
    pub async fn recv_update(&mut self) -> Result<(), anyhow::Error> {
        match self.rx_update.recv().await {
            Some(UpdateMessage::Paused(instance)) => {
                println!("Received paused update from thread.");
                self.adapter.borrow_mut().instance = Some(instance);
            }
            Some(UpdateMessage::Error(err, instance)) => {
                self.adapter.borrow_mut().instance = Some(instance);
                self.adapter.borrow_mut().exit()?;
                return Err(err);
            }
            None => return Err(anyhow::anyhow!("Channel closed")),
            _ => {}
        }
        Ok(())
    }
}

impl ThreadAdapter {
    pub fn new(instance: Box<dyn Instance>) -> Result<(Rc<RefCell<Self>>, ThreadUpdates), anyhow::Error> {
        use tokio::sync::mpsc::channel;
        // Create all our channels
        let (tx_control, rx_control) = channel::<ControlMessage>(1);
        let (tx_update, rx_update) = channel::<UpdateMessage>(1);
        let (tx_instance, rx_instance) = channel::<Box<dyn Instance>>(1);

        // Spawn the thread now.
        let join = std::thread::spawn(move || {
            Self::thread_main(rx_instance, rx_control, tx_update)
        });

        let adapter = std::rc::Rc::new(RefCell::new(Self {
            instance: Some(instance),
            tx_control,
            tx_instance,
            join: Some(join),
        }));

        let updates = ThreadUpdates {
            rx_update,
            adapter: adapter.clone(),
        };

        Ok((adapter, updates))
    }

    fn thread_main<'b>(
        mut rx_instance: Receiver<Box<dyn Instance>>,
        mut rx_control: Receiver<ControlMessage>,
        tx_update: Sender<UpdateMessage>,
    ) -> Result<(), anyhow::Error> {
        // We don't want to pay the overhead of starting a thread every time we unpause.
        // Instead, we transfer the instance to and from the thread main loop as needed
        loop {
            let mut instance = rx_instance.blocking_recv()
                .ok_or_else(|| anyhow::anyhow!("Channel closed"))?;

            tx_update.blocking_send(UpdateMessage::Running).map_err(|_| anyhow::anyhow!("Channel closed"))?;
            let result = instance.run(&mut rx_control, tx_update.clone());

            match result {
                Ok(_) => {
                    tx_update.blocking_send(UpdateMessage::Paused(instance))
                        .map_err(|_| anyhow::anyhow!("Channel closed"))?;
                }
                Err(e) => {
                    tx_update
                        .blocking_send(UpdateMessage::Error(anyhow::anyhow!("Run error: {:?}", e), instance))
                        .map_err(|_| anyhow::anyhow!("Channel closed"))?;
                    return Err(e);
                }
            }
        }
    }

    fn exit(&mut self) -> Result<(), anyhow::Error> {
        self.join
            .take()
            .expect("invalid instance state")
            .join()
            .unwrap()?;
        Ok(())
    }

}


impl ThreadAdapter {
    pub fn start(&mut self) -> Result<(), anyhow::Error> {
        match self.instance.take() {
            Some(instance) => self
                .tx_instance
                .try_send(instance)
                .map_err(|_| anyhow::anyhow!("Channel closed")),
            None => anyhow::bail!("invalid instance state"),
        }
    }

    #[cfg(feature = "ui")]
    pub fn paused_ui(&mut self, core: &dyn EmulationCore, ui: &mut egui::Ui) {
        match self.instance {
            Some(ref mut instance) => core.paused_ui(instance.as_mut(), ui),
            None => panic!("Instance running or paniced"),
        }
    }

    #[cfg(feature = "ui")]
    fn ui(&self, core: &dyn EmulationCore, ui: &mut egui::Ui) {
        assert!(self.instance.is_none());
        // Sync with instance thread
        if self.tx_control.blocking_send(ControlMessage::UiSync).is_ok() {
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

    #[cfg(feature = "dx")]
    pub fn ui(&self, core: &dyn EmulationCore) -> Element {
        core.ui(self.instance.as_deref())
    }

    pub fn status(&self) -> Status {
        match (&self.instance, &self.join) {
            (Some(_), _) => Status::Paused,
            (None, Some(join)) if !join.is_finished() => Status::Running,
            _ => Status::Error,
        }
    }

    pub async fn pause(&mut self) -> Result<(), anyhow::Error> {
        self.tx_control.send(ControlMessage::Pause)
            .await
            .map_err(|_| anyhow::anyhow!("Channel closed"))
    }
}
