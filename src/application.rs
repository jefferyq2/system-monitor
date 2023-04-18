use egui::{Button, Context, Layout, Ui};
use egui_extras::{Column, TableBuilder};
use sysinfo::{CpuExt, Pid, Process, ProcessExt, System, SystemExt};

pub struct Application {
    pub system: System,
    pub search: String,
    current_nav_item: NavItem,
}

pub enum NavItem {
    Processes,
}

impl Application {
    #[must_use]
    pub fn new() -> Self {
        let system = System::new_all();
        let search = String::default();

        Self {
            system,
            search,
            current_nav_item: NavItem::Processes,
        }
    }

    pub fn render(&mut self, ctx: &Context) {
        egui::SidePanel::new(egui::panel::Side::Left, "side-panel")
            .default_width(140.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    if ui
                        .add(Button::new("Processes").min_size(egui::Vec2 {
                            x: ui.available_width(),
                            y: 30.0,
                        }))
                        .clicked()
                    {
                        self.current_nav_item = NavItem::Processes;
                    };
                });
            });
        egui::CentralPanel::default().show(ctx, |ui| match self.current_nav_item {
            NavItem::Processes => self.processes_view(ui),
        });
    }

    fn processes_view(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Type a name or PID to search."),
            );
        });

        {
            let table = TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::auto())
                .column(Column::initial(100.0).range(40.0..=300.0))
                .column(Column::initial(100.0).range(40.0..=300.0))
                .column(Column::initial(100.0).range(40.0..=300.0))
                .column(Column::remainder())
                .min_scrolled_height(0.0);

            table
                .header(40.0, |mut header| {
                    header.col(|ui| {
                        ui.label("Name");
                    });
                    header.col(|ui| {
                        ui.label("PID");
                    });
                    header.col(|ui| {
                        ui.vertical(|ui| {
                            ui.strong(format!("{:.1}%", self.system.global_cpu_info().cpu_usage()));
                            ui.label("CPU");
                        });
                    });
                    header.col(|ui| {
                        ui.vertical(|ui| {
                            ui.strong(format!(
                                "{:.1}%",
                                (self.system.used_memory() as f32
                                    / self.system.total_memory() as f32)
                                    * 100.0
                            ));
                            ui.label("Memory");
                        });
                    });
                    header.col(|ui| {
                        ui.label("Disk");
                    });
                })
                .body(|mut body| {
                    let mut processes = self
                        .system
                        .processes()
                        .iter()
                        .filter(|(pid, process)| {
                            pid.to_string().contains(&self.search)
                                || process
                                    .name()
                                    .to_lowercase()
                                    .contains(&self.search.to_lowercase())
                        })
                        .collect::<Vec<(&Pid, &Process)>>();

                    processes.sort_by(|(_, process_a), (_, process_b)| {
                        process_a
                            .name()
                            .to_lowercase()
                            .cmp(&process_b.name().to_lowercase())
                    });

                    for (pid, process) in &processes {
                        body.row(18.0, |mut row| {
                            row.col(|ui| {
                                ui.label(process.name());
                            });
                            row.col(|ui| {
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(format!("{pid}"));
                                });
                            });
                            row.col(|ui| {
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(format!("{:.1}%", process.cpu_usage()));
                                });
                            });
                            row.col(|ui| {
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(format!("{:.1} MB", process.memory() / 1_000_000));
                                });
                            });
                            row.col(|ui| {
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(format!("{}/s", process.disk_usage().written_bytes));
                                });
                            });
                        });
                    }
                });
        }
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}
