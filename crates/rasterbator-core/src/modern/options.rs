//! The modern-mode option model.
//!
//! [`ModernOptions`] has exactly one public field per user-facing choice. The
//! field name is the canonical option key used by
//!
//! * the CLI long flag (see [`cli_flag`] for the handful of shared spellings),
//! * the GUI control id and combo-box entries,
//! * the preset file key,
//! * and the help-text heading, in [`OPTION_NAMES`] order.
//!
//! Every enum here carries `ALL`, [`cli_str`](TraitNames::cli_str) and
//! [`parse_cli`](TraitNames::parse_cli) from a single table, so the CLI value
//! parser, the preset writer and the GUI combo boxes cannot drift apart.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::size::{paper_preset, PAPER_PRESETS};
use crate::modern::color::Rgb;

// ---------------------------------------------------------------------------
// enum generation
// ---------------------------------------------------------------------------

/// Generate an option enum together with its CLI and preset-name tables.
macro_rules! option_enum {
    (
        $(#[$m:meta])*
        $name:ident, $slug:literal, [ $( $variant:ident => ($cli:literal, $snake:literal) ),+ $(,)? ]
    ) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $( $variant ),+
        }

        impl $name {
            /// Every variant, in CLI order.
            pub const ALL: &'static [Self] = &[ $( Self::$variant ),+ ];

            /// The exact token accepted by the CLI.
            pub fn cli_str(self) -> &'static str {
                match self { $( Self::$variant => $cli ),+ }
            }

            /// Parse a CLI token (case-insensitive, surrounding space ignored).
            pub fn parse_cli(s: &str) -> Option<Self> {
                let s = s.trim().to_ascii_lowercase();
                $( if s == $cli { return Some(Self::$variant); } )+
                None
            }

            /// The `lower_snake` name used in preset files.
            pub fn variant_name(self) -> &'static str {
                match self { $( Self::$variant => $snake ),+ }
            }

            /// Parse a preset-file variant name.
            pub fn from_variant(s: &str) -> Option<Self> {
                let s = s.trim().to_ascii_lowercase();
                $( if s == $snake { return Some(Self::$variant); } )+
                None
            }

            /// Stable slug, used by `--print-summary` and golden files.
            pub fn slug(self) -> &'static str { $slug }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.cli_str())
            }
        }
    };
}

option_enum!(
    /// Screen family: the lattice the cells sit on.
    ScreenFamily, "SCREEN",
    [
        Lattice => ("lattice", "lattice"),
        Hex => ("hex", "hex"),
        Plain => ("plain", "plain"),
    ]
);

option_enum!(
    /// Row stagger of a lattice screen.
    Stagger, "STAGGER",
    [
        Off => ("off", "off"),
        HalfPitch => ("half-pitch", "half_pitch"),
    ]
);

option_enum!(
    /// Shape emitted per cell.
    Pattern, "PATTERN",
    [
        Dot => ("dot", "dot"),
        Square => ("square", "square"),
        Triangle => ("triangle", "triangle"),
        TriangleAlt => ("triangle-alt", "triangle_alt"),
        LineH => ("line-h", "line_h"),
        LineV => ("line-v", "line_v"),
        LineMorph => ("line-morph", "line_morph"),
        Mosaic => ("mosaic", "mosaic"),
    ]
);

impl Pattern {
    /// True when the pattern is one of the three bar patterns.
    pub fn is_line(self) -> bool {
        matches!(self, Pattern::LineH | Pattern::LineV | Pattern::LineMorph)
    }

    /// True when the geometry is the cell square (`Square` and `Mosaic` are
    /// deliberately identical; only the colour operator differs).
    pub fn is_square(self) -> bool {
        matches!(self, Pattern::Square | Pattern::Mosaic)
    }

    /// Area of one shape, in units of `e^2` (half-extent squared).
    pub fn area_factor(self) -> f64 {
        match self {
            Pattern::Dot => std::f64::consts::PI,
            Pattern::Square | Pattern::Mosaic => 4.0,
            // Equilateral triangle with circumradius `e`: side = e*sqrt(3),
            // area = sqrt(3)/4 * 3e^2 = sqrt(3)/2 * e^2.
            Pattern::Triangle | Pattern::TriangleAlt => 3.0f64.sqrt() / 2.0,
            // Bar with half-length e and half-thickness e*line_aspect.
            Pattern::LineH | Pattern::LineV | Pattern::LineMorph => 4.0,
        }
    }
}

option_enum!(
    /// Maximum dot coverage relative to the cell.
    Coverage, "COVERAGE",
    [
        FullCell => ("full-cell", "full_cell"),
        Tangent => ("tangent", "tangent"),
    ]
);

option_enum!(
    /// Response curve applied to darkness before the tone band.
    ToneCurve, "TONE",
    [
        None => ("none", "none"),
        Exponential => ("exp", "exponential"),
        Logarithmic => ("log", "logarithmic"),
    ]
);

option_enum!(
    /// Per-cell sampling method.
    Sampling, "SAMPLING",
    [
        BoxAverage => ("box", "box_average"),
        Nearest => ("nearest", "nearest"),
        Bilinear => ("bilinear", "bilinear"),
    ]
);

option_enum!(
    /// Quantisation / error-diffusion algorithm applied to the tone output.
    Dither, "DITHER",
    [
        None => ("none", "none"),
        FloydSteinberg => ("fs", "floyd_steinberg"),
        Atkinson => ("atkinson", "atkinson"),
        Bayer4x4 => ("bayer4x4", "bayer4x4"),
    ]
);

option_enum!(
    /// Where the ink colour of a cell comes from.
    InkMode, "INK",
    [
        Fixed => ("fixed", "fixed"),
        Average => ("average", "average"),
        Grayscale => ("grayscale", "grayscale"),
        DualTone => ("dual", "dual_tone"),
    ]
);

option_enum!(
    /// Sheet orientation; swaps the paper dimensions.
    Orientation, "ORIENTATION",
    [
        Portrait => ("portrait", "portrait"),
        Landscape => ("landscape", "landscape"),
    ]
);

option_enum!(
    /// How the image is fitted to the poster.
    Fit, "FIT",
    [
        Contain => ("contain", "contain"),
        Cover => ("cover", "cover"),
        Stretch => ("stretch", "stretch"),
    ]
);

option_enum!(
    /// Horizontal / vertical placement of the image inside the poster.
    Align, "ALIGN",
    [
        Start => ("start", "start"),
        Center => ("center", "center"),
        End => ("end", "end"),
    ]
);

option_enum!(
    /// Unit of an explicit `--size` value.
    SizeUnit, "SIZE_UNIT",
    [
        Mm => ("mm", "mm"),
        Cm => ("cm", "cm"),
        In => ("in", "in"),
    ]
);

impl SizeUnit {
    /// Convert a value expressed in this unit to millimetres.
    pub fn to_mm(self, v: f64) -> f64 {
        match self {
            SizeUnit::Mm => v,
            SizeUnit::Cm => v * 10.0,
            SizeUnit::In => v * 25.4,
        }
    }

    /// Convert millimetres into this unit.
    pub fn from_mm(self, mm: f64) -> f64 {
        match self {
            SizeUnit::Mm => mm,
            SizeUnit::Cm => mm / 10.0,
            SizeUnit::In => mm / 25.4,
        }
    }

    /// The unit as a CLI suffix, used by `--size-unit`.
    pub fn suffix(self) -> &'static str {
        match self {
            SizeUnit::Mm => "mm",
            SizeUnit::Cm => "cm",
            SizeUnit::In => "in",
        }
    }
}

