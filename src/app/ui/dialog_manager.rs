use std::{any::Any, collections::{HashMap, VecDeque, hash_map::ValuesMut}};

use crate::app::ui::dialog::{Dialog, DialogAction};

pub type DialogFactory = Box<dyn Fn() -> Box<dyn Dialog + 'static>>;
pub type MaybeDlgAction = Option<DialogAction>;

pub enum DialogOpenResult<E> {
    OpenOK(&'static str),
    OpenError(&'static str, E)
}

pub enum DialogCloseResult<E> {
    CloseOK(&'static str),
    CloseCancelled(&'static str),
    CloseError(&'static str, E)
}

#[derive(Default)]
pub struct DialogManager {
    /// The registry containing ALL dialogs, opened or not
    dialog_registry: HashMap<&'static str, DialogFactory>,
    // hashset that contains all opened dialogs, excluding closed ones
    opened_dialogs: HashMap<&'static str, Box<dyn Dialog>>,
    
    dlg_open_results: VecDeque<DialogOpenResult<String>>,
    dlg_close_results: VecDeque<DialogCloseResult<String>>,

    /// A counter for how many dialogs are open at the same time
    opened_dialog_counter: usize
}

impl DialogManager {
    pub fn new() -> Self {
        Self {
            dialog_registry: HashMap::new(),
            opened_dialogs: HashMap::new(),

            dlg_open_results: VecDeque::new(),
            dlg_close_results: VecDeque::new(),

            opened_dialog_counter: 0
        }
    }

    pub fn register_dialog(&mut self, dialog_name: &'static str, dlg_factory: DialogFactory) {
        self.dialog_registry.insert(dialog_name, dlg_factory);
    }

    pub fn open_dialog_by_name(&mut self, dialog_name: &'static str, args: Vec<Box<dyn Any>>) {
        if let Some(factory) = self.dialog_registry.get(dialog_name) {
            let dlg = factory();
            self.open_dialog(dlg, args);
        } else {
            let m = format!("No such dialog: {}", dialog_name);
            self.push_open_err(dialog_name, m);
        }
    }

    pub fn open_dialog(&mut self, mut dlg: Box<dyn Dialog>, args: Vec<Box<dyn Any>>) {
        let dlg_id = dlg.get_dialog_name();

        if self.opened_dialogs.contains_key(dlg_id) {
            println!("[WARNING] Dialog with ID {} is already open, will close old Dialog", dlg_id);
            self.close_dialog(dlg_id);
        }

        match dlg.init_dialog(args) {
            Ok(_) => {
                self.opened_dialogs.insert(dlg_id, dlg);
                self.opened_dialog_counter += 1;
                self.push_open_ok(dlg_id);
            },

            Err(msg) => {
                self.push_open_err(dlg_id, msg);
            }
        }
    }

    pub fn close_dialog(&mut self, dlg_id: &'static str) {
        if !self.opened_dialogs.contains_key(dlg_id) {
            self.push_close_err(dlg_id, "Dialog was never open");
            return;
        }

        let dialog = self.opened_dialogs.get_mut(dlg_id).unwrap();
        match (*dialog).cleanup_dialog() {
            Ok(_) => {
                self.opened_dialogs.remove(dlg_id);
                self.opened_dialog_counter -= 1;
                self.push_close_ok(dlg_id);
            },
            Err(msg) => {
                self.push_close_err(dlg_id, msg);
            }
        }
    }

    pub fn get_opened_dialogs(&mut self) -> ValuesMut<&'static str, Box<dyn Dialog>> {
        self.opened_dialogs.values_mut()
    }

    pub fn is_any_dialog_shown(&self) -> bool {
        self.opened_dialog_counter > 0
    }

    pub fn close_all_dialogs(&mut self) {
        let mut to_close = Vec::with_capacity(self.opened_dialog_counter);
        for &dlg_id in self.dialog_registry.keys() {
            if self.opened_dialogs.contains_key(dlg_id) {
                to_close.push(dlg_id);
            }
        }

        for dlg_id in to_close.into_iter() {
            self.close_dialog(dlg_id);
        }
    }

    fn push_open_ok(&mut self, dialog_name: &'static str) {
        self.dlg_open_results.push_front(
            DialogOpenResult::OpenOK(dialog_name)
        );
    }

    fn push_open_err(&mut self, dialog_name: &'static str, err_msg: impl Into<String>) {
        self.dlg_open_results.push_front(
            DialogOpenResult::OpenError(dialog_name, err_msg.into())
        );
    }

    fn push_close_ok(&mut self, dialog_name: &'static str) {
        self.dlg_close_results.push_front(
            DialogCloseResult::CloseOK(dialog_name)
        );
    }

    fn push_close_err(&mut self, dialog_name: &'static str, err_msg: impl Into<String>) {
        self.dlg_close_results.push_front(
            DialogCloseResult::CloseError(dialog_name, err_msg.into())
        );
    }
}