//#![deny(unused)]
//#![deny(deprecated)]

mod midi;
mod app;
mod editor;
mod audio;

use crate::app::main_window::MainWindow;

fn main() -> eframe::Result {
    let mut native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1920.0, 1080.0]),
        ..Default::default()
    };
    native_options.centered = true;

    eframe::run_native("Andromeda", native_options, Box::new(|cc| {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        Ok(Box::new(MainWindow::new(cc)))
    }))
}