option_enum!(
    /// How the overlap strip between sheets is produced (decision D2 / A-1).
    OverlapMode, "OVERLAP_MODE",
    [
        Margin => ("margin", "margin"),
        Shared => ("shared", "shared"),
    ]
);

option_enum!(
    /// Cutting / trim guides drawn on each page.
    GuideStyle, "GUIDES",
    [
        None => ("none", "none"),
        Border => ("border", "border"),
        Corners => ("corners", "corners"),
    ]
);

option_enum!(
    /// Page position labels.
    LabelStyle, "LABELS",
    [
        None => ("none", "none"),
        Coordinates => ("coordinates", "coordinates"),
    ]
);

option_enum!(
    /// Output rendering path before the `Auto` policy resolves it.
    RenderMode, "RENDER",
    [
        Auto => ("auto", "auto"),
        Vector => ("vector", "vector"),
        Raster => ("raster", "raster"),
    ]
);

option_enum!(
    /// Raster resolution choice.
    Dpi, "DPI",
    [
        P150 => ("150", "p150"),
        P200 => ("200", "p200"),
        P300 => ("300", "p300"),
    ]
);

impl Dpi {
    /// The numeric resolution.
    pub fn value(self) -> u16 {
        match self {
            Dpi::P150 => 150,
            Dpi::P200 => 200,
            Dpi::P300 => 300,
        }
    }

    /// The value that `value_parser` should accept.
    pub fn parse_value(s: &str) -> Option<Dpi> {
        match s.trim() {
            "150" => Some(Dpi::P150),
            "200" => Some(Dpi::P200),
            "300" => Some(Dpi::P300),
            _ => None,
        }
    }
}

option_enum!(
    /// Whether `/CreationDate` is written. `Now` breaks byte determinism.
    CreationDate, "CREATION_DATE",
    [
        None => ("none", "none"),
        Now => ("now", "now"),
    ]
);
option_enum!(
    /// Whether the PDF asks compliant viewers to print at 100 %.
    PrintScaling, "PRINT_SCALING",
    [
        None => ("none", "none"),
        Viewer => ("viewer", "viewer"),
    ]
);

option_enum!(
    /// Quarter-turn applied to the decoded source.
    Rotation, "ROTATE",
    [
        D0 => ("0", "d0"),
        D90 => ("90", "d90"),
        D180 => ("180", "d180"),
        D270 => ("270", "d270"),
    ]
);

impl Rotation {
    /// The angle in degrees.
    pub fn degrees(self) -> u16 {
        match self {
            Rotation::D0 => 0,
            Rotation::D90 => 90,
            Rotation::D180 => 180,
            Rotation::D270 => 270,
        }
    }

    /// True when the rotation swaps width and height.
    pub fn swaps_axes(self) -> bool {
        matches!(self, Rotation::D90 | Rotation::D270)
    }
}

option_enum!(
    /// Whether the EXIF orientation tag is honoured.
    ExifOrientation, "EXIF_ORIENTATION",
    [
        Apply => ("apply", "apply"),
        Ignore => ("ignore", "ignore"),
    ]
);

option_enum!(
    /// Progress rendering style on stderr.
    ProgressMode, "PROGRESS",
    [
        Auto => ("auto", "auto"),
        Off => ("off", "off"),
        Tty => ("tty", "tty"),
        Plain => ("plain", "plain"),
    ]
);

// ---------------------------------------------------------------------------
// small value types
// ---------------------------------------------------------------------------

/// Paper size in millimetres, optionally tagged with the preset it came from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaperSpec {
    /// Sheet width in mm, before the orientation swap.
    pub w_mm: f64,
    /// Sheet height in mm, before the orientation swap.
    pub h_mm: f64,
    /// The preset id when the user picked one of `a4`, `a3`, `letter`, `legal`.
    pub preset: Option<&'static str>,
}

impl PaperSpec {
    /// A paper preset by id (`a4`, `a3`, `letter`, `legal`).
    pub fn preset(id: &'static str) -> Option<PaperSpec> {
        let p = paper_preset(id)?;
        Some(PaperSpec { w_mm: p.width_mm, h_mm: p.height_mm, preset: Some(p.id) })
    }

    /// A custom sheet size.
    pub fn custom(w_mm: f64, h_mm: f64) -> PaperSpec {
        PaperSpec { w_mm, h_mm, preset: None }
    }

    /// The preset id, but only when the stored dimensions still match it.
    pub fn preset_id(&self) -> Option<&'static str> {
        let id = self.preset?;
        match paper_preset(id) {
            Some(p) if (p.width_mm - self.w_mm).abs() < 1e-9 && (p.height_mm - self.h_mm).abs() < 1e-9 => Some(id),
            _ => None,
        }
    }

    /// Serialise as `a4` or `210x297`.
    pub fn to_token(self) -> String {
        match self.preset_id() {
            Some(id) => id.to_string(),
            None => format!("{}x{}", trim_f64(self.w_mm), trim_f64(self.h_mm)),
        }
    }

    /// Parse `a4` / `A3` / `210x297`.
    pub fn parse_token(s: &str) -> Result<PaperSpec, String> {
        let t = s.trim();
        if let Some(spec) = PAPER_PRESETS.iter().find(|p| p.id.eq_ignore_ascii_case(t)) {
            return Ok(PaperSpec {
                w_mm: spec.width_mm,
                h_mm: spec.height_mm,
                preset: Some(spec.id),
            });
        }
        let (a, b) = t
            .split_once(|c| c == 'x' || c == 'X')
            .ok_or_else(|| format!("invalid paper '{s}': use a4, a3, letter, legal or WxH in mm"))?;
        let w: f64 = a.trim().parse().map_err(|_| {
            format!("invalid paper '{s}': use a4, a3, letter, legal or WxH in mm")
        })?;
        let h: f64 = b.trim().parse().map_err(|_| {
            format!("invalid paper '{s}': use a4, a3, letter, legal or WxH in mm")
        })?;
        Ok(PaperSpec::custom(w, h))
    }
}

impl Default for PaperSpec {
    fn default() -> Self {
        PaperSpec::preset("a4").expect("a4 preset exists")
    }
}

/// How the assembled poster size is specified.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SizeSpec {
    /// Poster width in sheets; the height follows from the image aspect.
    WidthSheets(f64),
    /// Poster height in sheets; the width follows from the image aspect.
    HeightSheets(f64),
    /// Explicit size in [`ModernOptions::size_unit`] units.
    Size {
        /// Width.
        w: f64,
        /// Height.
        h: f64,
    },
}

impl Default for SizeSpec {
    fn default() -> Self {
        SizeSpec::WidthSheets(5.0)
    }
}

impl SizeSpec {
    /// Serialise as `width_sheets:5` / `height_sheets:3` / `size:1050x787.5`.
    pub fn to_token(self) -> String {
        match self {
            SizeSpec::WidthSheets(n) => format!("width_sheets:{}", trim_f64(n)),
            SizeSpec::HeightSheets(n) => format!("height_sheets:{}", trim_f64(n)),
            SizeSpec::Size { w, h } => format!("size:{}x{}", trim_f64(w), trim_f64(h)),
        }
    }

