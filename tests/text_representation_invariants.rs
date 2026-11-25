// SPEC-HAL-107-V
// Any failing test indicates a violation of SPEC-HAL-107.
// Demo behavior is non-authoritative.
//
// NOTE:
// These tests intentionally use ASCII-only boundary generation.
// They do NOT validate grapheme clustering or Unicode boundary correctness.
// Those are covered by separate invariant groups.

use halcyon::core::editing::{ClusterBoundary, Selection, TextCursor, apply_insert_op};

// ============================================================================
// Helper: Create minimal boundaries for ASCII text (one boundary per byte)
// ============================================================================

fn make_ascii_boundaries(text: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    for (i, _) in text.char_indices() {
        boundaries.push(ClusterBoundary {
            byte_start: i,
            byte_end: i + 1,
            line_index: 0,
            x_logical: i as f32 * 10.0,
        });
    }
    // Sentinel at end
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index: 0,
        x_logical: text.len() as f32 * 10.0,
    });
    boundaries
}

// ============================================================================
// SPEC-HAL-107 §0.1 — Text is linear UTF-8 sequence
// ============================================================================

#[test]
fn string_representation_is_utf8_by_construction() {
    // SPEC-HAL-107 §0.1: Document text is a valid UTF-8 scalar sequence.
    // Rust's String type guarantees UTF-8 validity.
    //
    // This test is a representation guard, not a behavioral test.

    let text = String::from("Hello, World!");

    // Assert: text is valid UTF-8 (would panic if not)
    assert!(std::str::from_utf8(text.as_bytes()).is_ok());

    // Assert: each byte position within a char is only at valid UTF-8 boundaries
    for (i, _) in text.char_indices() {
        assert!(
            text.is_char_boundary(i),
            "Position {} must be char boundary",
            i
        );
    }
    assert!(
        text.is_char_boundary(text.len()),
        "End position must be char boundary"
    );
}

// ============================================================================
// SPEC-HAL-107 §0.2 — Edit produces single well-defined byte sequence
// ============================================================================

#[test]
fn edit_produces_single_well_defined_byte_sequence() {
    // SPEC-HAL-107 §0.2: Each edit produces exactly one well-defined byte sequence.

    let mut text = String::from("AB");
    let boundaries = make_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(1); // Between A and B
    let mut selection = Selection::at(1);

    // Perform edit: insert "X" at position 1
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "X");

    // Assert: exact byte sequence is "AXB"
    assert_eq!(
        text.as_bytes(),
        b"AXB",
        "Edit must produce exact byte sequence 'AXB'"
    );
    assert_eq!(text, "AXB");
}

// ============================================================================
// SPEC-HAL-107 §0.3 — Text order is never implicitly altered
// ============================================================================

#[test]
fn text_order_is_never_implicitly_altered() {
    // SPEC-HAL-107 §0.3: Text order is never implicitly altered by edits.
    // Insert must produce: prefix + inserted + suffix (byte-for-byte)

    let mut text = String::from("ABCDEF");
    let boundaries = make_ascii_boundaries(&text);

    // Insert "XYZ" at position 3 (between C and D)
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    let prefix_before = &text[..3]; // "ABC"
    let suffix_before = &text[3..]; // "DEF"
    assert_eq!(prefix_before, "ABC");
    assert_eq!(suffix_before, "DEF");

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "XYZ");

    // Assert: result is exactly prefix + "XYZ" + suffix
    assert_eq!(&text[..3], "ABC", "Prefix must be preserved byte-for-byte");
    assert_eq!(
        &text[3..6],
        "XYZ",
        "Inserted text must appear at exact position"
    );
    assert_eq!(&text[6..], "DEF", "Suffix must be preserved byte-for-byte");
    assert_eq!(text, "ABCXYZDEF");
}

// ============================================================================
// SPEC-HAL-107 §0.4 — Newline is ordinary character
// ============================================================================

#[test]
fn newline_is_ordinary_character() {
    // SPEC-HAL-107 §0.4: '\n' is an ordinary character with no special semantics
    // during insertion. It exists as byte 0x0A at the insertion point.

    let mut text = String::from("AB");
    let boundaries = make_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(1); // Between A and B
    let mut selection = Selection::at(1);

    // Insert newline
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "\n");

    // Assert: newline exists as single byte 0x0A at position 1
    assert_eq!(text.as_bytes()[1], 0x0A, "Newline must be byte 0x0A");
    assert_eq!(text.as_bytes(), b"A\nB");

    // Assert: text is "A\nB"
    assert_eq!(text, "A\nB");
    assert_eq!(text.len(), 3, "Length must be 3 (A + newline + B)");
}

// ============================================================================
// SPEC-HAL-107 §0.5 — Document length is stable and exact
// ============================================================================

#[test]
fn document_length_is_stable_and_exact() {
    // SPEC-HAL-107 §0.5: Document length changes by exactly the edit delta.
    // len_after == len_before + inserted_len - deleted_len

    let mut text = String::from("Hello");
    let boundaries = make_ascii_boundaries(&text);

    let len_before = text.len();
    assert_eq!(len_before, 5);

    let mut cursor = TextCursor::at(5); // At end
    let mut selection = Selection::at(5);

    let insert_str = "!!!";
    let insert_len = insert_str.len();

    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        insert_str,
    );

    let len_after = text.len();

    // Assert: length delta equals inserted bytes (no deletion in this case)
    assert_eq!(
        len_after,
        len_before + insert_len,
        "len_after ({}) must equal len_before ({}) + insert_len ({})",
        len_after,
        len_before,
        insert_len
    );
    assert_eq!(len_after, 8);
    assert_eq!(text, "Hello!!!");
}
