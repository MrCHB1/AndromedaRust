use std::{fmt::Display, str::FromStr};

use eframe::egui;
use num_traits::{NumCast, ToPrimitive};

pub trait NumberField {
    fn show(&mut self, label: &str, ui: &mut egui::Ui, width: Option<f32>) -> egui::Response;
    fn changed(&self) -> bool;
    
    // fn as_any(&self) -> &dyn Any;
    // fn as_any_mut(&mut self) -> &mut dyn Any;

    fn as_f32(&self) -> f32;
    // fn as_i32(&self) -> i32;
    fn as_u8(&self) -> u8;
}

pub struct NumericField<T> {
    buffer: String,
    min_value: Option<T>,
    max_value: Option<T>,
    value: T,
    pub changed: bool
}

impl<T> NumericField<T>
where
    T: NumCast + ToPrimitive + FromStr + Display + PartialOrd + Copy
{
    pub fn new(initial: T, min_value: Option<T>, max_value: Option<T>) -> Self {
        Self {
            buffer: initial.to_string(),
            min_value,
            max_value,
            value: initial,
            changed: false
        }
    }

    fn update_buffer(&mut self) {
        if let Ok(mut parsed) = self.buffer.parse::<T>() {
            // clamping
            if let Some(min_value) = self.min_value {
                if parsed < min_value { parsed = min_value; }
            }

            if let Some(max_value) = self.max_value {
                if parsed > max_value { parsed = max_value; }
            }

            self.value = parsed;
            self.buffer = parsed.to_string();
            self.changed = true;
        } else {
            self.buffer = self.value.to_string();
            self.changed = false;
        }
    }

    pub fn value(&self) -> T {
        self.value
    }

    pub fn set_value(&mut self, val: T) {
        self.value = val;
        self.buffer = val.to_string();
    }
}

impl<T> NumberField for NumericField<T>
where
    T: NumCast + ToPrimitive + FromStr + Display + PartialOrd + Copy + 'static
{
    fn show(&mut self, label: &str, ui: &mut egui::Ui, width: Option<f32>) -> egui::Response {
        self.changed = false;
        let mut text_edit = egui::TextEdit::singleline(&mut self.buffer);
        if let Some(width) = width { 
            text_edit = text_edit.desired_width(width);
        }

        let response = ui.horizontal(|ui| {
            ui.label(label);
            ui.add(text_edit)
        }).inner;

        if response.lost_focus() { self.update_buffer(); }

        response
    }

    fn changed(&self) -> bool {
        self.changed
    }

    /*fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_i32(&self) -> i32 {
        self.value.to_i32().unwrap_or(0)
    }*/

    fn as_u8(&self) -> u8 {
        self.value.to_u8().unwrap_or(0)
    }

    fn as_f32(&self) -> f32 {
        self.value.to_f32().unwrap_or(0.0)
    }
}