    /// Parse [`SizeSpec::to_token`].
    pub fn parse_token(s: &str) -> Result<SizeSpec, String> {
        let (kind, rest) = s
            .split_once(':')
            .ok_or_else(|| format!("invalid size_spec '{s}'"))?;
        match kind {
            "width_sheets" => Ok(SizeSpec::WidthSheets(parse_f64(rest)?)),
            "height_sheets" => Ok(SizeSpec::HeightSheets(parse_f64(rest)?)),
            "size" => {
                let (a, b) = rest
                    .split_once('x')
                    .ok_or_else(|| format!("invalid size_spec '{s}': expected size:WxH"))?;
                Ok(SizeSpec::Size { w: parse_f64(a)?, h: parse_f64(b)? })
            }
            _ => Err(format!("invalid size_spec '{s}'")),
        }
    }
}

/// A crop rectangle in *source pixels*, before any rotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CropRectPx {
    /// Left edge, inclusive.
    pub x1: i64,
    /// Top edge, inclusive.
    pub y1: i64,
    /// Right edge, exclusive.
    pub x2: i64,
    /// Bottom edge, exclusive.
    pub y2: i64,
}

impl CropRectPx {
    /// Serialise as `x1,y1,x2,y2`.
    pub fn to_token(self) -> String {
        format!("{},{},{},{}", self.x1, self.y1, self.x2, self.y2)
    }

    /// Parse four comma-separated integers.
    pub fn parse_token(s: &str) -> Result<CropRectPx, String> {
        let parts: Vec<&str> = s.split(',').map(str::trim).collect();
        if parts.len() != 4 {
            return Err(format!("invalid crop '{s}': expected 4 numbers in 'X1,Y1,X2,Y2'"));
        }
        let mut v = [0i64; 4];
        for (i, p) in parts.iter().enumerate() {
            v[i] = p.parse().map_err(|_| format!("invalid crop '{s}': expected 4 numbers in 'X1,Y1,X2,Y2'"))?;
        }
        Ok(CropRectPx { x1: v[0], y1: v[1], x2: v[2], y2: v[3] })
    }
}

// ---------------------------------------------------------------------------
// option keys
// ---------------------------------------------------------------------------

/// Canonical option key order: render, tone, colour, layout, output, input, UX.
///
/// Preset files, `--print-summary` and the golden generator all use this order,
/// so it is part of the public contract.
pub const OPTION_NAMES: &[&str] = &[
    // render & screen
    "screen", "stagger", "halo_cells", "pattern", "nominal_dot_mm", "coverage", "grid_phase_mm",
    // tone & tone band
    "min_raster", "max_raster", "tone_curve", "tone_exponent", "tone_log_k", "invert_tone",
    "sampling", "dither", "dither_strength", "dither_levels", "dither_block", "line_aspect",
    "mosaic_contrast",
    // colour
    "ink", "ink_color", "fg_color", "bg_color", "dual_softness", "paper_color", "theme", "shading",
    "shading_strength", "invert_colors",
    // layout & assembly
    "paper", "orientation", "size_spec", "size_unit", "fit", "align_h", "align_v", "margin_mm",
    "overlap_mode", "overlap_mm", "bleed_mm", "guides", "guide_width_pt", "corner_mark_mm",
    "guide_color", "labels", "label_size_pt", "label_offset_mm", "page_warn", "page_limit",
    // output & quality
    "render", "auto_raster_cells", "dpi", "jpeg_quality", "compress", "title", "creator",
    "creation_date", "print_scaling", "atomic", "strict",
    // input preparation
    "crop", "rotate", "exif_orientation", "brightness", "contrast", "saturation", "alpha_threshold",
    "resample", "resample_dpi",
    // UX / job control
    "preset", "save_preset", "delete_preset", "list_presets", "preset_dir", "progress", "quiet",
    "print_summary",
];

/// The UX / job-control subset, which the GUI settings file deliberately omits.
pub const UX_OPTION_NAMES: &[&str] = &[
    "preset", "save_preset", "delete_preset", "list_presets", "preset_dir", "progress", "quiet",
    "print_summary",
];

/// The default preset directory: `%APPDATA%\rasterbator\presets` on Windows,
/// `$XDG_CONFIG_HOME/rasterbator/presets` (or `$HOME/.config/…`) elsewhere.
///
/// Read from the environment so no new dependency is needed.
pub fn default_preset_dir() -> PathBuf {
    if cfg!(windows) {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::var_os("USERPROFILE").map_or_else(PathBuf::new, |h| PathBuf::from(h).join("AppData").join("Roaming")));
        base.join("rasterbator").join("presets")
    } else {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("rasterbator").join("presets")
    }
}

/// CLI spellings that differ from the canonical option key (amendment A-6).
///
/// `--dot-size` keeps the legacy spelling for the same physical quantity, and
/// the four ink/colour flags collapse into the extended `--color` value.
const FLAG_OVERRIDES: &[(&str, &str)] = &[
    ("nominal_dot_mm", "dot-size"),
    ("ink", "color"),
    ("ink_color", "color"),
    ("fg_color", "color"),
    ("bg_color", "color"),
];

/// The CLI long flag for an option key.
///
/// `--dot-size` keeps the legacy spelling for the same physical quantity, and
/// the four ink/colour fields collapse into the documented extended `--color`
/// value (`fixed:RRGGBB | average | grayscale | dual:RRGGBB/RRGGBB`).
/// Every other option is its own flag, spelled with dashes.
pub fn cli_flag(key: &str) -> String {
    for (k, f) in FLAG_OVERRIDES {
        if *k == key {
            return (*f).to_string();
        }
    }
    if OPTION_NAMES.contains(&key) {
        return key.replace('_', "-");
    }
    "unknown".to_string()
}

/// Minimum accepted preset / theme name length class: portable filenames only.
pub fn is_portable_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 48
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

// ---------------------------------------------------------------------------
// options
// ---------------------------------------------------------------------------

/// Every user-facing choice of a modern-mode job.
///
/// See `IMPLEMENTATION.md` and the module documentation of the validation
/// module for the rules attached to each row; the defaults below are the
/// documented ones.
#[derive(Clone, Debug, PartialEq)]
pub struct ModernOptions {
    // ---- render & screen ------------------------------------------------
    /// Screen family (lattice, hex, plain).
    pub screen: ScreenFamily,
    /// Half-pitch row stagger of a lattice screen.
    pub stagger: Stagger,
    /// Extra cell rows/columns drawn on each side of a page window.
    pub halo_cells: u32,
    /// Shape emitted per cell.
    pub pattern: Pattern,
    /// Nominal dot size / grid pitch, in millimetres (flag `--dot-size`).
    pub nominal_dot_mm: f64,
    /// Maximum dot coverage relative to the cell.
    pub coverage: Coverage,
    /// Lattice phase shift in millimetres, `[x, y]`.
    pub grid_phase_mm: (f64, f64),

    // ---- tone & tone band ----------------------------------------------
    /// Dot size fraction at full white.
    pub min_raster: f64,
    /// Dot size fraction at full black.
    pub max_raster: f64,
    /// Response curve applied to darkness.
    pub tone_curve: ToneCurve,
    /// Exponent for the exponential curve.
    pub tone_exponent: f64,
    /// Strength `k` for the logarithmic curve.
    pub tone_log_k: f64,
    /// Flip the dot-size mapping.
    pub invert_tone: bool,
    /// Per-cell sampling method.
    pub sampling: Sampling,
    /// Quantisation / error-diffusion algorithm.
    pub dither: Dither,
    /// How much of the quantised value is mixed into the continuous tone.
    pub dither_strength: f64,
    /// Quantiser denominator (`2..=64`).
    pub dither_levels: u32,
    /// Ordered-dither granularity in cells.
    pub dither_block: u32,
    /// Bar half-thickness as a fraction of the half-length, for line patterns.
    pub line_aspect: f64,
    /// Mosaic high-contrast gain about mid grey.
    pub mosaic_contrast: f64,

