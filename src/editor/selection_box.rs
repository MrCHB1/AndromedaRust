use num_traits::{PrimInt, Zero};

/// A Selection Box struct. [`T`] is the X pos type. [`U`] is the Y pos type.
#[derive(Clone, Copy)]
pub struct SelectionBox<T: PrimInt + Default, U: PrimInt + Default> {
    start_x: T,
    end_x: T,
    start_y: U,
    end_y: U
}

impl<T, U> From<(T, U)> for SelectionBox<T, U>
where T: PrimInt + Default, U: PrimInt + Default {
    fn from((x, y): (T, U)) -> Self {
        Self {
            start_x: x,
            end_x: x,
            start_y: y,
            end_y: y
        }
    }
}

impl<T, U> From<((T, U), (T, U))> for SelectionBox<T, U>
where T: PrimInt + Default, U: PrimInt + Default {
    fn from(((x1, y1), (x2, y2)): ((T, U), (T, U))) -> Self {
        let (start_x, end_x) = if x1 > x2 {
            (x2, x1)
        } else {
            (x1, x2)
        };

        let (start_y, end_y) = if y1 > y2 {
            (y2, y1)
        } else {
            (y1, y2)
        };

        Self {
            start_x,
            end_x,
            start_y,
            end_y
        }
    }
}

impl<T: PrimInt + Default, U: PrimInt + Default> SelectionBox<T, U> {
    pub fn new() -> Self {
        Self { start_x: T::default(), end_x: T::default(), start_y: U::default(), end_y: U::default() }
    }

    pub fn init_from(&mut self, start_pos: (T, U)) {
        self.start_x = start_pos.0;
        self.end_x = start_pos.0;
        self.start_y = start_pos.1;
        self.end_y = start_pos.1;
    }

    pub fn top_left(&self) -> (T, U) {
        (self.start_x, self.start_y)
    }

    pub fn top_right(&self) -> (T, U) {
        (self.end_x, self.start_y)
    }

    pub fn bottom_left(&self) -> (T, U) {
        (self.start_x, self.end_y)
    }

    pub fn bottom_right(&self) -> (T, U) {
        (self.end_x, self.end_y)
    }
}