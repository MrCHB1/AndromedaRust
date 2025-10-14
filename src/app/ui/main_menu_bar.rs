use std::rc::Rc;

use eframe::egui::{self, Ui};
use crate::{app::{main_window::MainWindow, util::image_loader::ImageResources}};

pub enum MenuItem {
    MenuButton(Option<Box<dyn FnMut(&mut MainWindow)>>), // just a regular button
    MenuButtonEnabled(Option<Box<dyn FnMut(&mut MainWindow)>>, Box<dyn Fn(&mut MainWindow) -> bool>),
    Separator,
    SubMenu(Vec<(String, MenuItem)>)
}

pub enum MenuLabelType {
    Text(&'static str),
    Image(Rc<egui::TextureHandle>)
}

pub enum MenuType {
    Menu(Vec<(String, MenuItem)>),
    DirectAction(Box<dyn FnMut(&mut MainWindow)>)
}

/// Creates a Menu bar given a Vec of type (&str, MenuItem)
pub struct MainMenuBar {
    menu: Vec<(MenuLabelType, MenuType)>,
}

impl MainMenuBar {
    pub fn new() -> Self {
        Self { menu: Vec::new() }
    }

    pub fn add_menu(&mut self, name: &'static str, item: Vec<(String, MenuItem)>) {
        self.menu.push((MenuLabelType::Text(name), MenuType::Menu(item)))
    }

    /// Preloads an image and adds the image's handle to the menu array.
    /*pub fn add_menu_image(
        &mut self,
        image_id: &str,
        item: Vec<(&'static str, MenuItem)>,
        image_resources: &ImageResources
    ) {
        let handle = image_resources.get_image_handle(String::from(image_id));

        self.menu.push((
            MenuLabelType::Image(handle),
            MenuType::Menu(item)
        ))
    }*/

    /// Adds a menu item that acts like a button but is placed directly on the Main Menu.
    /*pub fn add_menu_action(&mut self, name: &'static str, action: Box<dyn FnMut(&mut MainWindow) + 'static>) {
        self.menu.push((MenuLabelType::Text(name), MenuType::DirectAction(action)));
    }*/

    pub fn add_menu_image_action(
        &mut self,
        image_id: &str,
        action: Box<dyn FnMut(&mut MainWindow) + 'static>,
        image_resources: &ImageResources
    ) {
        let handle = image_resources.get_image_handle(String::from(image_id));

        self.menu.push((
            MenuLabelType::Image(handle),
            MenuType::DirectAction(action)
        ));
    }

    fn draw_menu_items(parent: &mut MainWindow, ui: &mut Ui, menu_items: &mut Vec<(String, MenuItem)>) {
        for (label, menu_item) in menu_items.iter_mut() {
            match menu_item {
                MenuItem::MenuButton(action) => {
                    if ui.button(label.as_str()).clicked() {
                        if let Some(action) = action.as_mut() {
                            action(parent);
                            ui.close_menu();
                        }
                    }
                },
                MenuItem::Separator => {
                    ui.separator();
                },
                MenuItem::SubMenu(sub_menu_items) => {
                    ui.menu_button(label.as_str(), |ui| {
                        Self::draw_menu_items(parent, ui, sub_menu_items);
                    });
                },
                MenuItem::MenuButtonEnabled(action, enabled) => {
                    if ui.add_enabled(enabled(parent), egui::Button::new(label.as_str())).clicked() {
                        if let Some(action) = action.as_mut() {
                            action(parent);
                            ui.close_menu();
                        }
                    }
                }
            }
        }
    }

    pub fn draw_menu(&mut self, parent: &mut MainWindow, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar")
        .show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                for (menu_label_type, menu_contents) in self.menu.iter_mut() {
                    match menu_label_type {
                        MenuLabelType::Text(name) => {
                            match menu_contents {
                                MenuType::Menu(contents) => {
                                    ui.menu_button(*name, |ui| {
                                        Self::draw_menu_items(parent, ui, contents);
                                    });
                                },
                                MenuType::DirectAction(action) => {
                                    if ui.button(*name).clicked() {
                                        action(parent);
                                    }
                                }
                            }
                            
                        },

                        MenuLabelType::Image(handle) => {
                            let handle = &**handle;
                            match menu_contents {
                                MenuType::Menu(contents) => {
                                    ui.menu_image_button(handle, |ui| {
                                        Self::draw_menu_items(parent, ui, contents);
                                    });
                                },
                                MenuType::DirectAction(action) => {
                                    if ui.add(egui::ImageButton::new(handle)).clicked() {
                                        action(parent);
                                    }
                                }
                            }
                            
                        }
                    }
                    
                }
            })
        });
        //egui::menu::bar()
    }
}