    // ---- colour ---------------------------------------------------------
    /// Where the ink colour of a cell comes from.
    pub ink: InkMode,
    /// Ink colour for [`InkMode::Fixed`].
    pub ink_color: Rgb,
    /// Dual-tone foreground (dark).
    pub fg_color: Rgb,
    /// Dual-tone background (light).
    pub bg_color: Rgb,
    /// Dual-tone softness: `1.0` is the hard two-level decision.
    pub dual_softness: f64,
    /// Page background / paper colour; transparent pixels resolve to this.
    pub paper_color: Rgb,
    /// Named colour theme, if any.
    pub theme: String,
    /// Modulate fixed ink by the local tone.
    pub shading: bool,
    /// How far shading modulates the ink.
    pub shading_strength: f64,
    /// Invert the emitted colours.
    pub invert_colors: bool,

    // ---- layout & assembly ---------------------------------------------
    /// Sheet size.
    pub paper: PaperSpec,
    /// Sheet orientation.
    pub orientation: Orientation,
    /// Assembled poster size.
    pub size_spec: SizeSpec,
    /// Unit of [`SizeSpec::Size`].
    pub size_unit: SizeUnit,
    /// Contain / cover / stretch.
    pub fit: Fit,
    /// Horizontal placement inside the poster.
    pub align_h: Align,
    /// Vertical placement inside the poster.
    pub align_v: Align,
    /// Cutting margin in millimetres.
    pub margin_mm: f64,
    /// Whether the overlap strip is extra paper or shared content.
    pub overlap_mode: OverlapMode,
    /// Overlap / bleed between sheets, in millimetres.
    pub overlap_mm: f64,
    /// Extra trim-less area outside the trim box, in millimetres.
    pub bleed_mm: f64,
    /// Cutting guides.
    pub guides: GuideStyle,
    /// Guide stroke width in points.
    pub guide_width_pt: f64,
    /// Corner mark arm length in millimetres.
    pub corner_mark_mm: f64,
    /// Guide and label colour.
    pub guide_color: Rgb,
    /// Page position labels.
    pub labels: LabelStyle,
    /// Label size in points.
    pub label_size_pt: f64,
    /// Label inset from the sheet edge, in millimetres.
    pub label_offset_mm: f64,
    /// Page count above which a warning is raised (`0` disables).
    pub page_warn: u32,
    /// Page count above which the job is rejected (`0` disables).
    pub page_limit: u32,

    // ---- output & quality ----------------------------------------------
    /// Output rendering path before `Auto` resolves.
    pub render: RenderMode,
    /// Cell count above which `Auto` switches to raster (`0` never does).
    pub auto_raster_cells: u32,
    /// Raster resolution.
    pub dpi: Dpi,
    /// JPEG quality of the raster path.
    pub jpeg_quality: u8,
    /// Cap dpi and quality for a smaller file.
    pub compress: bool,
    /// PDF `/Title` metadata.
    pub title: String,
    /// PDF `/Creator` metadata.
    pub creator: String,
    /// Whether `/CreationDate` is written.
    pub creation_date: CreationDate,
    /// Whether the PDF asks compliant viewers to print at 100 %.
    pub print_scaling: PrintScaling,
    /// Write through a temporary file and rename.
    pub atomic: bool,
    /// Promote every warning to a blocking error.
    pub strict: bool,

    // ---- input preparation ---------------------------------------------
    /// Crop rectangle in source pixels.
    pub crop: Option<CropRectPx>,
    /// Quarter-turn applied before sizing.
    pub rotate: Rotation,
    /// Whether the EXIF orientation tag is applied.
    pub exif_orientation: ExifOrientation,
    /// Brightness adjustment in percent (`-100..=100`).
    pub brightness: f64,
    /// Contrast adjustment in percent (`-100..=100`).
    pub contrast: f64,
    /// Saturation adjustment in percent (`-100..=100`).
    pub saturation: f64,
    /// Pixels with `a < threshold` become [`ModernOptions::paper_color`].
    pub alpha_threshold: u8,
    /// Box-filter the source down to the cell grid first.
    pub resample: bool,
    /// Resolution the optional resample targets.
    pub resample_dpi: u16,

    // ---- UX / job control ----------------------------------------------
    /// Preset to load before rendering.
    pub preset: Option<String>,
    /// Preset to write after rendering.
    pub save_preset: Option<String>,
    /// Preset to delete.
    pub delete_preset: Option<String>,
    /// List the preset store and exit.
    pub list_presets: bool,
    /// Preset store directory.
    pub preset_dir: PathBuf,
    /// Progress rendering style.
    pub progress: ProgressMode,
    /// Suppress informational output.
    pub quiet: bool,
    /// Write a JSON summary to stdout.
    pub print_summary: bool,
}

impl Default for ModernOptions {
    fn default() -> Self {
        ModernOptions {
            // render & screen
            screen: ScreenFamily::Lattice,
            stagger: Stagger::Off,
            halo_cells: 1,
            pattern: Pattern::Dot,
            nominal_dot_mm: 10.0,
            coverage: Coverage::FullCell,
            grid_phase_mm: (0.0, 0.0),
            // tone & tone band
            min_raster: 0.0,
            max_raster: 1.0,
            tone_curve: ToneCurve::None,
            tone_exponent: 2.2,
            tone_log_k: 8.0,
            invert_tone: false,
            sampling: Sampling::BoxAverage,
            dither: Dither::None,
            dither_strength: 1.0,
            dither_levels: 8,
            dither_block: 1,
            line_aspect: 0.5,
            mosaic_contrast: 1.0,
            // colour
            ink: InkMode::Fixed,
            ink_color: Rgb::BLACK,
            fg_color: Rgb::BLACK,
            bg_color: Rgb::WHITE,
            dual_softness: 1.0,
            paper_color: Rgb::WHITE,
            theme: String::new(),
            shading: false,
            shading_strength: 0.35,
            invert_colors: false,
            // layout & assembly
            paper: PaperSpec::default(),
            orientation: Orientation::Portrait,
            size_spec: SizeSpec::default(),
            size_unit: SizeUnit::Mm,
            fit: Fit::Contain,
            align_h: Align::Center,
            align_v: Align::Center,
            margin_mm: 10.0,
            overlap_mode: OverlapMode::Margin,
            overlap_mm: 0.0,
            bleed_mm: 0.0,
            guides: GuideStyle::Corners,
            guide_width_pt: 0.5,
            corner_mark_mm: 4.0,
            guide_color: Rgb(128, 128, 128),
            labels: LabelStyle::None,
            label_size_pt: 9.0,
            label_offset_mm: 8.0,
            page_warn: 64,
            page_limit: 1024,
            // output & quality
            render: RenderMode::Auto,
            auto_raster_cells: 200_000,
            dpi: Dpi::P300,
            jpeg_quality: 85,
            compress: false,
            title: String::new(),
            creator: "rasterbator-rs modern".to_string(),
            creation_date: CreationDate::None,
            print_scaling: PrintScaling::None,
            atomic: true,
            strict: false,
            // input preparation
            crop: None,
            rotate: Rotation::D0,
            exif_orientation: ExifOrientation::Apply,
            brightness: 0.0,
            contrast: 0.0,
            saturation: 0.0,
            alpha_threshold: 128,
            resample: true,
            resample_dpi: 300,
            // UX / job control
            preset: None,
            save_preset: None,
            delete_preset: None,
            list_presets: false,
            preset_dir: default_preset_dir(),
            progress: ProgressMode::Auto,
            quiet: false,
            print_summary: false,
        }
    }
}

