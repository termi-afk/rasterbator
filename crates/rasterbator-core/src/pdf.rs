//! Minimal PDF writer whose content streams match the original output
//! (produced through iTextSharp 1.0.4.0) op for op, including iText 1.x's
//! number formatting, page preamble, and circle geometry.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::canvas::{circle_path, Canvas};
use crate::numfmt::format_double;
use crate::size::paper_size_pt;

/// A multi-page PDF document that implements [`Canvas`].
#[derive(Debug, Clone)]
pub struct PdfDocument {
    page_width_pt: f32,
    page_height_pt: f32,
    current: String,
    pages: Vec<String>,
    /// PDF `/Title`.
    pub title: String,
    /// PDF `/Creator`.
    pub creator: String,
    /// PDF `/Producer`.
    pub producer: String,
}

impl PdfDocument {
    /// Create a document with pages of `paper_width_mm` x `paper_height_mm`.
    pub fn from_paper_mm(paper_width_mm: f64, paper_height_mm: f64) -> Self {
        let (page_width_pt, page_height_pt) = paper_size_pt(paper_width_mm, paper_height_mm);
        PdfDocument {
            page_width_pt,
            page_height_pt,
            current: String::new(),
            pages: Vec::new(),
            title: "Rasterbation".to_string(),
            creator: "The Rasterbator at http://homokaasu.org/rasterbator/".to_string(),
            producer: concat!("rasterbator-rs ", env!("CARGO_PKG_VERSION")).to_string(),
        }
    }

    /// Page size in PDF points.
    pub fn page_size_pt(&self) -> (f32, f32) {
        (self.page_width_pt, self.page_height_pt)
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Write the document to `path`.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let n = self.pages.len();
        let info_id = 3 + 2 * n; // ids: 1 catalog, 2 pages, 3+2i page, 4+2i content, info last
        let mut out: Vec<u8> = Vec::with_capacity(64 * 1024);
        let mut offsets: Vec<usize> = vec![0; info_id];

        out.extend_from_slice(b"%PDF-1.4\n%\xE0\xE1\xE2\xE3\n");

        fn obj(out: &mut Vec<u8>, offsets: &mut [usize], id: usize, body: &[u8]) {
            offsets[id - 1] = out.len();
            out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }

        obj(
            &mut out,
            &mut offsets,
            1,
            b"<< /Type /Catalog /ViewerPreferences << /FitWindow true >> /Pages 2 0 R >>",
        );

        let mut kids = String::new();
        for i in 0..n {
            kids.push_str(&format!("{} 0 R ", 3 + 2 * i));
        }
        obj(
            &mut out,
            &mut offsets,
            2,
            format!(
                "<< /Type /Pages /Kids [{}] /Count {} >>",
                kids.trim_end(),
                n
            )
            .as_bytes(),
        );

        for (i, page) in self.pages.iter().enumerate() {
            let page_id = 3 + 2 * i;
            let content_id = 4 + 2 * i;
            obj(
                &mut out,
                &mut offsets,
                page_id,
                format!(
                    "<< /Type /Page /MediaBox [0 0 {} {}] /Parent 2 0 R /Resources << /ProcSet [/PDF] >> /Contents {} 0 R >>",
                    format_double(self.page_width_pt as f64),
                    format_double(self.page_height_pt as f64),
                    content_id
                )
                .as_bytes(),
            );

            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(page.as_bytes())?;
            let compressed = encoder.finish()?;

            let mut body = Vec::with_capacity(compressed.len() + 64);
            body.extend_from_slice(
                format!(
                    "<< /Length {} /Filter /FlateDecode >>\nstream\n",
                    compressed.len()
                )
                .as_bytes(),
            );
            body.extend_from_slice(&compressed);
            body.extend_from_slice(b"\nendstream");
            obj(&mut out, &mut offsets, content_id, &body);
        }

        obj(
            &mut out,
            &mut offsets,
            info_id,
            format!(
                "<< /CreationDate ({}) /Producer ({}) /Title ({}) /Creator ({}) >>",
                pdf_date_now(),
                pdf_escape(&self.producer),
                pdf_escape(&self.title),
                pdf_escape(&self.creator)
            )
            .as_bytes(),
        );

        let xref_offset = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n", info_id + 1).as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for offset in offsets.iter().take(info_id) {
            out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R /Info {} 0 R >>\nstartxref\n{}\n%%EOF\n",
                info_id + 1,
                info_id,
                xref_offset
            )
            .as_bytes(),
        );

        let mut file = File::create(path)?;
        file.write_all(&out)?;
        Ok(())
    }
}

