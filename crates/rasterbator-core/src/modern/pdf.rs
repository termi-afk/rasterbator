//! The modern PDF writer (design §2.5).
//!
//! This is a **sibling** of the legacy [`PdfDocument`](crate::PdfDocument), not
//! an extension of it. The legacy writer is byte-pinned on every axis: its
//! 2-decimal coordinate quantisation via
//! [`format_double`](crate::format_double), its first-page `q BT 0 Hd ET Q`
//! prologue, `/MediaBox` only, `/Resources << /ProcSet [/PDF] >>` and its
//! literal `/Title` / `/Creator`. Modern output needs none of those, so
//! touching the legacy writer would break every one of them (invariant I1).
//!
//! The two modes share exactly two pure, fixed-behaviour pieces:
//! [`format_double`](crate::format_double) and
//! [`CIRCLE_K`](crate::CIRCLE_K).
//!
//! Object layout, assigned up front from the plan so ids are deterministic:
//!
//! ```text
//! 1        Catalog  <</Type /Catalog /ViewerPreferences <</FitWindow true [/PrintScaling /None]>> /Pages 2 0 R>>
//! 2        Pages    /Kids [...] /Count N [/PageLabels ...]
//! per page p, base = 3 + 3p:
//!   base+0 Page     <</Type /Page /MediaBox [...] /TrimBox [...] /BleedBox [...]
//!                    /Parent 2 0 R /Resources <</ProcSet [/PDF][ /XObject <</Im0 base+2 0 R>>]> /Contents base+1 0 R>>
//!   base+1 Contents <</Length n /Filter /FlateDecode>> stream
//!   base+2 XObject  image stream, only when the page has one
//! 3 + 3N   Info     <</Producer (...) /Title (...) /Creator (...) [/CreationDate (...)]>
//! ```
//!
//! Unused object ids become free xref entries, which keeps the numbering a
//! pure function of the page count.

use std::io::Write;
use std::path::Path;

use crate::format_double;
use crate::modern::color::Rgb;
use crate::modern::geometry::{CellWindow, JobGeometry, PageBoxes};
use crate::modern::options::{LabelStyle, ModernOptions, PrintScaling};
use crate::modern::session::{EncodedImage, PageTarget};
use crate::modern::strokefont::col_label;

/// The PDF version modern output declares.
pub const PDF_VERSION: &str = "1.4";

/// Document metadata, all strings neutral and escape-checked.
#[derive(Clone, Debug, PartialEq)]
pub struct PdfInfo {
    /// `/Title`.
    pub title: String,
    /// `/Creator`.
    pub creator: String,
    /// `/Producer`.
    pub producer: String,
    /// `/CreationDate` in `D:YYYYMMDDHHmmSS` form, when requested.
    pub creation_date: Option<String>,
}

impl Default for PdfInfo {
    fn default() -> Self {
        PdfInfo {
            title: String::new(),
            creator: "rasterbator-rs modern".to_string(),
            producer: "rasterbator-rs".to_string(),
            creation_date: None,
        }
    }
}

impl PdfInfo {
    /// Build from the option set.
    pub fn from_options(o: &ModernOptions) -> PdfInfo {
        PdfInfo {
            title: o.title.clone(),
            creator: o.creator.clone(),
            producer: "rasterbator-rs".to_string(),
            creation_date: match o.creation_date {
                crate::modern::options::CreationDate::None => None,
                crate::modern::options::CreationDate::Now => Some(now_pdf_date()),
            },
        }
    }
}

/// `D:YYYYMMDDHHmmSS` for the current local time, truncated to minutes.
///
/// Only used when the user explicitly opts into a creation date; the default
/// is `None` so output stays byte-identical across runs (invariant I4).
pub fn now_pdf_date() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "D:{y:04}{m:02}{d:02}{:02}{:02}{:02}",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

/// Days since the Unix epoch to a civil `(year, month, day)`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn escape_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

fn f(v: f64) -> String {
    format_double(v)
}

/// One accumulated page.
#[derive(Clone, Debug, Default)]
pub struct ModernPage {
    /// Page boxes in points.
    pub boxes: PageBoxes,
    /// Uncompressed content stream.
    pub content: Vec<u8>,
    /// Embedded image, when the raster render path was used.
    pub image: Option<EncodedImage>,
}