impl ModernOptions {
    /// The canonical option key order.
    pub fn option_names() -> &'static [&'static str] {
        OPTION_NAMES
    }

    /// The default preset store directory.
    pub fn default_preset_dir() -> PathBuf {
        default_preset_dir()
    }

    /// Look up the current value of an option by key, as a display string.
    ///
    /// This is what `--print-summary`, the CLI help footers and the GUI status
    /// strip use; it never diverges from [`Self::to_text`] because both call
    /// the same `to_token` implementations.
    pub fn value_of(&self, key: &str) -> Option<String> {
        Some(match key {
            "screen" => self.screen.cli_str().into(),
            "stagger" => self.stagger.cli_str().into(),
            "halo_cells" => self.halo_cells.to_string(),
            "pattern" => self.pattern.cli_str().into(),
            "nominal_dot_mm" => trim_f64(self.nominal_dot_mm),
            "coverage" => self.coverage.cli_str().into(),
            "grid_phase_mm" => format!("{},{}", trim_f64(self.grid_phase_mm.0), trim_f64(self.grid_phase_mm.1)),
            "min_raster" => trim_f64(self.min_raster),
            "max_raster" => trim_f64(self.max_raster),
            "tone_curve" => self.tone_curve.cli_str().into(),
            "tone_exponent" => trim_f64(self.tone_exponent),
            "tone_log_k" => trim_f64(self.tone_log_k),
            "invert_tone" => self.invert_tone.to_string(),
            "sampling" => self.sampling.cli_str().into(),
            "dither" => self.dither.cli_str().into(),
            "dither_strength" => trim_f64(self.dither_strength),
            "dither_levels" => self.dither_levels.to_string(),
            "dither_block" => self.dither_block.to_string(),
            "line_aspect" => trim_f64(self.line_aspect),
            "mosaic_contrast" => trim_f64(self.mosaic_contrast),
            "ink" => self.ink.cli_str().into(),
            "ink_color" => self.ink_color.to_hex(),
            "fg_color" => self.fg_color.to_hex(),
            "bg_color" => self.bg_color.to_hex(),
            "dual_softness" => trim_f64(self.dual_softness),
            "paper_color" => self.paper_color.to_hex(),
            "theme" => self.theme.clone(),
            "shading" => self.shading.to_string(),
            "shading_strength" => trim_f64(self.shading_strength),
            "invert_colors" => self.invert_colors.to_string(),
            "paper" => self.paper.to_token(),
            "orientation" => self.orientation.cli_str().into(),
            "size_spec" => self.size_spec.to_token(),
            "size_unit" => self.size_unit.cli_str().into(),
            "fit" => self.fit.cli_str().into(),
            "align_h" => self.align_h.cli_str().into(),
            "align_v" => self.align_v.cli_str().into(),
            "margin_mm" => trim_f64(self.margin_mm),
            "overlap_mode" => self.overlap_mode.cli_str().into(),
            "overlap_mm" => trim_f64(self.overlap_mm),
            "bleed_mm" => trim_f64(self.bleed_mm),
            "guides" => self.guides.cli_str().into(),
            "guide_width_pt" => trim_f64(self.guide_width_pt),
            "corner_mark_mm" => trim_f64(self.corner_mark_mm),
            "guide_color" => self.guide_color.to_hex(),
            "labels" => self.labels.cli_str().into(),
            "label_size_pt" => trim_f64(self.label_size_pt),
            "label_offset_mm" => trim_f64(self.label_offset_mm),
            "page_warn" => self.page_warn.to_string(),
            "page_limit" => self.page_limit.to_string(),
            "render" => self.render.cli_str().into(),
            "auto_raster_cells" => self.auto_raster_cells.to_string(),
            "dpi" => self.dpi.cli_str().into(),
            "jpeg_quality" => self.jpeg_quality.to_string(),
            "compress" => self.compress.to_string(),
            "title" => self.title.clone(),
            "creator" => self.creator.clone(),
            "creation_date" => self.creation_date.cli_str().into(),
            "print_scaling" => self.print_scaling.cli_str().into(),
            "atomic" => self.atomic.to_string(),
            "strict" => self.strict.to_string(),
            "crop" => match self.crop {
                Some(c) => c.to_token(),
                None => "none".into(),
            },
            "rotate" => self.rotate.cli_str().into(),
            "exif_orientation" => self.exif_orientation.cli_str().into(),
            "brightness" => trim_f64(self.brightness),
            "contrast" => trim_f64(self.contrast),
            "saturation" => trim_f64(self.saturation),
            "alpha_threshold" => self.alpha_threshold.to_string(),
            "resample" => self.resample.to_string(),
            "resample_dpi" => self.resample_dpi.to_string(),
            "preset" => opt_str(self.preset.as_deref()),
            "save_preset" => opt_str(self.save_preset.as_deref()),
            "delete_preset" => opt_str(self.delete_preset.as_deref()),
            "list_presets" => self.list_presets.to_string(),
            "preset_dir" => self.preset_dir.to_string_lossy().into_owned(),
            "progress" => self.progress.cli_str().into(),
            "quiet" => self.quiet.to_string(),
            "print_summary" => self.print_summary.to_string(),
            _ => return None,
        })
    }

    /// Serialise to the preset/settings text format.
    ///
    /// One `key=value` line per option in [`OPTION_NAMES`] order, LF separated,
    /// with a `rasterbator-options v1` header.
    pub fn to_text(&self) -> String {
        self.to_text_filtered(|_| true)
    }

    /// Serialise without the UX group (the GUI settings file).
    pub fn to_settings_text(&self) -> String {
        self.to_text_filtered(|k| !UX_OPTION_NAMES.contains(&k))
    }

    fn to_text_filtered(&self, keep: impl Fn(&str) -> bool) -> String {
        let mut s = String::from("rasterbator-options v1\n");
        for k in OPTION_NAMES {
            if !keep(k) {
                continue;
            }
            if let Some(v) = self.value_of(k) {
                s.push_str(k);
                s.push('=');
                s.push_str(&escape_value(&v));
                s.push('\n');
            }
        }
        s
    }

    /// Parse the preset/settings text format.
    ///
    /// Unknown keys, malformed values and out-of-order keys are blocking
    /// errors that name the offending line. Missing keys keep their default,
    /// which is what lets the GUI settings file omit the UX group.
    pub fn from_text(text: &str) -> Result<ModernOptions, crate::modern::diagnostics::Diagnostic> {
        use crate::modern::diagnostics::{DiagCode, Diagnostic};

        let mut opts = ModernOptions::default();
        let mut last = 0usize;
        let mut saw_header = false;
        let mut seen: BTreeMap<String, ()> = BTreeMap::new();

        for (idx, raw) in text.lines().enumerate() {
            let lineno = idx + 1;
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if lineno == 1 {
                if line.trim() == "rasterbator-options v1" {
                    saw_header = true;
                    continue;
                }
                return Err(Diagnostic::error(
                    DiagCode::PresetInvalid,
                    format!("line {lineno}: missing version header, expected 'rasterbator-options v1'"),
                )
                .with_hint("preset files start with that exact line"));
            }
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('=').ok_or_else(|| {
                Diagnostic::error(
                    DiagCode::PresetInvalid,
                    format!("line {lineno}: expected 'key=value', found '{line}'"),
                )
            })?;
            let key = key.trim();
            let pos = OPTION_NAMES.iter().position(|k| *k == key).ok_or_else(|| {
                Diagnostic::error(
                    DiagCode::PresetInvalid,
                    format!("line {lineno}: unknown option key '{key}'"),
                )
                .with_hint(format!("known keys: {}", OPTION_NAMES.join(", ")))
            })?;
            if pos < last {
                return Err(Diagnostic::error(
                    DiagCode::PresetInvalid,
                    format!("line {lineno}: option '{key}' is out of order"),
                )
                .with_hint("keys must follow option_names() order"));
            }
            if seen.insert(key.to_string(), ()).is_some() {
                return Err(Diagnostic::error(
                    DiagCode::PresetInvalid,
                    format!("line {lineno}: option '{key}' appears more than once"),
                ));
            }
            last = pos;
            let value = unescape_value(value);
            apply_key(&mut opts, key, &value).map_err(|m| {
                Diagnostic::error(
                    DiagCode::PresetInvalid,
                    format!("line {lineno}: option '{key}': {m}"),
                )
            })?;
        }
        if !saw_header {
            return Err(Diagnostic::error(
                DiagCode::PresetInvalid,
                "missing version header, expected 'rasterbator-options v1'",
            ));
        }
        Ok(opts)
    }
}

