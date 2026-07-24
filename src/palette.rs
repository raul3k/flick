//! Indicator color palettes.
//!
//! The diagonal cut on the muted icon already carries the state without color,
//! so the palette is a preference, not an accessibility requirement. The
//! colorblind-safe option is offered for comfort, and labeled by the problem
//! it solves so the people who need it can find it.

/// One palette: the color when the mic is live and when it is muted.
pub struct Palette {
    pub code: &'static str,
    /// i18n key for the color name (e.g. "Blue / Orange").
    pub name_key: &'static str,
    /// The color vision condition this palette targets, spelled out. Empty for
    /// the default. Not translated: these are the same medical terms in every
    /// language we ship.
    pub condition: &'static str,
    on: (u8, u8, u8),
    off: (u8, u8, u8),
}

impl Palette {
    fn rgb_f(c: (u8, u8, u8)) -> (f64, f64, f64) {
        (c.0 as f64 / 255.0, c.1 as f64 / 255.0, c.2 as f64 / 255.0)
    }

    fn hex(c: (u8, u8, u8)) -> String {
        format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
    }

    /// Live/muted color as cairo floats (0.0..=1.0).
    pub fn color_f(&self, active: bool) -> (f64, f64, f64) {
        Self::rgb_f(if active { self.on } else { self.off })
    }

    pub fn on_hex(&self) -> String {
        Self::hex(self.on)
    }

    pub fn off_hex(&self) -> String {
        Self::hex(self.off)
    }
}

/// Fallback when the config names an unknown palette.
pub const DEFAULT: &str = "classic";

pub const PALETTES: &[Palette] = &[
    Palette {
        code: "classic",
        name_key: "palette.classic",
        condition: "",
        on: (46, 204, 113), // green
        off: (231, 76, 60), // red
    },
    Palette {
        code: "redgreen",
        name_key: "palette.redgreen",
        // Red-green deficiencies are the common ~8%. Blue/orange keeps the two
        // states ~130% apart under both protanopia and deuteranopia, vs 42% for
        // green/red under protanopia (measured).
        condition: "Protanopia / Deuteranopia",
        on: (0, 114, 178),  // blue
        off: (230, 159, 0), // orange
    },
    Palette {
        code: "tritan",
        name_key: "palette.tritan",
        // Blue/orange collapses under tritanopia (down to 91%); blue/yellow
        // stays apart there (104%) thanks to its strong luminance gap.
        condition: "Tritanopia",
        on: (0, 114, 178),   // blue
        off: (240, 228, 66), // yellow
    },
];

/// The palette for `code`, falling back to the default for unknown values.
pub fn get(code: &str) -> &'static Palette {
    PALETTES
        .iter()
        .find(|p| p.code == code)
        .unwrap_or(&PALETTES[0])
}
