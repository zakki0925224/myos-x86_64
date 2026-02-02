use core::ops::{Add, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: usize,
    pub y: usize,
}

impl From<(usize, usize)> for Point {
    fn from(value: (usize, usize)) -> Self {
        Self::new(value.0, value.1)
    }
}

impl Add for Point {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Point {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Point {
    pub const fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }

    pub fn xy(&self) -> (usize, usize) {
        (self.x, self.y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Size {
    pub width: usize,
    pub height: usize,
}

impl From<(usize, usize)> for Size {
    fn from(value: (usize, usize)) -> Self {
        Self::new(value.0, value.1)
    }
}

impl Size {
    pub const fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    pub fn wh(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    pub const fn from_point_and_size(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.origin.x
            && p.x < self.origin.x + self.size.width
            && p.y >= self.origin.y
            && p.y < self.origin.y + self.size.height
    }
}