impl Canvas for PdfDocument {
    fn set_fill_rgb(&mut self, r: u8, g: u8, b: u8) {
        rgb(&mut self.current, r, g, b, "rg");
    }

    fn set_stroke_rgb(&mut self, r: u8, g: u8, b: u8) {
        rgb(&mut self.current, r, g, b, "RG");
    }

    fn set_line_width(&mut self, width: f32) {
        let s = &mut self.current;
        s.push_str(&num(width));
        s.push_str(" w\n");
    }

    fn move_to(&mut self, x: f32, y: f32) {
        let s = &mut self.current;
        s.push_str(&num(x));
        s.push(' ');
        s.push_str(&num(y));
        s.push_str(" m\n");
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let s = &mut self.current;
        s.push_str(&num(x));
        s.push(' ');
        s.push_str(&num(y));
        s.push_str(" l\n");
    }

    fn circle(&mut self, x: f32, y: f32, r: f32) {
        let (start, curves) = circle_path(x, y, r);
        self.move_to(start.0, start.1);
        for c in curves {
            let s = &mut self.current;
            s.push_str(&num(c[0]));
            s.push(' ');
            s.push_str(&num(c[1]));
            s.push(' ');
            s.push_str(&num(c[2]));
            s.push(' ');
            s.push_str(&num(c[3]));
            s.push(' ');
            s.push_str(&num(c[4]));
            s.push(' ');
            s.push_str(&num(c[5]));
            s.push_str(" c\n");
        }
    }

    fn rectangle(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let s = &mut self.current;
        s.push_str(&num(x));
        s.push(' ');
        s.push_str(&num(y));
        s.push(' ');
        s.push_str(&num(w));
        s.push(' ');
        s.push_str(&num(h));
        s.push_str(" re\n");
    }

    fn fill(&mut self) {
        self.current.push_str("f\n");
    }

    fn stroke(&mut self) {
        self.current.push_str("S\n");
    }

    fn begin_page(&mut self, page_index: usize) {
        self.current.clear();
        if page_index == 0 {
            // iText 1.0.4 writes this prologue to the first page's content.
            self.current.push_str("q\nBT\n0 ");
            self.current.push_str(&num(self.page_height_pt));
            self.current.push_str(" Td\nET\nQ\n");
        }
    }

    fn end_page(&mut self) {
        let page = std::mem::take(&mut self.current);
        self.pages.push(page);
    }
}

/// Format a coordinate the way iTextSharp 1.0.4 does: float32 value, then the
/// double formatter.
#[inline]
fn num(value: f32) -> String {
    format_double(value as f64)
}

fn rgb(s: &mut String, r: u8, g: u8, b: u8, op: &str) {
    s.push_str(&format_double((r as f32 / 255.0f32) as f64));
    s.push(' ');
    s.push_str(&format_double((g as f32 / 255.0f32) as f64));
    s.push(' ');
    s.push_str(&format_double((b as f32 / 255.0f32) as f64));
    s.push(' ');
    s.push_str(op);
    s.push('\n');
}

fn pdf_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            c if c.is_ascii() => out.push(c),
            c => {
                // Encode as UTF-16BE hex string fragment inside a literal
                // string is not possible; use '?' fallback for the metadata
                // strings, which are ASCII in the original.
                let _ = c;
                out.push('?');
            }
        }
    }
    out
}

/// Current UTC time as a PDF date string (`D:YYYYMMDDHHmmSSZ`).
fn pdf_date_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (year, month, day, hour, minute, second) = civil_from_unix(secs);
    format!("D:{year:04}{month:02}{day:02}{hour:02}{minute:02}{second:02}Z")
}

/// Days-from-epoch to civil date (Howard Hinnant's algorithm).
fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hour = (rem / 3600) as u32;
    let minute = ((rem % 3600) / 60) as u32;
    let second = (rem % 60) as u32;

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d, hour, minute, second)
}