/// A modern-mode PDF document.
#[derive(Clone, Debug)]
pub struct ModernPdf {
    pages: Vec<ModernPage>,
    info: PdfInfo,
    labels: LabelStyle,
    pages_x: u32,
    total_pages: u32,
    raw_content: bool,
    /// Whether the PDF asks compliant viewers to print at 100 %.
    print_scaling: PrintScaling,
}

impl ModernPdf {
    /// An empty document sized for a known page count.
    pub fn new(opts: &ModernOptions, geom: &JobGeometry) -> ModernPdf {
        ModernPdf {
            pages: Vec::with_capacity(geom.total_pages as usize),
            info: PdfInfo::from_options(opts),
            labels: opts.labels,
            pages_x: geom.pages_x.max(1),
            total_pages: geom.total_pages,
            raw_content: false,
            print_scaling: opts.print_scaling,
        }
    }

    /// An empty document with no page count known yet.
    pub fn empty() -> ModernPdf {
        ModernPdf {
            pages: Vec::new(),
            info: PdfInfo::default(),
            labels: LabelStyle::None,
            pages_x: 1,
            total_pages: 0,
            raw_content: false,
            print_scaling: PrintScaling::None,
        }
    }

    /// Disable stream compression. Test / debug hook, never an option.
    pub fn set_raw_content(&mut self, raw: bool) {
        self.raw_content = raw;
    }

    /// Replace the metadata.
    pub fn set_info(&mut self, info: PdfInfo) {
        self.info = info;
    }

    /// The metadata.
    pub fn info(&self) -> &PdfInfo {
        &self.info
    }

    /// How many pages were pushed.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// The pages, for tests and golden files.
    pub fn pages(&self) -> &[ModernPage] {
        &self.pages
    }

    fn push(&mut self, page: ModernPage) {
        if self.total_pages == 0 {
            self.total_pages = self.pages.len() as u32 + 1;
        }
        self.pages.push(page);
    }

