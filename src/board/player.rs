use crate::pieces::pieces::Color;

pub struct Player {
    color: Color,
    name: String,
}

impl Player {
    pub fn new(name: String, color: Color) -> Self {
        Self { name, color }
    }

    pub fn color(&self) -> Color {
        self.color
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}