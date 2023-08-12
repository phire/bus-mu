use common::{State, Core, ControlMessage};
use eframe::egui;


struct BusMuApp {
    active_core: Option<Box<dyn common::Core>>,
    core_instance: common::CoreCommunication,
    core_state: common::State,
}

impl BusMuApp {
    fn new(_cc: &eframe::CreationContext<'_>, core: Box<dyn Core>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        let core_instance = core.create().unwrap();
        Self {
            active_core: Some(core),
            core_instance,
            core_state: common::State::Pause,
        }
    }
}

impl eframe::App for BusMuApp {
   fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
       egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(core) = &self.active_core {
                    let (current, change) = match self.core_state {
                        State::Run => ("Running", State::Pause),
                        State::Pause => ("Paused", State::Run),
                    };

                ui.heading(format!("{} is {}", core.name(), current));
                if ui.button(format!("{:?}", change)).clicked() {
                    self.core_instance.control.send(ControlMessage::MoveTo(change)).unwrap();
                }
            }
       });
   }
}

pub fn run(cores: Vec<Box<dyn common::Core>>) {
    // TODO: support dynamically selecting between cores
    let core = cores.into_iter().next().unwrap();

    let native_options = eframe::NativeOptions::default();
    let result = eframe::run_native(
        "Bus-mu",
        native_options,
        Box::new(|cc| Box::new(BusMuApp::new(cc, core)))
    );
    result.unwrap();
}