    /// Assemble the file.
    pub fn bytes(&self) -> Vec<u8> {
        let n = self.pages.len();
        let info_id = 3 + 3 * n;
        let mut objects: Vec<Option<Vec<u8>>> = vec![None; info_id as usize + 1];

        // 1: catalog
        objects[1] = Some({
            let vp = match self.print_scaling {
                PrintScaling::None => "<< /FitWindow true /PrintScaling /None >>",
                PrintScaling::Viewer => "<< /FitWindow true >>",
            };
            format!("<< /Type /Catalog /ViewerPreferences {vp} /Pages 2 0 R >>").into_bytes()
        });

        // 2: page tree
        let mut kids = String::new();
        for p in 0..n {
            if p > 0 {
                kids.push(' ');
            }
            let base = 3 + 3 * p;
            kids.push_str(&format!("{} 0 R", base));
        }
        let mut pages = String::new();
        pages.push_str("<< /Type /Pages /Kids [");
        pages.push_str(&kids);
        pages.push_str(&format!("] /Count {} ", n));
        if self.labels == LabelStyle::Coordinates {
            // One label per page: the same "column letter + row number" the
            // page prints (e.g. `A1`), as a prefix-only label dictionary
            // (valid per the PDF spec; no numbering style is applied).
            pages.push_str("/PageLabels << /Nums [");
            let cols = self.pages_x.max(1);
            for p in 0..n {
                let col = (p as u32) % cols;
                let row = (p as u32) / cols;
                pages.push_str(&format!(
                    "{} << /P ({}) >> ",
                    p,
                    escape_literal(&format!("{}{}", col_label(col + 1), row + 1))
                ));
            }
            pages.push_str("] >> ");
        }
        pages.push_str(">>");
        objects[2] = Some(pages.into_bytes());

        for (p, page) in self.pages.iter().enumerate() {
            let base = 3 + 3 * p;
            let b = &page.boxes;
            let mut resources = String::from("<< /ProcSet [/PDF]");
            if page.image.is_some() {
                resources.push_str(&format!(" /XObject << /Im0 {} 0 R >>", base + 2));
            }
            resources.push_str(" >>");
            let page_obj = format!(
                "<< /Type /Page /MediaBox [{} {} {} {}] /TrimBox [{} {} {} {}] \
                 /BleedBox [{} {} {} {}] /Parent 2 0 R /Resources {} /Contents {} 0 R >>",
                f(b.media[0]),
                f(b.media[1]),
                f(b.media[2]),
                f(b.media[3]),
                f(b.trim[0]),
                f(b.trim[1]),
                f(b.trim[2]),
                f(b.trim[3]),
                f(b.bleed[0]),
                f(b.bleed[1]),
                f(b.bleed[2]),
                f(b.bleed[3]),
                resources,
                base + 1,
            );
            objects[base as usize] = Some(page_obj.into_bytes());

            let body = if self.raw_content {
                page.content.clone()
            } else {
                deflate(&page.content)
            };
            let mut contents = format!(
                "<< /Length {} >>\nstream\n",
                body.len()
            )
            .into_bytes();
            if !self.raw_content {
                contents.splice(0..0, b"<< /Filter /FlateDecode ".to_vec());
            }
            contents.extend_from_slice(&body);
            contents.extend_from_slice(b"\nendstream");
            objects[(base + 1) as usize] = Some(contents);

            if let Some(img) = &page.image {
                let mut stream =
                    format!(
                        "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace {} \
                         /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
                        img.w,
                        img.h,
                        img.component.pdf_name(),
                        img.bytes.len()
                    )
                    .into_bytes();
                stream.extend_from_slice(&img.bytes);
                stream.extend_from_slice(b"\nendstream");
                objects[(base + 2) as usize] = Some(stream);
            }
        }

        let mut info = String::from("<< ");
        info.push_str(&format!("/Producer ({}) ", escape_literal(&self.info.producer)));
        info.push_str(&format!("/Creator ({}) ", escape_literal(&self.info.creator)));
        if !self.info.title.is_empty() {
            info.push_str(&format!("/Title ({}) ", escape_literal(&self.info.title)));
        }
        if let Some(d) = &self.info.creation_date {
            info.push_str(&format!("/CreationDate ({}) ", escape_literal(d)));
        }
        info.push_str(">>");
        objects[info_id as usize] = Some(info.into_bytes());

        // Assemble.
        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(format!("%PDF-{PDF_VERSION}\n").as_bytes());
        out.extend_from_slice(b"%\xE2\xE3\xCF\xD3\n");
        let mut offsets = vec![0usize; info_id as usize + 1];
        for (id, body) in objects.iter().enumerate() {
            if id == 0 || objects[id].is_none() {
                continue;
            }
            offsets[id] = out.len();
            out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
            if let Some(b) = body {
                out.extend_from_slice(b);
            }
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref_at = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n", info_id + 1).as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for id in 1..=info_id as usize {
            if objects[id].is_some() {
                out.extend_from_slice(format!("{:010} 00000 n \n", offsets[id]).as_bytes());
            } else {
                out.extend_from_slice(b"0000000000 65535 f \n");
            }
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R /Info {} 0 R >>\nstartxref\n{}\n%%EOF\n",
                info_id + 1,
                info_id,
                xref_at
            )
            .as_bytes(),
        );
        out
    }

    /// Write the document, optionally through a temporary file and rename.
    ///
    /// `atomic` never changes the bytes, only how they land.
    pub fn save(&self, path: &Path, atomic: bool) -> Result<u64, String> {
        let bytes = self.bytes();
        if atomic {
            let tmp = temp_path(path);
            {
                let mut f = std::fs::File::create(&tmp)
                    .map_err(|e| format!("{}: {e}", tmp.display()))?;
                f.write_all(&bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
                f.flush().map_err(|e| format!("{}: {e}", tmp.display()))?;
                let _ = f.sync_all();
            }
            std::fs::rename(&tmp, path).map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                format!("{}: {e}", path.display())
            })?;
        } else {
            let mut f = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
            f.write_all(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
            f.flush().map_err(|e| format!("{}: {e}", path.display()))?;
        }
        Ok(bytes.len() as u64)
    }
}

fn temp_path(path: &Path) -> std::path::PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(format!(".tmp{}", std::process::id()));
    path.with_file_name(name)
}

/// zlib-compress with the level the legacy writer's `flate2` dependency uses.
fn deflate(data: &[u8]) -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    if enc.write_all(data).is_err() {
        return data.to_vec();
    }
    enc.finish().unwrap_or_else(|_| data.to_vec())
}

/// A [`PageTarget`] that accumulates pages into a [`ModernPdf`].
pub struct PdfTarget<'a> {
    pdf: &'a mut ModernPdf,
    page: Option<ModernPage>,
}

