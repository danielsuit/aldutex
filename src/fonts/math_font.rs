//! Math font constants from the OpenType MATH table.
//!
//! These constants drive the math layout engine's spacing and positioning.
//! They are read from the font's MATH table via `ttf_parser` and scaled
//! to the requested point size.

use crate::error::AldutexError;
use crate::fonts::loader::LoadedFont;

/// Constants read from the OpenType MATH table, scaled to the requested point size.
/// These drive the math layout engine's spacing and positioning decisions.
#[derive(Debug, Clone)]
pub struct MathConstants {
    /// Scale factor for script size (e.g., 70%).
    pub script_percent_scale_down: i32,
    /// Scale factor for script-script size (e.g., 50%).
    pub script_script_percent_scale_down: i32,
    /// Shift up for fraction numerators.
    pub fraction_numerator_shift_up: f64,
    /// Shift down for fraction denominators.
    pub fraction_denominator_shift_down: f64,
    /// Thickness of fraction bars.
    pub fraction_rule_thickness: f64,
    /// Thickness of radical (square root) rules.
    pub radical_rule_thickness: f64,
    /// Gap between radical body and rule.
    pub radical_vertical_gap: f64,
    /// Extra ascender above radical rule.
    pub radical_extra_ascender: f64,
    /// Shift up for superscripts.
    pub superscript_shift_up: f64,
    /// Shift down for subscripts.
    pub subscript_shift_down: f64,
    /// Minimum gap between sub and superscripts.
    pub sub_superscript_gap_min: f64,
    /// Minimum gap for upper limits.
    pub upper_limit_gap_min: f64,
    /// Minimum gap for lower limits.
    pub lower_limit_gap_min: f64,
    /// Leading between math lines.
    pub math_leading: f64,
    /// Height of the math axis (center for fractions).
    pub axis_height: f64,
}

impl MathConstants {
    /// Provide sensible default constants for when a font lacks a MATH table.
    /// Scaled to the given point size.
    pub fn defaults(size_pt: f64) -> Self {
        Self {
            script_percent_scale_down: 70,
            script_script_percent_scale_down: 50,
            fraction_numerator_shift_up: size_pt * 0.676,
            fraction_denominator_shift_down: size_pt * 0.480,
            fraction_rule_thickness: size_pt * 0.040,
            radical_rule_thickness: size_pt * 0.040,
            radical_vertical_gap: size_pt * 0.060,
            radical_extra_ascender: size_pt * 0.040,
            superscript_shift_up: size_pt * 0.413,
            subscript_shift_down: size_pt * 0.150,
            sub_superscript_gap_min: size_pt * 0.150,
            upper_limit_gap_min: size_pt * 0.150,
            lower_limit_gap_min: size_pt * 0.150,
            math_leading: size_pt * 0.150,
            axis_height: size_pt * 0.250,
        }
    }
}

/// Scale a raw MathValue (in design units) to points.
fn scale_value(raw: i16, units_per_em: u16, size_pt: f64) -> f64 {
    (raw as f64 / units_per_em as f64) * size_pt
}

/// Load math constants from the OpenType MATH table of the given font.
///
/// Falls back to sensible defaults if the MATH table is missing or incomplete.
pub fn load_math_constants(font: &LoadedFont, size_pt: f64) -> miette::Result<MathConstants> {
    let font_data = font.data.as_slice();
    let face = ttf_parser::Face::parse(font_data, 0).map_err(|e| AldutexError::FontLoadFailed {
        reason: format!("Failed to parse font for MATH table: {e}"),
    })?;

    let math_table = match face.tables().math {
        Some(mt) => mt,
        None => return Ok(MathConstants::defaults(size_pt)),
    };

    let constants = match math_table.constants {
        Some(c) => c,
        None => return Ok(MathConstants::defaults(size_pt)),
    };
    let upem = font.units_per_em;

    Ok(MathConstants {
        script_percent_scale_down: constants.script_percent_scale_down() as i32,
        script_script_percent_scale_down: constants.script_script_percent_scale_down() as i32,
        fraction_numerator_shift_up: scale_value(
            constants.fraction_numerator_display_style_shift_up().value,
            upem,
            size_pt,
        ),
        fraction_denominator_shift_down: scale_value(
            constants
                .fraction_denominator_display_style_shift_down()
                .value,
            upem,
            size_pt,
        ),
        fraction_rule_thickness: scale_value(
            constants.fraction_rule_thickness().value,
            upem,
            size_pt,
        ),
        radical_rule_thickness: scale_value(
            constants.radical_rule_thickness().value,
            upem,
            size_pt,
        ),
        radical_vertical_gap: scale_value(
            constants.radical_display_style_vertical_gap().value,
            upem,
            size_pt,
        ),
        radical_extra_ascender: scale_value(
            constants.radical_extra_ascender().value,
            upem,
            size_pt,
        ),
        superscript_shift_up: scale_value(constants.superscript_shift_up().value, upem, size_pt),
        subscript_shift_down: scale_value(constants.subscript_shift_down().value, upem, size_pt),
        sub_superscript_gap_min: scale_value(
            constants.sub_superscript_gap_min().value,
            upem,
            size_pt,
        ),
        upper_limit_gap_min: scale_value(constants.upper_limit_gap_min().value, upem, size_pt),
        lower_limit_gap_min: scale_value(constants.lower_limit_gap_min().value, upem, size_pt),
        math_leading: scale_value(constants.math_leading().value, upem, size_pt),
        axis_height: scale_value(constants.axis_height().value, upem, size_pt),
    })
}
