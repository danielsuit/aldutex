//! Font subsystem: loading, metrics, shaping, and math constants.

pub mod loader;
pub mod math_font;
pub mod metrics;
pub mod shaper;

// Bundled Latin Modern fonts — embedded at compile time.
// All are licensed under the GUST Font License (GFL), compatible with OFL.

/// Latin Modern Roman 10 — Regular (main body text).
pub static LM_REGULAR: &[u8] = include_bytes!("data/lmroman10-regular.otf");
/// Latin Modern Roman 10 — Bold.
pub static LM_BOLD: &[u8] = include_bytes!("data/lmroman10-bold.otf");
/// Latin Modern Roman 10 — Italic.
pub static LM_ITALIC: &[u8] = include_bytes!("data/lmroman10-italic.otf");
/// Latin Modern Roman 10 — Bold Italic.
pub static LM_BOLDITALIC: &[u8] = include_bytes!("data/lmroman10-bolditalic.otf");
/// Latin Modern Mono 10 — Regular (monospace / `\texttt`).
pub static LM_MONO: &[u8] = include_bytes!("data/lmmono10-regular.otf");
/// Latin Modern Math — OpenType math font for `$...$` and `\[...\]`.
pub static LM_MATH: &[u8] = include_bytes!("data/latinmodern-math.otf");
