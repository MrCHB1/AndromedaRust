use eframe::{APP_KEY, egui::RichText};

use crate::{LAST_PANIC, app::ui::dialog::{Dialog, DialogAction, DialogActionButtons, flags::DIALOG_NO_COLLAPSABLE, names::DIALOG_NAME_CRASH}, util::{debugger::Debugger, send_discord_webhook_crash_message}};

#[derive(Default)]
pub struct CrashDialog {
    msg: String,
    user_crash_name: String,
    user_crash_details: String
}

const API_KEY: &str = include_str!("../../../../api_key.txt");

impl Dialog for CrashDialog {
    fn init_dialog(&mut self, _: Vec<Box<dyn std::any::Any>>) -> Result<(), &'static str> {
        let msg = LAST_PANIC
            .lock()
            .unwrap()
            .take()
            .unwrap_or_else(|| "Unknown panic".to_string());
        self.msg = msg;
        self.user_crash_details = String::new();
        Ok(())
    }

    fn draw(&mut self, ui: &mut eframe::egui::Ui, _: &crate::app::util::image_loader::ImageResources) -> Option<crate::app::ui::dialog::DialogAction> {
        ui.vertical(|ui| {
            ui.label("A problem has occured and Andromeda needs to shut down. Sorry for the inconvenience. A report will automatically be sent to developers.");
            ui.separator();

            ui.label(RichText::new("Details").size(15.0));
            ui.label(RichText::new(format!("{}", self.msg)).code());
            ui.separator();

            ui.label("Optionally, specify what caused the crash in the field(s) below.");
            ui.label("Name");
            ui.text_edit_singleline(&mut self.user_crash_name);
            ui.label("How did Andromeda crash?");
            ui.text_edit_multiline(&mut self.user_crash_details);
        });
        None
    }

    fn get_dialog_name(&self) -> &'static str {
        DIALOG_NAME_CRASH
    }

    fn get_dialog_title(&self) -> String {
        "Catastrophic Error".into()
    }

    fn get_action_buttons(&self) -> Option<crate::app::ui::dialog::DialogActionButtons> {
        Some(DialogActionButtons::Ok(
            Box::new(|dlg| {
                let dlg = dlg.as_any().downcast_ref::<Self>().unwrap();
                Debugger::log(format!("{}", dlg.msg));
                
                send_discord_webhook_crash_message(
                    "https://nonconvertibly-untrue-denise.ngrok-free.dev/send",
                    &dlg.msg,
                    API_KEY,
                    if dlg.user_crash_name.is_empty() { None } else { Some(dlg.user_crash_name.clone()) },
                    if dlg.user_crash_details.is_empty() { None } else { Some(dlg.user_crash_details.clone()) }
                )
                .unwrap();

                Some(DialogAction::TerminateApp)
            })
        ))
    }

    fn get_flags(&self) -> u16 {
        DIALOG_NO_COLLAPSABLE
    }
}