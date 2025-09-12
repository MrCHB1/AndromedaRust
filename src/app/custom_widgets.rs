use eframe::egui;
use num_traits::{NumCast, PrimInt, Num};

pub trait EditField<T> {
    fn show(&mut self, label: &str, ui: &mut egui::Ui, width: Option<f32>) -> egui::Response;
    fn update_value(&mut self, new_value: T);
    fn update_buffer(&mut self);
    fn value(&self) -> T;
}

pub struct IntegerField {
    buffer: String,
    min_value: i32,
    max_value: i32,
    value: i32,
    pub changed: bool,
}

impl IntegerField {
    pub fn new(initial: i32, min_value: Option<i32>, max_value: Option<i32>) -> Self {
        Self {
            buffer: initial.to_string(),
            min_value: min_value.unwrap_or(i32::MIN),
            max_value: max_value.unwrap_or(i32::MAX),
            value: initial,
            changed: false
        }
    }
}

impl EditField<i32> for IntegerField {
    fn show(&mut self, label: &str, ui: &mut egui::Ui, width: Option<f32>) -> egui::Response {
        self.changed = false;
        let mut text_edit = egui::TextEdit::singleline(&mut self.buffer);
        if let Some(width) = width {
            text_edit = text_edit.desired_width(width);
        }

        let res = ui.horizontal(|ui| {
            ui.label(label);
            let response = ui.add(text_edit);
            response
        }).inner;

        if res.lost_focus() {
            self.update_buffer();
        }

        res
    }

    fn update_value(&mut self, new_value: i32) {
        self.buffer = new_value.to_string();
        self.value = new_value;
    }

    fn update_buffer(&mut self) {
        if let Ok(parsed) = self.buffer.parse::<i32>() {
            self.value = parsed.clamp(self.min_value,self.max_value);
            self.changed = true;
        } else {
            self.buffer = self.value.to_string();
            self.changed = false;
        }
    }

    fn value(&self) -> i32 {
        self.value
    }
}

pub struct DecimalField {
    buffer: String,   // what the user is typing
    min_value: f64,
    max_value: f64,
    value: f64,       // the last successfully parsed value
}

impl DecimalField {
    pub fn new(initial: f64, min_value: Option<f64>, max_value: Option<f64>) -> Self {
        Self {
            buffer: initial.to_string(),
            min_value: min_value.unwrap_or(f64::MIN),
            max_value: max_value.unwrap_or(f64::MAX),
            value: initial,
        }
    }

    pub fn show(&mut self, label: &str, ui: &mut egui::Ui, width: Option<f32>) -> egui::Response {
        ui.label(label);

        let mut text_edit = egui::TextEdit::singleline(&mut self.buffer);
        if let Some(width) = width {
            text_edit = text_edit.desired_width(width);
        }

        let res = ui.add(text_edit);

        if res.lost_focus() {
            self.update_buffer();
        }

        res
    }

    fn update_buffer(&mut self) {
        if let Ok(parsed) = self.buffer.parse::<f64>() {
            self.value = parsed.clamp(self.min_value, self.max_value);
        } else {
            self.buffer = self.value.to_string();
        }
    }

    pub fn value(&self) -> f64 {
        self.value
    }
}