impl<'a> PdfTarget<'a> {
    /// Wrap a document.
    pub fn new(pdf: &'a mut ModernPdf) -> PdfTarget<'a> {
        PdfTarget { pdf, page: None }
    }

    /// Push the page in flight, if any.
    pub fn finish(&mut self) {
        if let Some(p) = self.page.take() {
            self.pdf.push(p);
        }
    }
}

fn op(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(b'\n');
}

fn num(out: &mut Vec<u8>, v: f64) {
    out.extend_from_slice(format_double(v).as_bytes());
    out.push(b' ');
}

impl<'a> PageTarget for PdfTarget<'a> {
    fn begin_page(&mut self, _index: u32, boxes: &PageBoxes, _job: &CellWindow) {
        self.page = Some(ModernPage {
            boxes: *boxes,
            content: Vec::new(),
            image: None,
        });
    }
    fn fill_rgb(&mut self, c: Rgb) {
        if let Some(p) = self.page.as_mut() {
            op(&mut p.content, &format!("{} {} {} rg", c.0, c.1, c.2));
        }
    }
    fn stroke_rgb(&mut self, c: Rgb) {
        if let Some(p) = self.page.as_mut() {
            op(&mut p.content, &format!("{} {} {} RG", c.0, c.1, c.2));
        }
    }
    fn line_width_pt(&mut self, w: f64) {
        if let Some(p) = self.page.as_mut() {
            let mut v = Vec::new();
            num(&mut v, w);
            op(&mut p.content, &format!("{}w", String::from_utf8_lossy(&v).trim_end()));
        }
    }
    fn move_to(&mut self, x: f64, y: f64) {
        if let Some(p) = self.page.as_mut() {
            let mut v = Vec::new();
            num(&mut v, x);
            num(&mut v, y);
            op(&mut p.content, &format!("{}m", String::from_utf8_lossy(&v).trim_end()));
        }
    }
    fn line_to(&mut self, x: f64, y: f64) {
        if let Some(p) = self.page.as_mut() {
            let mut v = Vec::new();
            num(&mut v, x);
            num(&mut v, y);
            op(&mut p.content, &format!("{}l", String::from_utf8_lossy(&v).trim_end()));
        }
    }
    fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) {
        if let Some(p) = self.page.as_mut() {
            let mut v = Vec::new();
            num(&mut v, x1);
            num(&mut v, y1);
            num(&mut v, x2);
            num(&mut v, y2);
            num(&mut v, x3);
            num(&mut v, y3);
            op(&mut p.content, &format!("{}c", String::from_utf8_lossy(&v).trim_end()));
        }
    }
    fn close_fill(&mut self) {
        if let Some(p) = self.page.as_mut() {
            op(&mut p.content, "h f");
        }
    }
    fn close_stroke(&mut self) {
        if let Some(p) = self.page.as_mut() {
            op(&mut p.content, "h S");
        }
    }
    fn draw_image(&mut self, img: &EncodedImage, rect: [f64; 4]) {
        if let Some(p) = self.page.as_mut() {
            let mut c = Vec::new();
            num(&mut c, rect[2] - rect[0]);
            num(&mut c, 0.0);
            num(&mut c, 0.0);
            num(&mut c, rect[3] - rect[1]);
            num(&mut c, rect[0]);
            num(&mut c, rect[1]);
            let mut v = String::from_utf8_lossy(&c).trim_end().to_string();
            v.push_str(" cm /Im0 Do Q");
            op(&mut p.content, &format!("q {v}"));
            p.image = Some(img.clone());
        }
    }
    fn abort_page(&mut self) {
        self.page = None;
    }
    fn end_page(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_are_sane() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_000), (2022, 1, 8));
    }

    #[test]
    fn literal_escaping() {
        assert_eq!(escape_literal("a(b)c\\d"), "a\\(b\\)c\\\\d");
        assert_eq!(escape_literal("plain"), "plain");
    }

    #[test]
    fn an_empty_document_is_well_formed() {
        let b = ModernPdf::empty().bytes();
        assert!(b.starts_with(b"%PDF-1.4\n"));
        assert!(b.ends_with(b"%%EOF\n"));
        assert_eq!(b.windows(7).filter(|w| *w == b"/Type /").count(), 2);
    }

    #[test]
    fn page_ids_are_assigned_from_the_plan() {
        let mut pdf = ModernPdf::empty();
        pdf.push(ModernPage {
            boxes: PageBoxes {
                media: [0.0, 0.0, 595.28, 841.89],
                trim: [28.35, 28.35, 566.93, 813.54],
                bleed: [28.35, 28.35, 566.93, 813.54],
            },
            content: b"0 0 m\n".to_vec(),
            image: None,
        });
        pdf.push(ModernPage {
            boxes: PageBoxes { media: [0.0; 4], trim: [0.0; 4], bleed: [0.0; 4] },
            content: b"0 0 l\n".to_vec(),
            image: None,
        });
        let b = String::from_utf8_lossy(&pdf.bytes()).into_owned();
        assert!(b.contains("3 0 obj\n<< /Type /Page"));
        assert!(b.contains("6 0 obj\n<< /Type /Page"));
        assert!(b.contains("9 0 obj\n<< /Producer"));
        // The image slot of a vector page is a free entry.
        assert!(b.contains("/Size 10"));
        // The cross-reference table has one entry per object plus the free
        // head, and every `n` entry must match an object header offset. The
        // offsets are byte offsets, so this check runs on the raw file.
        let raw = pdf.bytes();
        let needle = |id: u32| format!("\n{id} 0 obj\n");
        let find = |id: u32| raw.windows(needle(id).len()).position(|w| w == needle(id).as_bytes());
        let idx = raw.windows(5).position(|w| w == b"xref\n").unwrap();
        let xref = String::from_utf8_lossy(&raw[idx + 5..]).into_owned();
        let entries: Vec<&str> = xref.lines().skip(1).take(10).collect();
        assert_eq!(entries.len(), 10, "/Size is 10, so ten entries follow");
        assert!(entries[0].starts_with("0000000000 65535 f"), "the head is free");
        let used: Vec<u32> = (1..=9).filter(|id| find(*id).is_some()).collect();
        assert_eq!(used, vec![1, 2, 3, 4, 6, 7, 9], "the image slots stay free");
        for id in used {
            let at = find(id).unwrap() + 1;
            let entry = format!("{at:010} 00000 n");
            assert!(xref.contains(&entry), "missing xref entry for object {id} at {at}");
        }
    }
    #[test]
    fn raw_content_disables_compression() {
        let mut pdf = ModernPdf::empty();
        pdf.set_raw_content(true);
        pdf.push(ModernPage {
            boxes: PageBoxes { media: [0.0; 4], trim: [0.0; 4], bleed: [0.0; 4] },
            content: b"1 2 m\n3 4 l\n".to_vec(),
            image: None,
        });
        let b = String::from_utf8_lossy(&pdf.bytes()).into_owned();
        assert!(b.contains("1 2 m"), "raw stream must be readable");
        assert!(!b.contains("/FlateDecode"), "raw mode must not compress");

        let mut pdf2 = ModernPdf::empty();
        pdf2.push(ModernPage {
            boxes: PageBoxes { media: [0.0; 4], trim: [0.0; 4], bleed: [0.0; 4] },
            content: b"1 2 m\n".to_vec(),
            image: None,
        });
        let c = String::from_utf8_lossy(&pdf2.bytes()).into_owned();
        assert!(c.contains("/FlateDecode"));
    }

    #[test]
    fn labels_are_emitted_only_for_the_coordinate_style() {
        let mut pdf = ModernPdf::empty();
        pdf.labels = LabelStyle::Coordinates;
        pdf.push(ModernPage {
            boxes: PageBoxes { media: [0.0; 4], trim: [0.0; 4], bleed: [0.0; 4] },
            content: Vec::new(),
            image: None,
        });
        let b = String::from_utf8_lossy(&pdf.bytes()).into_owned();
        assert!(b.contains("/PageLabels << /Nums [0 << /P (A1) >> ]"));

        let mut plain = ModernPdf::empty();
        plain.push(ModernPage {
            boxes: PageBoxes { media: [0.0; 4], trim: [0.0; 4], bleed: [0.0; 4] },
            content: Vec::new(),
            image: None,
        });
        assert!(!String::from_utf8_lossy(&plain.bytes()).into_owned().contains("/PageLabels"));
    }

    #[test]
    fn bytes_are_deterministic() {
        let build = || {
            let mut pdf = ModernPdf::empty();
            pdf.push(ModernPage {
                boxes: PageBoxes {
                    media: [0.0, 0.0, 595.28, 841.89],
                    trim: [28.35, 28.35, 566.93, 813.54],
                    bleed: [28.35, 28.35, 566.93, 813.54],
                },
                content: b"1 2 m 3 4 l h S\n".to_vec(),
                image: None,
            });
            pdf.bytes()
        };
        assert_eq!(build(), build());
    }
}