use egui::{Ui, TextStyle, TextEdit, CollapsingHeader};

use crate::{Core, Pipeline};


impl Core {
    pub fn ui(&mut self, ui: &mut Ui) {

        self.pipeline.ui(ui);
    }
}

impl Pipeline {
    pub fn ui(&mut self, ui: &mut Ui) {
        // Registers
        CollapsingHeader::new("Registers").show(ui, |ui| {
            egui::Grid::new("Registers")
            .min_col_width(160.0)
            .show(ui, |ui| {
            for (i, val) in self.regs.regs.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    if i == 0 {
                        ui.heading("Interger");
                    } else {
                        let id = ui.id().with(i);
                        let name = format!("{}", crate::instructions::MIPS_REG_NAMES[i]);
                        ui.monospace(name);

                         // Persist invalid input values until they are valid
                        let dirty_string: Option<String> = ui.ctx().data(
                            |data| data.get_temp::<String>(id)
                        );
                        let (mut string, dirty, text_color) = match dirty_string {
                            Some(string) => (string, true, ui.visuals().error_fg_color),
                            None => (format!("{:X}", *val), false, ui.visuals().text_color()),
                        };

                        let edit = TextEdit::singleline(&mut string)
                            .horizontal_align(egui::Align::RIGHT)
                            .cursor_at_end(true)
                            .font(TextStyle::Monospace)
                            .text_color(text_color);

                        let response = ui.add(edit);

                        if dirty && response.lost_focus() {
                            if string.is_empty() {
                                // If the user clicks away while the input is empty, reset to the current value
                                ui.ctx().data_mut(|d| d.remove::<String>(id));
                            } else {
                                u64::from_str_radix(&string, 16)
                                    .map(|new_val| {
                                        *val = new_val;
                                        // Clear the dirty string
                                        ui.ctx().data_mut(|d| d.remove::<String>(id));
                                    })
                                    .unwrap_or_default();
                            }
                        }
                        if response.changed() {
                            ui.ctx().data_mut(|d| d.insert_temp::<String>(id, string));
                        }
                    }
                });

                if i % 4 == 3 { // eight rows of four registers
                    ui.end_row();
                }
            }
        });
        });

    }
}