fn opt_str(v: Option<&str>) -> String {
    match v {
        Some(s) => s.to_string(),
        None => "none".to_string(),
    }
}

fn parse_f64(s: &str) -> Result<f64, String> {
    let t = s.trim();
    t.parse::<f64>().map_err(|_| format!("invalid number '{t}'"))
}

fn parse_u32(s: &str) -> Result<u32, String> {
    s.trim().parse::<u32>().map_err(|_| format!("invalid unsigned integer '{}'", s.trim()))
}

fn parse_u8(s: &str) -> Result<u8, String> {
    s.trim().parse::<u8>().map_err(|_| format!("invalid byte '{}'", s.trim()))
}

fn parse_u16(s: &str) -> Result<u16, String> {
    s.trim().parse::<u16>().map_err(|_| format!("invalid unsigned integer '{}'", s.trim()))
}

fn parse_bool(s: &str) -> Result<bool, String> {
    match s.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("invalid boolean '{other}', expected true or false")),
    }
}

fn parse_rgb(s: &str) -> Result<Rgb, String> {
    Rgb::parse(s)
}

fn parse_opt_string(s: &str) -> Result<Option<String>, String> {
    if s == "none" {
        Ok(None)
    } else {
        Ok(Some(s.to_string()))
    }
}

fn enum_err(what: &'static str, s: &str) -> String {
    format!("invalid {what} '{s}'")
}

fn escape_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

fn unescape_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Shortest round-trip float text, with `-0` normalised to `0`.
pub fn trim_f64(v: f64) -> String {
    if v == 0.0 {
        "0".to_string()
    } else {
        format!("{v}")
    }
}

fn parse_pair_f64(s: &str) -> Result<(f64, f64), String> {
    let parts: Vec<&str> = s.split(',').map(str::trim).collect();
    if parts.len() != 2 {
        return Err(format!("invalid pair '{s}': expected 'X,Y'"));
    }
    Ok((parse_f64(parts[0])?, parse_f64(parts[1])?))
}

