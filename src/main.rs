//#![deny(unused)]
//#![deny(deprecated)]

mod midi;
mod app;
mod editor;
mod audio;
mod util;

use std::{panic, sync::Mutex};

use crate::app::main_window::MainWindow;

#[macro_export]
macro_rules! deprecated {
    ($msg:literal) => {{
        panic!("Use of deprecated code: {}", $msg)
    }};
}

static LAST_PANIC: Mutex<Option<String>> = Mutex::new(None);

fn make_panic_hook() {
    panic::set_hook(Box::new(|panic_info| {
        let msg = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        *LAST_PANIC.lock().unwrap() = Some(format!("Panic at {}\n{}", location, msg));
    }));
}

fn main() -> eframe::Result {
    dotenvy::dotenv().ok();
    make_panic_hook();
    
    let mut native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1920.0, 1080.0])
            .with_maximized(true),
        ..Default::default()
    };
    native_options.centered = true;

    //let app_result = std::panic::catch_unwind(|| {
        eframe::run_native("Andromeda", native_options, Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(MainWindow::new(cc)))
        }))
    /* });

    match app_result {
        Ok(o) => {
            o
        },
        Err(e) => {
            // show a window after andromeda unfortunately crashes :(
            rfd::MessageDialog::new()
                .set_buttons(rfd::MessageButtons::Ok)
                .set_title("Andromeda has crashed :c")
                .set_description("A problem has occured and Andromeda needs to shut down. Sorry for the inconvenience. You can send a report to the Discord server if you want.")
                .show();
            Ok(())
        }
    }*/
}