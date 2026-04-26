//! Font loading and registry.
//!
//! The [`FontRegistry`] loads all 6 bundled Latin Modern fonts at startup
//! and provides access by [`FontId`]. Each font is parsed into both a
//! `rustybuzz::Face` (for shaping) and a `skrifa::FontRef` (for metrics).

use crate::error::AldutexError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A unique identifier for a loaded font.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FontId(pub u8);

impl FontId {
    /// Font ID for Latin Modern Roman Regular.
    pub fn regular() -> FontId {
        FontId(0)
    }
    /// Font ID for Latin Modern Roman Bold.
    pub fn bold() -> FontId {
        FontId(1)
    }
    /// Font ID for Latin Modern Roman Italic.
    pub fn italic() -> FontId {
        FontId(2)
    }
    /// Font ID for Latin Modern Roman Bold Italic.
    pub fn bolditalic() -> FontId {
        FontId(3)
    }
    /// Font ID for Latin Modern Mono.
    pub fn mono() -> FontId {
        FontId(4)
    }
    /// Font ID for Latin Modern Math.
    pub fn math() -> FontId {
        FontId(5)
    }
}

/// A loaded font with both shaping and metrics faces.
///
/// The `data` field holds an `Arc<Vec<u8>>` that is shared between
/// the rustybuzz and skrifa faces. Both reference the same bytes.
pub struct LoadedFont {
    /// Unique ID.
    pub id: FontId,
    /// Owned copy of the font bytes (Arc-shared).
    pub data: Arc<Vec<u8>>,
    /// rustybuzz face for text shaping.
    pub face: rustybuzz::Face<'static>,
    /// Units per em from the font's head table.
    pub units_per_em: u16,
}

impl std::fmt::Debug for LoadedFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedFont")
            .field("id", &self.id)
            .field("units_per_em", &self.units_per_em)
            .finish()
    }
}

/// Registry of all available fonts.
pub struct FontRegistry {
    fonts: Vec<LoadedFont>,
}

impl FontRegistry {
    /// Load all bundled fonts and create the registry.
    ///
    /// Returns `Err` if any font file fails to parse.
    pub fn new() -> miette::Result<Self> {
        let font_defs: Vec<(FontId, &'static [u8], &str)> = vec![
            (FontId::regular(), super::LM_REGULAR, "LM Roman Regular"),
            (FontId::bold(), super::LM_BOLD, "LM Roman Bold"),
            (FontId::italic(), super::LM_ITALIC, "LM Roman Italic"),
            (
                FontId::bolditalic(),
                super::LM_BOLDITALIC,
                "LM Roman Bold Italic",
            ),
            (FontId::mono(), super::LM_MONO, "LM Mono Regular"),
            (FontId::math(), super::LM_MATH, "LM Math"),
        ];

        let mut fonts = Vec::with_capacity(font_defs.len());

        for (id, data_bytes, name) in font_defs {
            let data = Arc::new(data_bytes.to_vec());

            // Create rustybuzz face.
            // We need the face to have a 'static lifetime, so we leak
            // a reference from the Arc. This is safe because the Arc is
            // held by the LoadedFont and lives as long as the registry.
            let data_ref: &'static [u8] =
                unsafe { std::slice::from_raw_parts(data.as_ptr(), data.len()) };

            let face = rustybuzz::Face::from_slice(data_ref, 0).ok_or_else(|| {
                AldutexError::FontLoadFailed {
                    reason: format!("Failed to parse rustybuzz face for {name}"),
                }
            })?;

            let units_per_em = face.units_per_em() as u16;

            fonts.push(LoadedFont {
                id,
                data,
                face,
                units_per_em,
            });
        }

        Ok(Self { fonts })
    }

    /// Get a font by ID.
    pub fn get(&self, id: FontId) -> &LoadedFont {
        &self.fonts[id.0 as usize]
    }

    /// Font ID for Latin Modern Roman Regular.
    pub fn regular() -> FontId {
        FontId::regular()
    }
    /// Font ID for Latin Modern Roman Bold.
    pub fn bold() -> FontId {
        FontId::bold()
    }
    /// Font ID for Latin Modern Roman Italic.
    pub fn italic() -> FontId {
        FontId::italic()
    }
    /// Font ID for Latin Modern Roman Bold Italic.
    pub fn bolditalic() -> FontId {
        FontId::bolditalic()
    }
    /// Font ID for Latin Modern Mono.
    pub fn mono() -> FontId {
        FontId::mono()
    }
    /// Font ID for Latin Modern Math.
    pub fn math() -> FontId {
        FontId::math()
    }

    /// Total number of loaded fonts.
    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    /// Returns true if no fonts are loaded (should never happen after `new()`).
    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }
}
