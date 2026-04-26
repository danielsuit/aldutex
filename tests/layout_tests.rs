use aldutex::layout::paragraph::{break_paragraph, Item};

fn dummy_box(width: f64) -> Item {
    Item::Box {
        width,
        content: vec![],
    }
}

fn dummy_glue(width: f64, stretch: f64, shrink: f64) -> Item {
    Item::Glue {
        width,
        stretch,
        shrink,
    }
}

fn dummy_penalty(width: f64, penalty: f64) -> Item {
    Item::Penalty {
        width,
        penalty,
        flagged: false,
    }
}

/// Helper to assert line breaks are correct
fn assert_breaks(items: &[Item], line_width: f64, expected_breaks: &[usize]) {
    let breaks = break_paragraph(items, line_width, 10.0, 50.0);
    assert_eq!(breaks, expected_breaks);
}

#[test]
fn test_simple_paragraph_break() {
    // 3 words of length 30 separated by glues of 10.
    // Total width = 30 + 10 + 30 + 10 + 30 = 110.
    // If line width is 80, it should break after the second word.
    let items = vec![
        dummy_box(30.0),            // 0
        dummy_glue(10.0, 5.0, 2.0), // 1
        dummy_box(30.0),            // 2
        dummy_glue(10.0, 5.0, 2.0), // 3
        dummy_box(30.0),            // 4
        // Forced end
        dummy_penalty(0.0, -10000.0), // 5
    ];

    // For width 80:
    // Line 1: 0 (30) + 1 (10) + 2 (30) = 70. Stretch to 80 is easy.
    // Line 2: 4 (30)
    assert_breaks(&items, 80.0, &[3, 5]);
}

#[test]
fn test_too_long_word() {
    let items = vec![dummy_box(100.0), dummy_penalty(0.0, -10000.0)];
    // If line width is 50, word is 100. The Knuth-Plass algorithm should be forced to keep it and not drop it.
    assert_breaks(&items, 50.0, &[1]);
}

#[test]
fn test_perfect_fit() {
    let items = vec![
        dummy_box(50.0),              // 0
        dummy_glue(10.0, 5.0, 2.0),   // 1
        dummy_box(40.0),              // 2
        dummy_penalty(0.0, -10000.0), // 3
    ];
    // Exact sum = 100. Should break perfectly at end.
    assert_breaks(&items, 100.0, &[3]);
}
