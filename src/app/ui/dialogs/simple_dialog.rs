use crate::app::ui::dialog::{Dialog, DialogAction, DialogActionButtons, dialog_default_close_action, names::DIALOG_NAME_SIMPLE};

#[derive(Default)]
pub struct SimpleDialog {
    title: String,
    msg: String,
    pub id: String,
    pub ok_clicked: bool,
    is_yesno: bool
}

impl Dialog for SimpleDialog {
    fn init_dialog(&mut self, args: Vec<Box<dyn std::any::Any>>) -> Result<(), &'static str> {
        self.title = args[0].downcast_ref::<String>().unwrap().clone();
        self.msg = args[1].downcast_ref::<String>().unwrap().clone();
        self.id = args.get(2).expect("Simple dialog requires a unique ID.").downcast_ref::<String>().unwrap().clone();
        self.is_yesno = if args.len() != 4 { true } else { *(args[3].downcast_ref::<bool>().unwrap()) };
        Ok(())
    }

    fn draw(&mut self, ui: &mut eframe::egui::Ui, _: &crate::app::util::image_loader::ImageResources) -> Option<crate::app::ui::dialog::DialogAction> {
        ui.vertical(|ui| {
            ui.label(&self.msg);
        });

        None
    }

    fn get_dialog_name(&self) -> &'static str {
        DIALOG_NAME_SIMPLE
    }

    fn get_dialog_title(&self) -> String {
        self.title.clone()
    }

    fn get_action_buttons(&self) -> Option<crate::app::ui::dialog::DialogActionButtons> {
        Some(
            if self.is_yesno {
                DialogActionButtons::YesNo(
                    Box::new(|dlg| {
                        let dlg = dlg.as_any_mut().downcast_mut::<Self>().unwrap();
                        dlg.ok_clicked = true;
                        let dlg_name = dlg.get_dialog_name();
                        Some(DialogAction::Close(dlg_name))
                    }),
                    dialog_default_close_action()
                )
            } else {
                DialogActionButtons::Ok(
                    dialog_default_close_action()
                )
            }
        )
    }
}