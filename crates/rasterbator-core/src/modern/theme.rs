//! Named colour themes (feature C7).
//!
//! A theme is a small named pair of colours plus a pattern. Applying one writes
//! `ink`, `ink_color`, `fg_color`, `bg_color` and `pattern`, so
//! [`ModernOptions`](crate::modern::ModernOptions) stays the single source of
//! truth and the GUI can still override any individual field afterwards.

use crate::modern::color::Rgb;
use crate::modern::options::{InkMode, ModernOptions, Pattern};

/// One named colour theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    /// Stable id, matched case-insensitively against `--theme`.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Ink colour (also the dual-tone foreground).
    pub fg: Rgb,
    /// Paper / dual-tone background colour.
    pub bg: Rgb,
    /// The pattern the theme looks best with.
    pub style: Pattern,
    /// The ink mode the theme selects.
    pub ink: InkMode,
}

impl Theme {
    /// The ink mode the theme implies.
    pub fn ink_mode(&self) -> InkMode {
        self.ink
    }

    /// Apply the theme to an option set, leaving every other field alone.
    pub fn apply(&self, o: &mut ModernOptions) {
        o.ink = self.ink_mode();
        o.ink_color = self.fg;
        o.fg_color = self.fg;
        o.bg_color = self.bg;
        o.paper_color = self.bg;
        o.pattern = self.style;
        o.theme = self.id.to_string();
    }
}

/// The built-in themes.
pub const THEMES: &[Theme] = &[
    Theme { id: "black", name: "Black on white", fg: Rgb(0, 0, 0), bg: Rgb(255, 255, 255), style: Pattern::Dot, ink: InkMode::Fixed },
    Theme { id: "ink-blue", name: "Ink blue", fg: Rgb(18, 52, 86), bg: Rgb(248, 248, 244), style: Pattern::Dot, ink: InkMode::Fixed },
    Theme { id: "sepia", name: "Sepia", fg: Rgb(92, 58, 30), bg: Rgb(250, 244, 228), style: Pattern::Mosaic, ink: InkMode::Fixed },
    Theme { id: "duotone-coral", name: "Coral duotone", fg: Rgb(214, 74, 58), bg: Rgb(255, 244, 236), style: Pattern::Dot, ink: InkMode::DualTone },
    Theme { id: "duotone-teal", name: "Teal duotone", fg: Rgb(22, 107, 116), bg: Rgb(232, 245, 244), style: Pattern::Triangle, ink: InkMode::DualTone },
    Theme { id: "graphite", name: "Graphite", fg: Rgb(38, 38, 38), bg: Rgb(242, 242, 242), style: Pattern::Square, ink: InkMode::Fixed },
    Theme { id: "engraving", name: "Engraving lines", fg: Rgb(0, 0, 0), bg: Rgb(255, 255, 255), style: Pattern::LineMorph, ink: InkMode::Fixed },
    Theme { id: "hex-honey", name: "Honeycomb", fg: Rgb(120, 72, 12), bg: Rgb(255, 248, 224), style: Pattern::Dot, ink: InkMode::Fixed },
];

/// Look up a theme by id (case-insensitive).
pub fn theme(id: &str) -> Option<Theme> {
    let t = id.trim();
    THEMES.iter().copied().find(|x| x.id.eq_ignore_ascii_case(t))
}

/// Every theme id, for the `--theme` help text and the GUI combo box.
pub fn theme_ids() -> Vec<&'static str> {
    THEMES.iter().map(|t| t.id).collect()
}

/// Apply a theme by id, reporting whether it exists.
pub fn apply_theme(id: &str, o: &mut ModernOptions) -> bool {
    match theme(id) {
        Some(t) => {
            t.apply(o);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_is_case_insensitive() {
        assert_eq!(theme("Sepia").map(|t| t.id), Some("sepia"));
        assert_eq!(theme(" SEPIA ").map(|t| t.id), Some("sepia"));
        assert!(theme("nope").is_none());
    }

    #[test]
    fn ids_are_unique_and_well_formed() {
        let ids = theme_ids();
        let mut seen = std::collections::BTreeSet::new();
        for id in &ids {
            assert!(seen.insert(*id), "duplicate theme id {id}");
            assert!(crate::modern::options::is_portable_name(id));
        }
    }

    #[test]
    fn applying_writes_the_option_fields() {
        let mut o = ModernOptions::default();
        let t = theme("duotone-coral").unwrap();
        t.apply(&mut o);
        assert_eq!(o.ink, InkMode::DualTone);
        assert_eq!(o.ink_color, t.fg);
        assert_eq!(o.fg_color, t.fg);
        assert_eq!(o.bg_color, t.bg);
        assert_eq!(o.paper_color, t.bg);
        assert_eq!(o.pattern, Pattern::Dot);
        assert_eq!(o.theme, "duotone-coral");
        // Untouched fields keep their values.
        assert_eq!(o.margin_mm, 10.0);
        assert_eq!(o.nominal_dot_mm, 10.0);
    }

    #[test]
    fn a_same_colour_theme_is_fixed_ink() {
        let mut o = ModernOptions::default();
        assert!(apply_theme("black", &mut o));
        assert_eq!(o.ink, InkMode::Fixed);
        assert!(!apply_theme("nope", &mut o));
        assert_eq!(o.ink, InkMode::Fixed);
    }
}