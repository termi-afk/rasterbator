//! The drawing surface the engine paints onto.

/// Drawing operations mirroring the `iTextSharp.text.pdf.PdfContentByte`
/// calls made by the original engine (see `original/source/Rasterbator.cs`).
///
/// Implementations receive one `begin_page`/`end_page` pair per output page.
/// Coordinates are in PDF points, with the origin at the bottom-left page
/// corner; colors are 0-255 RGB.
pub trait Canvas {
    fn set_fill_rgb(&mut self, r: u8, g: u8, b: u8);
    fn set_stroke_rgb(&mut self, r: u8, g: u8, b: u8);
    fn set_line_width(&mut self, width: f32);
    fn move_to(&mut self, x: f32, y: f32);
    fn line_to(&mut self, x: f32, y: f32);
    fn circle(&mut self, x: f32, y: f32, r: f32);
    fn rectangle(&mut self, x: f32, y: f32, w: f32, h: f32);
    fn fill(&mut self);
    fn stroke(&mut self);
    fn begin_page(&mut self, page_index: usize);
    fn end_page(&mut self);
}

/// Bezier constant used by iTextSharp 1.0.4's `PdfContentByte.circle`.
pub const CIRCLE_K: f32 = 0.5523;

/// Geometry of one circle drawn as four cubic Bezier curves, exactly as
/// iTextSharp 1.0.4 emits it: starting point plus four `[x1 y1 x2 y2 x3 y3]`
/// control/end point triples, counterclockwise from `(x + r, y)`.
pub fn circle_path(x: f32, y: f32, r: f32) -> ((f32, f32), [[f32; 6]; 4]) {
    let k = CIRCLE_K;
    (
        (x + r, y),
        [
            [x + r, y + r * k, x + r * k, y + r, x, y + r],
            [x - r * k, y + r, x - r, y + r * k, x - r, y],
            [x - r, y - r * k, x - r * k, y - r, x, y - r],
            [x + r * k, y - r, x + r, y - r * k, x + r, y],
        ],
    )
}
