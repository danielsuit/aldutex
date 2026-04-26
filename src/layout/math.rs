//! Math expression layout.

/// Layout style for math expressions.
#[derive(Debug, Clone, Copy)]
pub enum LayoutStyle {
    Display,
    Text,
    Script,
    ScriptScript,
}
