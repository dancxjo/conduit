//! Tiny fixed Conduit icon vocabulary drawn from bounded geometry.

use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point, Primitive, RgbColor},
    primitives::{Circle, Line, PrimitiveStyle, Rectangle, Triangle},
    Drawable,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Form,
    Build,
    Body,
    Wake,
    Lull,
    Plan,
    Play,
    Stop,
    Hold,
    Host,
    Gear,
    PortInput,
    PortOutput,
    Cord,
    Line,
    Sign,
    Info,
    Face,
    Back,
    Warning,
    Failure,
    Success,
    Open,
    Save,
    Inspect,
}

impl Icon {
    pub const ALL: [Self; 25] = [
        Self::Form,
        Self::Build,
        Self::Body,
        Self::Wake,
        Self::Lull,
        Self::Plan,
        Self::Play,
        Self::Stop,
        Self::Hold,
        Self::Host,
        Self::Gear,
        Self::PortInput,
        Self::PortOutput,
        Self::Cord,
        Self::Line,
        Self::Sign,
        Self::Info,
        Self::Face,
        Self::Back,
        Self::Warning,
        Self::Failure,
        Self::Success,
        Self::Open,
        Self::Save,
        Self::Inspect,
    ];

    pub const fn accessibility_name(self) -> &'static str {
        match self {
            Self::Form => "Form",
            Self::Build => "Build",
            Self::Body => "Body",
            Self::Wake => "Wake",
            Self::Lull => "Lull",
            Self::Plan => "Plan",
            Self::Play => "Play",
            Self::Stop => "Stop",
            Self::Hold => "Hold",
            Self::Host => "Host",
            Self::Gear => "Gear",
            Self::PortInput => "Input Port",
            Self::PortOutput => "Output Port",
            Self::Cord => "Cord",
            Self::Line => "Line",
            Self::Sign => "Sign",
            Self::Info => "Info",
            Self::Face => "Face",
            Self::Back => "Back",
            Self::Warning => "Warning",
            Self::Failure => "Failure",
            Self::Success => "Success",
            Self::Open => "Open",
            Self::Save => "Save",
            Self::Inspect => "Inspect",
        }
    }
}

pub fn draw_icon<D>(target: &mut D, icon: Icon, origin: Point, color: Rgb888)
where
    D: DrawTarget<Color = Rgb888>,
{
    let stroke = PrimitiveStyle::with_stroke(color, 1);
    let fill = PrimitiveStyle::with_fill(color);
    let p = |x, y| origin + Point::new(x, y);
    match icon {
        Icon::Play => {
            let _ = Triangle::new(p(3, 2), p(3, 13), p(12, 7))
                .into_styled(fill)
                .draw(target);
        }
        Icon::Stop | Icon::Hold => {
            let _ = Rectangle::with_corners(p(3, 3), p(12, 12))
                .into_styled(if icon == Icon::Stop { fill } else { stroke })
                .draw(target);
        }
        Icon::PortInput | Icon::PortOutput => {
            let _ = Circle::new(p(4, 4), 8).into_styled(stroke).draw(target);
            let (a, b) = if icon == Icon::PortInput {
                (13, 8)
            } else {
                (8, 13)
            };
            let _ = Line::new(p(a, 8), p(b, 8)).into_styled(stroke).draw(target);
        }
        Icon::Success => {
            let _ = Line::new(p(2, 8), p(6, 12))
                .into_styled(stroke)
                .draw(target);
            let _ = Line::new(p(6, 12), p(14, 3))
                .into_styled(stroke)
                .draw(target);
        }
        Icon::Failure => {
            let _ = Line::new(p(3, 3), p(13, 13))
                .into_styled(stroke)
                .draw(target);
            let _ = Line::new(p(13, 3), p(3, 13))
                .into_styled(stroke)
                .draw(target);
        }
        Icon::Warning => {
            let _ = Triangle::new(p(8, 1), p(1, 14), p(15, 14))
                .into_styled(stroke)
                .draw(target);
            let _ = Line::new(p(8, 5), p(8, 10))
                .into_styled(stroke)
                .draw(target);
        }
        Icon::Cord | Icon::Line => {
            let _ = Line::new(p(1, 12), p(6, 5))
                .into_styled(stroke)
                .draw(target);
            let _ = Line::new(p(6, 5), p(14, 9))
                .into_styled(stroke)
                .draw(target);
        }
        Icon::Gear => {
            let _ = Circle::new(p(3, 3), 10).into_styled(stroke).draw(target);
            let _ = Circle::new(p(6, 6), 4).into_styled(stroke).draw(target);
        }
        Icon::Wake => {
            let _ = Circle::new(p(3, 3), 10).into_styled(stroke).draw(target);
            let _ = Line::new(p(8, 0), p(8, 4)).into_styled(stroke).draw(target);
        }
        Icon::Lull => {
            let _ = Circle::new(p(2, 2), 12).into_styled(stroke).draw(target);
            let _ = Circle::new(p(7, 0), 12)
                .into_styled(PrimitiveStyle::with_fill(Rgb888::BLACK))
                .draw(target);
        }
        Icon::Inspect => {
            let _ = Circle::new(p(2, 2), 9).into_styled(stroke).draw(target);
            let _ = Line::new(p(10, 10), p(15, 15))
                .into_styled(stroke)
                .draw(target);
        }
        Icon::Open => {
            let _ = Rectangle::with_corners(p(1, 5), p(14, 13))
                .into_styled(stroke)
                .draw(target);
            let _ = Line::new(p(1, 5), p(6, 1)).into_styled(stroke).draw(target);
        }
        Icon::Save => {
            let _ = Rectangle::with_corners(p(2, 1), p(13, 14))
                .into_styled(stroke)
                .draw(target);
            let _ = Rectangle::with_corners(p(5, 9), p(11, 13))
                .into_styled(stroke)
                .draw(target);
        }
        _ => {
            let _ = Rectangle::with_corners(p(2, 2), p(13, 13))
                .into_styled(stroke)
                .draw(target);
            let _ = Line::new(p(4, 6), p(11, 6))
                .into_styled(stroke)
                .draw(target);
            let _ = Line::new(p(4, 9), p(11, 9))
                .into_styled(stroke)
                .draw(target);
        }
    }
}
