use std::{collections::HashMap, rc::Rc};

use eframe::egui::{self};

pub fn load_texture_from_path(
    ctx: &egui::Context,
    path: &str
) -> Result<egui::TextureHandle, String> {
    let img = image::open(path).map_err(|e| format!("Failed to open {}: {}", path, e))?;
    let img = img.to_rgba8();
    let (w, h) = img.dimensions();
    let size = [w as usize, h as usize];
    let pixels = img.into_raw();

    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);

    let texture = ctx.load_texture(path.to_owned(), color_image, egui::TextureOptions::default());
    Ok(texture)
}

#[derive(Default)]
pub struct ImageResources {
    handles: HashMap<String, Rc<egui::TextureHandle>>
}

impl ImageResources {
    pub fn new() -> Self {
        Self { handles: HashMap::new() }
    }

    pub fn preload_image(&mut self, ctx: &egui::Context, path: &str, id: String) {
        if self.handles.get(&id).is_some() { eprintln!("Warning: texture with id {} already exists, will overwrite", id); }
        let handle = load_texture_from_path(ctx, path).unwrap();
        self.handles.insert(id, Rc::new(handle));
    }

    pub fn get_image_handle(&self, name: String) -> Rc<egui::TextureHandle> {
        self.handles.get(&name).unwrap().clone()
    }
}