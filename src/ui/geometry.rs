#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    #[must_use]
    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);

        (right > left && bottom > top).then_some(Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        })
    }
}

pub type Rgba = [f32; 4];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextWeight {
    Normal,
    Semibold,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextAlign {
    #[default]
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Quad {
    pub bounds: Rect,
    pub color: Rgba,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub content: String,
    pub bounds: Rect,
    pub clip_bounds: Rect,
    pub color: [u8; 4],
    pub font_size: f32,
    pub line_height: f32,
    pub weight: TextWeight,
    pub align: TextAlign,
}

pub(super) fn rgba8(color: Rgba) -> [u8; 4] {
    color.map(|channel| (channel * 255.0) as u8)
}

pub(super) fn border_quads(
    bounds: Rect,
    thickness: f32,
    color: Rgba,
) -> impl Iterator<Item = Quad> {
    [
        quad(bounds.x, bounds.y, bounds.width, thickness, color),
        quad(
            bounds.x,
            bounds.y + bounds.height - thickness,
            bounds.width,
            thickness,
            color,
        ),
        quad(bounds.x, bounds.y, thickness, bounds.height, color),
        quad(
            bounds.x + bounds.width - thickness,
            bounds.y,
            thickness,
            bounds.height,
            color,
        ),
    ]
    .into_iter()
}

pub(super) fn quad(x: f32, y: f32, width: f32, height: f32, color: Rgba) -> Quad {
    Quad {
        bounds: rect(x, y, width, height),
        color,
    }
}

pub(super) fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect {
        x,
        y,
        width: width.max(0.0),
        height: height.max(0.0),
    }
}

pub(super) fn text(
    content: impl Into<String>,
    bounds: Rect,
    color: [u8; 4],
    font_size: f32,
    line_height: f32,
    weight: TextWeight,
) -> Text {
    Text {
        content: content.into(),
        bounds,
        clip_bounds: bounds,
        color,
        font_size,
        line_height,
        weight,
        align: TextAlign::Left,
    }
}

pub(super) fn right_aligned_text(
    content: impl Into<String>,
    bounds: Rect,
    color: [u8; 4],
    font_size: f32,
    line_height: f32,
    weight: TextWeight,
) -> Text {
    Text {
        align: TextAlign::Right,
        ..text(content, bounds, color, font_size, line_height, weight)
    }
}
