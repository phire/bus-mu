use common::{EmulationCore, Status};
use eframe::egui;


struct BusMuApp {
    active_core: Option<&'static dyn common::EmulationCore>,
    instance: Option<Box<dyn common::ThreadedInstance>>,
}

impl BusMuApp {
    fn new(_cc: &eframe::CreationContext<'_>, core: &'static dyn EmulationCore) -> Result<Self, anyhow::Error> {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        let instance = core.new_threadded()?;
        Ok(Self {
            active_core: Some(core),
            instance: Some(instance),
        })
    }
}

impl eframe::App for BusMuApp {
   fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
       egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(core) = self.active_core {
                match &mut self.instance {
                    Some(instance) => {
                        ui.heading(format!("{} is {:?}", core.name(), instance.status()));
                        match instance.status() {
                            Status::Paused => {
                                let responce = ui.button("Resume");
                                ui.separator();
                                instance.paused_ui(core, ui);

                                if responce.clicked() {
                                    instance.start().unwrap();
                                }
                            }
                            Status::Running => {
                                if ui.button("Pause").clicked() {
                                    instance.pause().unwrap();
                                }
                            }
                            Status::Error => {
                                ui.heading("Instance paniced");
                            }
                        }
                    }
                    None => {
                        ui.heading(format!("{} is stopped", core.name()));
                    }
                }
            }
       });
   }
}

pub fn run(core: Option<&'static dyn EmulationCore>, cores: Vec<&'static dyn EmulationCore>) -> Result<(), anyhow::Error> {
    let core = match core {
        Some(core) => core,
        None => {
            // TODO: support dynamically selecting between cores
            cores.into_iter().next().unwrap()
        }
    };

    let native_options = eframe::NativeOptions::default();
    let result = eframe::run_native(
        "Bus-mu",
        native_options,
        Box::new(|cc| Box::new(BusMuApp::new(cc, core).unwrap()))
    );
    result.map_err(|e| anyhow::anyhow!("eframe error: {:?}", e))
}
