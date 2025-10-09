use as_any::AsAny;
use eframe::egui::{self};
use crate::app::{util::image_loader::ImageResources};

pub trait Dialog: AsAny {
    fn show(&mut self) -> ();
    fn close(&mut self) -> ();
    fn is_showing(&self) -> bool;
    fn draw(&mut self, ctx: &egui::Context, image_resources: &ImageResources) -> ();
}