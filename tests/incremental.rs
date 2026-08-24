//! Tests for the incremental highlighting logic: dirty-region detection,
//! windowed parse planning, line slicing, and delta command building.

use giallo_kak::highlight::{
    build_delta_commands, chunk_option_name, line_slice, parse_plan, Plan, CHUNK_LINES,
    MARGIN_LINES,
};
use giallo_kak::highlighting::{FaceDef, RangeToken};

fn tokens(spans: &[(usize, usize, &str)]) -> Vec<RangeToken> {
    spans
        .iter()
        .map(|(s, e, f)| RangeToken {
            start: *s,
            end: *e,
            face: f.to_string(),
        })
        .collect()
}

#[test]
fn plan_no_change_for_identical_text() {
    let text = "fn main() {}\nlet x = 1;\n";
    assert!(matches!(parse_plan(text, text), Plan::NoChange));
}

#[test]
fn plan_full_on_first_highlight() {
    assert!(matches!(parse_plan("", "let x = 1;\n"), Plan::Full));
}

#[test]
fn plan_window_for_small_midfile_edit() {
    // 1000 lines; edit a number on line 500 (0-indexed 499).
    let mut old = String::new();
    let mut new = String::new();
    for i in 0..1000 {
        old.push_str(&format!("let v{i} = {i};\n"));
        if i == 499 {
            new.push_str("let v499 = 49999;\n");
        } else {
            new.push_str(&format!("let v{i} = {i};\n"));
        }
    }
    match parse_plan(&old, &new) {
        Plan::Window(sp) => {
            assert_eq!(sp.dirty_start, 499);
            assert_eq!(sp.dirty_end_new, 499);
            assert_eq!(sp.dirty_end_old, 499);
            assert_eq!(sp.window_start, 499 - 200);
            assert_eq!(sp.window_end, 499 + MARGIN_LINES);
        }
        _ => panic!("expected windowed plan"),
    }
}

#[test]
fn plan_full_when_edit_touches_string_delimiters() {
    let mut old = String::new();
    let mut new = String::new();
    for i in 0..1000 {
        old.push_str(&format!("let v{i} = {i};\n"));
        if i == 500 {
            new.push_str("let v500 = \"str\";\n");
        } else {
            new.push_str(&format!("let v{i} = {i};\n"));
        }
    }
    assert!(matches!(parse_plan(&old, &new), Plan::Full));
}

#[test]
fn plan_full_for_comment_marker_edits() {
    let old = "let a = 1;\nlet b = 2;\n".repeat(300);
    let new = format!("{}/* hmm\n{}", "let a = 1;\n".repeat(299), &old[0..0]);
    // Introducing a block comment opener must force a full parse.
    let old2 = "let a = 1;\n".repeat(600);
    let new2 = {
        let mut s: String = "let a = 1;\n".repeat(599);
        s.push_str("let b = /* x */ 2;\n");
        s
    };
    assert!(matches!(parse_plan(&old2, &new2), Plan::Full));
    let _ = (old, new);
}

#[test]
fn plan_full_for_oversized_dirty_region() {
    // Rewrite 300 of 1000 lines: more than a quarter of the document.
    let mut old = String::new();
    let mut new = String::new();
    for i in 0..1000 {
        old.push_str(&format!("let v{i} = {i};\n"));
        if (400..700).contains(&i) {
            new.push_str(&format!("changed_token_{i};\n"));
        } else {
            new.push_str(&format!("let v{i} = {i};\n"));
        }
    }
    assert!(matches!(parse_plan(&old, &new), Plan::Full));
}