#[allow(clippy::too_many_lines)]
fn apply_key(o: &mut ModernOptions, key: &str, value: &str) -> Result<(), String> {
    match key {
        "screen" => o.screen = ScreenFamily::parse_cli(value).ok_or_else(|| enum_err("screen", value))?,
        "stagger" => o.stagger = Stagger::parse_cli(value).ok_or_else(|| enum_err("stagger", value))?,
        "halo_cells" => o.halo_cells = parse_u32(value)?,
        "pattern" => o.pattern = Pattern::parse_cli(value).ok_or_else(|| enum_err("pattern", value))?,
        "nominal_dot_mm" => o.nominal_dot_mm = parse_f64(value)?,
        "coverage" => o.coverage = Coverage::parse_cli(value).ok_or_else(|| enum_err("coverage", value))?,
        "grid_phase_mm" => o.grid_phase_mm = parse_pair_f64(value)?,
        "min_raster" => o.min_raster = parse_f64(value)?,
        "max_raster" => o.max_raster = parse_f64(value)?,
        "tone_curve" => o.tone_curve = ToneCurve::parse_cli(value).ok_or_else(|| enum_err("tone curve", value))?,
        "tone_exponent" => o.tone_exponent = parse_f64(value)?,
        "tone_log_k" => o.tone_log_k = parse_f64(value)?,
        "invert_tone" => o.invert_tone = parse_bool(value)?,
        "sampling" => o.sampling = Sampling::parse_cli(value).ok_or_else(|| enum_err("sampling", value))?,
        "dither" => o.dither = Dither::parse_cli(value).ok_or_else(|| enum_err("dither", value))?,
        "dither_strength" => o.dither_strength = parse_f64(value)?,
        "dither_levels" => o.dither_levels = parse_u32(value)?,
        "dither_block" => o.dither_block = parse_u32(value)?,
        "line_aspect" => o.line_aspect = parse_f64(value)?,
        "mosaic_contrast" => o.mosaic_contrast = parse_f64(value)?,
        "ink" => o.ink = InkMode::parse_cli(value).ok_or_else(|| enum_err("ink mode", value))?,
        "ink_color" => o.ink_color = parse_rgb(value)?,
        "fg_color" => o.fg_color = parse_rgb(value)?,
        "bg_color" => o.bg_color = parse_rgb(value)?,
        "dual_softness" => o.dual_softness = parse_f64(value)?,
        "paper_color" => o.paper_color = parse_rgb(value)?,
        "theme" => o.theme = value.to_string(),
        "shading" => o.shading = parse_bool(value)?,
        "shading_strength" => o.shading_strength = parse_f64(value)?,
        "invert_colors" => o.invert_colors = parse_bool(value)?,
        "paper" => o.paper = PaperSpec::parse_token(value)?,
        "orientation" => o.orientation = Orientation::parse_cli(value).ok_or_else(|| enum_err("orientation", value))?,
        "size_spec" => o.size_spec = SizeSpec::parse_token(value)?,
        "size_unit" => o.size_unit = SizeUnit::parse_cli(value).ok_or_else(|| enum_err("size unit", value))?,
        "fit" => o.fit = Fit::parse_cli(value).ok_or_else(|| enum_err("fit", value))?,
        "align_h" => o.align_h = Align::parse_cli(value).ok_or_else(|| enum_err("align", value))?,
        "align_v" => o.align_v = Align::parse_cli(value).ok_or_else(|| enum_err("align", value))?,
        "margin_mm" => o.margin_mm = parse_f64(value)?,
        "overlap_mode" => o.overlap_mode = OverlapMode::parse_cli(value).ok_or_else(|| enum_err("overlap mode", value))?,
        "overlap_mm" => o.overlap_mm = parse_f64(value)?,
        "bleed_mm" => o.bleed_mm = parse_f64(value)?,
        "guides" => o.guides = GuideStyle::parse_cli(value).ok_or_else(|| enum_err("guides", value))?,
        "guide_width_pt" => o.guide_width_pt = parse_f64(value)?,
        "corner_mark_mm" => o.corner_mark_mm = parse_f64(value)?,
        "guide_color" => o.guide_color = parse_rgb(value)?,
        "labels" => o.labels = LabelStyle::parse_cli(value).ok_or_else(|| enum_err("labels", value))?,
        "label_size_pt" => o.label_size_pt = parse_f64(value)?,
        "label_offset_mm" => o.label_offset_mm = parse_f64(value)?,
        "page_warn" => o.page_warn = parse_u32(value)?,
        "page_limit" => o.page_limit = parse_u32(value)?,
        "render" => o.render = RenderMode::parse_cli(value).ok_or_else(|| enum_err("render mode", value))?,
        "auto_raster_cells" => o.auto_raster_cells = parse_u32(value)?,
        "dpi" => o.dpi = Dpi::parse_value(value).ok_or_else(|| enum_err("dpi", value))?,
        "jpeg_quality" => o.jpeg_quality = parse_u8(value)?,
        "compress" => o.compress = parse_bool(value)?,
        "title" => o.title = value.to_string(),
        "creator" => o.creator = value.to_string(),
        "creation_date" => o.creation_date = CreationDate::parse_cli(value).ok_or_else(|| enum_err("creation date", value))?,
        "print_scaling" => o.print_scaling = PrintScaling::parse_cli(value).ok_or_else(|| enum_err("print scaling", value))?,
        "atomic" => o.atomic = parse_bool(value)?,
        "strict" => o.strict = parse_bool(value)?,
        "crop" => o.crop = if value == "none" { None } else { Some(CropRectPx::parse_token(value)?) },
        "rotate" => o.rotate = Rotation::parse_cli(value).ok_or_else(|| enum_err("rotation", value))?,
        "exif_orientation" => o.exif_orientation = ExifOrientation::parse_cli(value).ok_or_else(|| enum_err("exif orientation", value))?,
        "brightness" => o.brightness = parse_f64(value)?,
        "contrast" => o.contrast = parse_f64(value)?,
        "saturation" => o.saturation = parse_f64(value)?,
        "alpha_threshold" => o.alpha_threshold = parse_u8(value)?,
        "resample" => o.resample = parse_bool(value)?,
        "resample_dpi" => o.resample_dpi = parse_u16(value)?,
        "preset" => o.preset = parse_opt_string(value)?,
        "save_preset" => o.save_preset = parse_opt_string(value)?,
        "delete_preset" => o.delete_preset = parse_opt_string(value)?,
        "list_presets" => o.list_presets = parse_bool(value)?,
        "preset_dir" => o.preset_dir = PathBuf::from(value),
        "progress" => o.progress = ProgressMode::parse_cli(value).ok_or_else(|| enum_err("progress mode", value))?,
        "quiet" => o.quiet = parse_bool(value)?,
        "print_summary" => o.print_summary = parse_bool(value)?,
        _ => return Err(format!("unknown option key '{key}'")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_names_are_unique_and_cover_the_struct() {
        let mut seen = std::collections::BTreeSet::new();
        for k in OPTION_NAMES {
            assert!(seen.insert(*k), "duplicate option key {k}");
        }
        for k in UX_OPTION_NAMES {
            assert!(seen.contains(k), "{k} not in OPTION_NAMES");
        }
        // Every field is reachable through value_of().
        let o = ModernOptions::default();
        for k in OPTION_NAMES {
            assert!(o.value_of(k).is_some(), "no value for option {k}");
        }
        assert!(o.value_of("not_an_option").is_none());
    }

    #[test]
    fn defaults_match_the_specified_table() {
        let o = ModernOptions::default();
        assert_eq!(o.screen, ScreenFamily::Lattice);
        assert_eq!(o.pattern, Pattern::Dot);
        assert_eq!(o.coverage, Coverage::FullCell);
        assert_eq!(o.nominal_dot_mm, 10.0);
        assert_eq!(o.halo_cells, 1);
        assert_eq!(o.min_raster, 0.0);
        assert_eq!(o.max_raster, 1.0);
        assert_eq!(o.tone_exponent, 2.2);
        assert_eq!(o.tone_log_k, 8.0);
        assert_eq!(o.dither_levels, 8);
        assert_eq!(o.dither_block, 1);
        assert_eq!(o.line_aspect, 0.5);
        assert_eq!(o.mosaic_contrast, 1.0);
        assert_eq!(o.ink, InkMode::Fixed);
        assert_eq!(o.ink_color, Rgb::BLACK);
        assert_eq!(o.paper_color, Rgb::WHITE);
        assert_eq!(o.bg_color, Rgb::WHITE);
        assert_eq!(o.dual_softness, 1.0);
        assert_eq!(o.shading_strength, 0.35);
        assert_eq!(o.paper.to_token(), "a4");
        assert_eq!(o.size_spec, SizeSpec::WidthSheets(5.0));
        assert_eq!(o.margin_mm, 10.0);
        assert_eq!(o.overlap_mode, OverlapMode::Margin);
        assert_eq!(o.overlap_mm, 0.0);
        assert_eq!(o.guides, GuideStyle::Corners);
        assert_eq!(o.guide_width_pt, 0.5);
        assert_eq!(o.corner_mark_mm, 4.0);
        assert_eq!(o.guide_color, Rgb(128, 128, 128));
        assert_eq!(o.labels, LabelStyle::None);
        assert_eq!(o.label_size_pt, 9.0);
        assert_eq!(o.label_offset_mm, 8.0);
        assert_eq!(o.page_warn, 64);
        assert_eq!(o.page_limit, 1024);
        assert_eq!(o.render, RenderMode::Auto);
        assert_eq!(o.auto_raster_cells, 200_000);
        assert_eq!(o.dpi, Dpi::P300);
        assert_eq!(o.jpeg_quality, 85);
        assert_eq!(o.title, "");
        assert_eq!(o.creator, "rasterbator-rs modern");
        assert_eq!(o.creation_date, CreationDate::None);
        assert!(o.atomic);
        assert!(!o.strict);
        assert_eq!(o.rotate, Rotation::D0);
        assert_eq!(o.exif_orientation, ExifOrientation::Apply);
        assert_eq!(o.alpha_threshold, 128);
        assert!(o.resample);
        assert_eq!(o.resample_dpi, 300);
        assert_eq!(o.progress, ProgressMode::Auto);
    }

    #[test]
    fn enum_tables_are_total_and_consistent() {
        macro_rules! check {
            ($t:ty) => {{
                for v in <$t>::ALL {
                    let c = v.cli_str();
                    assert_eq!(<$t>::parse_cli(c), Some(*v));
                    assert_eq!(<$t>::parse_cli(&c.to_uppercase()), Some(*v));
                    let s = v.variant_name();
                    assert_eq!(<$t>::from_variant(s), Some(*v));
                }
            }};
        }
        check!(ScreenFamily);
        check!(Stagger);
        check!(Pattern);
        check!(Coverage);
        check!(ToneCurve);
        check!(Sampling);
        check!(Dither);
        check!(InkMode);
        check!(Orientation);
        check!(Fit);
        check!(Align);
        check!(SizeUnit);
        check!(OverlapMode);
        check!(GuideStyle);
        check!(LabelStyle);
        check!(RenderMode);
        check!(Dpi);
        check!(CreationDate);
        check!(Rotation);
        check!(ExifOrientation);
        check!(ProgressMode);
    }

    #[test]
    fn pattern_tables_match_the_design() {
        assert_eq!(Pattern::ALL.len(), 8);
        let cli: Vec<&str> = Pattern::ALL.iter().map(|p| p.cli_str()).collect();
        assert_eq!(
            cli,
            vec!["dot", "square", "triangle", "triangle-alt", "line-h", "line-v", "line-morph", "mosaic"]
        );
        assert!(Pattern::LineMorph.is_line());
        assert!(Pattern::Mosaic.is_square());
        assert!((Pattern::Dot.area_factor() - std::f64::consts::PI).abs() < 1e-12);
        assert_eq!(Pattern::Square.area_factor(), 4.0);
        assert!((Pattern::Triangle.area_factor() - 3.0f64.sqrt() / 2.0).abs() < 1e-12);
    }

    #[test]
    fn round_trip_default() {
        let o = ModernOptions::default();
        let text = o.to_text();
        assert!(text.starts_with("rasterbator-options v1\n"));
        assert_eq!(ModernOptions::from_text(&text).unwrap(), o);
    }

    #[test]
    fn round_trip_settings_file_keeps_defaults_for_missing_ux_keys() {
        let mut o = ModernOptions::default();
        o.screen = ScreenFamily::Hex;
        o.title = "poster".into();
        let text = o.to_settings_text();
        assert!(!text.contains("\nquiet="));
        let back = ModernOptions::from_text(&text).unwrap();
        assert_eq!(back.screen, ScreenFamily::Hex);
        assert_eq!(back.title, "poster");
        assert_eq!(back.quiet, false);
    }

    #[test]
    fn from_text_rejects_bad_input() {
        use crate::modern::diagnostics::{DiagCode, Severity};
        let bad_header = "screen=lattice\n";
        let r = ModernOptions::from_text(bad_header).unwrap_err();
        assert_eq!(r.severity, Severity::Error);
        assert!(r.message.contains("version header"), "{}", r.message);

        let unknown = "rasterbator-options v1\nnot_an_option=1\n";
        let r = ModernOptions::from_text(unknown).unwrap_err();
        assert_eq!(r.code, DiagCode::PresetInvalid);
        assert!(r.message.contains("line 2"), "{}", r.message);
        assert!(r.message.contains("not_an_option"), "{}", r.message);

        let bad_enum = "rasterbator-options v1\nscreen=triangle\n";
        let r = ModernOptions::from_text(bad_enum).unwrap_err();
        assert!(r.message.contains("line 2"), "{}", r.message);

        let bad_float = "rasterbator-options v1\nnominal_dot_mm=abc\n";
        let r = ModernOptions::from_text(bad_float).unwrap_err();
        assert!(r.message.contains("line 2"), "{}", r.message);

        let out_of_order = "rasterbator-options v1\npattern=dot\nscreen=hex\n";
        let r = ModernOptions::from_text(out_of_order).unwrap_err();
        assert!(r.message.contains("out of order"), "{}", r.message);

        let dupe = "rasterbator-options v1\nscreen=hex\nscreen=lattice\n";
        let r = ModernOptions::from_text(dupe).unwrap_err();
        assert!(r.message.contains("more than once"), "{}", r.message);
    }

    #[test]
    fn deterministic_across_runs() {
        let mut o = ModernOptions::default();
        o.preset_dir = PathBuf::from("/tmp/does-not-matter");
        assert_eq!(o.to_text(), o.to_text());
    }

    #[test]
    fn paper_tokens() {
        assert_eq!(PaperSpec::default().to_token(), "a4");
        assert_eq!(PaperSpec::custom(210.0, 297.0).to_token(), "210x297");
        assert_eq!(PaperSpec::parse_token("A3").unwrap(), PaperSpec::preset("a3").unwrap());
        assert_eq!(
            PaperSpec::parse_token("300x400").unwrap(),
            PaperSpec::custom(300.0, 400.0)
        );
        assert!(PaperSpec::parse_token("big").is_err());
        // Editing a preset dimension drops the preset tag.
        let mut p = PaperSpec::default();
        p.h_mm = 300.0;
        assert_eq!(p.to_token(), "210x300");
        assert_eq!(p.preset_id(), None);
    }

    #[test]
    fn size_and_crop_tokens() {
        assert_eq!(SizeSpec::WidthSheets(5.0).to_token(), "width_sheets:5");
        assert_eq!(
            SizeSpec::parse_token("height_sheets:3.5").unwrap(),
            SizeSpec::HeightSheets(3.5)
        );
        assert_eq!(
            SizeSpec::parse_token("size:100x50").unwrap(),
            SizeSpec::Size { w: 100.0, h: 50.0 }
        );
        assert!(SizeSpec::parse_token("nope").is_err());

        let c = CropRectPx { x1: 1, y1: 2, x2: 3, y2: 4 };
        assert_eq!(CropRectPx::parse_token(&c.to_token()).unwrap(), c);
        assert!(CropRectPx::parse_token("1,2,3").is_err());
    }

    #[test]
    fn cli_flag_table_applies_the_shared_spellings() {
        assert_eq!(cli_flag("nominal_dot_mm"), "dot-size");
        assert_eq!(cli_flag("ink"), "color");
        assert_eq!(cli_flag("ink_color"), "color");
        assert_eq!(cli_flag("margin_mm"), "margin-mm");
        assert_eq!(cli_flag("dither_strength"), "dither-strength");
        assert_eq!(cli_flag("nope"), "unknown");
        // Every option has a flag, and no two share one except the documented
        // `--color` group.
        let mut seen: std::collections::BTreeMap<String, Vec<&str>> = Default::default();
        for k in OPTION_NAMES {
            seen.entry(cli_flag(k)).or_default().push(k);
        }
        for (flag, keys) in &seen {
            if keys.len() > 1 {
                assert_eq!(*flag, "color", "unexpected shared flag {flag} for {keys:?}");
            }
        }
    }

    #[test]
    fn portable_names() {
        assert!(is_portable_name("my-preset_1"));
        assert!(!is_portable_name(""));
        assert!(!is_portable_name("has space"));
        assert!(!is_portable_name(&"x".repeat(49)));
    }

    #[test]
    fn strings_are_escaped() {
        let mut o = ModernOptions::default();
        o.title = "a\\b\nc".into();
        let text = o.to_text();
        assert!(text.contains("title=a\\\\b\\nc"), "{text}");
        assert_eq!(ModernOptions::from_text(&text).unwrap().title, "a\\b\nc");
    }

    #[test]
    fn many_option_sets_round_trip() {
        // A cheap pseudo-random sweep: 64 variants of the numeric/enum fields.
        let mut seed = 0x12345678u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..64 {
            let mut o = ModernOptions::default();
            o.screen = ScreenFamily::ALL[(next() as usize) % ScreenFamily::ALL.len()];
            o.pattern = Pattern::ALL[(next() as usize) % Pattern::ALL.len()];
            o.ink = InkMode::ALL[(next() as usize) % InkMode::ALL.len()];
            o.dither = Dither::ALL[(next() as usize) % Dither::ALL.len()];
            o.sampling = Sampling::ALL[(next() as usize) % Sampling::ALL.len()];
            o.coverage = Coverage::ALL[(next() as usize) % Coverage::ALL.len()];
            o.nominal_dot_mm = 1.0 + (next() % 499) as f64 / 10.0;
            o.mosaic_contrast = 0.1 + (next() % 79) as f64 / 10.0;
            let n1 = next() % 100; let n2 = next() % 100;
            o.grid_phase_mm = (n1 as f64 / 7.0, n2 as f64 / -7.0);
            o.ink_color = Rgb((next() % 256) as u8, (next() % 256) as u8, (next() % 256) as u8);
            o.crop = if next() % 2 == 0 {
                None
            } else {
                Some(CropRectPx { x1: 1, y1: 2, x2: 30, y2: 40 })
            };
            o.size_spec = SizeSpec::WidthSheets((next() % 90) as f64 / 7.0);
            let tag = next();
            o.title = format!("set {tag}");
            let text = o.to_text();
            assert_eq!(text, o.to_text(), "serialisation is not deterministic");
            assert_eq!(ModernOptions::from_text(&text).unwrap(), o);
        }
    }
}