#[test]
fn plan_handles_line_insertions_and_removals() {
    let mut old = String::new();
    for i in 0..1000 {
        old.push_str(&format!("let v{i} = {i};\n"));
    }
    // Insert two lines at index 700.
    let mut new = String::new();
    for i in 0..1000 {
        if i == 700 {
            new.push_str("let ins_a = 1;\nlet ins_b = 2;\n");
        }
        new.push_str(&format!("let v{i} = {i};\n"));
    }
    match parse_plan(&old, &new) {
        Plan::Window(sp) => {
            assert_eq!(sp.dirty_start, 700);
            // Both inserted lines must be covered (boundary may extend into
            // the neighboring line depending on where byte comparison stops).
            assert!(sp.dirty_end_new >= 701);
            // Boundary may extend into the neighboring line on the old side.
            assert!((699..=700).contains(&sp.dirty_end_old));
        }
        _ => panic!("expected windowed plan for insertion"),
    }

    // Removal: delete the same two lines again.
    match parse_plan(&new, &old) {
        Plan::Window(sp) => {
            assert_eq!(sp.dirty_start, 700);
            assert!(sp.dirty_end_old >= 700);
        }
        _ => panic!("expected windowed plan for removal"),
    }
}

#[test]
fn line_slice_extracts_inclusive_range() {
    let text = "a\nb\nc\nd\ne";
    assert_eq!(line_slice(text, 1, 3), "b\nc\nd");
    assert_eq!(line_slice(text, 0, 0), "a");
    assert_eq!(line_slice(text, 4, 10), "e"); // clamped
}

#[test]
fn delta_commands_send_all_chunks_on_first_update() {
    let lines: Vec<Vec<RangeToken>> = (0..CHUNK_LINES + 5)
        .map(|i| tokens(&[(0, 3, "giallo_0001")]))
        .collect();
    let faces = vec![FaceDef {
        name: "giallo_0001".into(),
        spec: "rgb:ffffff,default".into(),
    }];
    let cmd = build_delta_commands(&[], &lines, &faces, &mut Default::default())
        .expect("first update must send");

    assert!(cmd.contains("set-face global giallo_0001"));
    assert!(cmd.contains("set-option buffer giallo_hl_ranges %val{timestamp}"));
    assert!(cmd.contains(&format!(
        "set-option buffer {} %val{{timestamp}}",
        chunk_option_name(1)
    )));
}

#[test]
fn delta_commands_skip_unchanged_chunks() {
    // Two chunks: 1200 lines where only a line in the second chunk changes.
    let mk = |face: &str| tokens(&[(0, 3, face)]);
    let old_lines: Vec<Vec<RangeToken>> = (0..1200).map(|_| mk("giallo_0001")).collect();
    let mut new_lines = old_lines.clone();
    new_lines[1150] = mk("giallo_0002");

    let cmd = build_delta_commands(&old_lines, &new_lines, &[], &mut Default::default())
        .expect("changed chunk must send");

    // Only the second chunk should be updated (note: exact match incl. the
    // value separator, so "giallo_hl_ranges_1" doesn't satisfy chunk 0).
    let opt0 = format!("set-option buffer {} %val", chunk_option_name(0));
    let opt1 = format!("set-option buffer {} %val", chunk_option_name(1));
    assert!(!cmd.contains(&opt0));
    assert!(cmd.contains(&opt1));
}

#[test]
fn delta_commands_none_when_identical() {
    let lines = vec![tokens(&[(0, 3, "giallo_0001")]); 20];
    assert!(build_delta_commands(&lines, &lines, &[], &mut Default::default()).is_none());
}

#[test]
fn delta_commands_clear_shrunken_chunks() {
    // Two chunks worth of lines shrink to one.
    let big: Vec<Vec<RangeToken>> = vec![tokens(&[(0, 1, "giallo_0001")]); CHUNK_LINES * 2];
    let small: Vec<Vec<RangeToken>> = vec![tokens(&[(0, 1, "giallo_0001")]); 10];
    let cmd = build_delta_commands(&big, &small, &[], &mut Default::default())
        .expect("shrink must send");
    assert!(cmd.contains(&format!(
        "set-option buffer {} %val{{timestamp}}\n",
        chunk_option_name(1)
    )));
}
