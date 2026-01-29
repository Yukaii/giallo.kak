use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn make_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let unique = format!(
        "{}-{}-{}",
        prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    dir.push(unique);
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

fn write_config(config_dir: &Path, theme: &str) {
    let cfg_dir = config_dir.join("giallo.kak");
    fs::create_dir_all(&cfg_dir).expect("failed to create config dir");
    let config_path = cfg_dir.join("config.toml");
    let contents = format!("theme = \"{}\"\n", theme);
    fs::write(&config_path, contents).expect("failed to write config");
}

fn run_oneshot_highlight(lang: &str, theme: &str, code: &str) -> String {
    let config_home = make_temp_dir("giallo-kak-test-config");
    write_config(&config_home, theme);

    let payload = code.as_bytes();
    let header = format!("H {} {} {}\n", lang, theme, payload.len());

    let bin = env!("CARGO_BIN_EXE_giallo-kak");
    let mut child = Command::new(bin)
        .arg("--oneshot")
        .env("XDG_CONFIG_HOME", &config_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn giallo-kak");

    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        stdin
            .write_all(header.as_bytes())
            .expect("failed to write header");
        stdin.write_all(payload).expect("failed to write payload");
    }

    let output = child
        .wait_with_output()
        .expect("failed to read giallo-kak output");

    assert!(output.status.success(), "giallo-kak failed");

    String::from_utf8_lossy(&output.stdout).to_string()
}

fn assert_valid_highlighting(output: &str, fixture_name: &str) {
    // The output should contain face definitions
    assert!(
        output.contains("set-face global"),
        "{}: output should contain face definitions",
        fixture_name
    );

    // The output should contain highlight ranges
    assert!(
        output.contains("set-option buffer giallo_hl_ranges"),
        "{}: output should contain highlight ranges",
        fixture_name
    );

    // Extract the ranges line and verify it has content
    let ranges_line = output
        .lines()
        .find(|line| line.starts_with("set-option buffer giallo_hl_ranges"))
        .unwrap_or_else(|| panic!("{}: should have ranges line", fixture_name));

    // Should have actual ranges (not just timestamp)
    let parts: Vec<&str> = ranges_line.split_whitespace().collect();
    assert!(
        parts.len() > 4,
        "{}: should have highlight ranges beyond timestamp, got: {}",
        fixture_name,
        ranges_line
    );

    // Verify face definitions are valid (rgb format)
    for line in output.lines() {
        if line.starts_with("set-face global") {
            assert!(
                line.contains("rgb:") || line.contains("default"),
                "{}: face definition should use rgb format: {}",
                fixture_name,
                line
            );
        }
    }
}

fn count_highlights(output: &str) -> usize {
    // Count the number of highlight ranges
    let ranges_line = output
        .lines()
        .find(|line| line.starts_with("set-option buffer giallo_hl_ranges"))
        .expect("should have ranges line");

    // Skip "set-option", "buffer", "giallo_hl_ranges", and timestamp
    let parts: Vec<&str> = ranges_line.split_whitespace().collect();
    if parts.len() <= 4 {
        return 0;
    }

    // Count ranges (each range has format "line.col,line.col|face")
    parts[4..].len()
}

#[test]
fn fixture_rust_sample() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("rust_sample.rs");

    let code = fs::read_to_string(&fixture_path).expect("failed to read fixture");
    let output = run_oneshot_highlight("rust", "catppuccin-frappe", &code);

    assert_valid_highlighting(&output, "rust_sample.rs");

    // Should have many highlights for a full Rust file
    let count = count_highlights(&output);
    assert!(
        count > 50,
        "rust_sample.rs should have substantial highlighting, got {} ranges",
        count
    );
}

#[test]
fn fixture_javascript_sample() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("javascript_sample.js");

    let code = fs::read_to_string(&fixture_path).expect("failed to read fixture");
    let output = run_oneshot_highlight("javascript", "catppuccin-frappe", &code);

    assert_valid_highlighting(&output, "javascript_sample.js");

    let count = count_highlights(&output);
    assert!(
        count > 50,
        "javascript_sample.js should have substantial highlighting, got {} ranges",
        count
    );
}

#[test]
fn fixture_python_sample() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("python_sample.py");

    let code = fs::read_to_string(&fixture_path).expect("failed to read fixture");
    let output = run_oneshot_highlight("python", "catppuccin-frappe", &code);

    assert_valid_highlighting(&output, "python_sample.py");

    let count = count_highlights(&output);
    assert!(
        count > 50,
        "python_sample.py should have substantial highlighting, got {} ranges",
        count
    );
}

#[test]
fn fixture_go_sample() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("go_sample.go");

    let code = fs::read_to_string(&fixture_path).expect("failed to read fixture");
    let output = run_oneshot_highlight("go", "catppuccin-frappe", &code);

    assert_valid_highlighting(&output, "go_sample.go");

    let count = count_highlights(&output);
    assert!(
        count > 50,
        "go_sample.go should have substantial highlighting, got {} ranges",
        count
    );
}

#[test]
fn fixture_rust_with_different_themes() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("rust_sample.rs");

    let code = fs::read_to_string(&fixture_path).expect("failed to read fixture");

    let themes = vec![
        "catppuccin-frappe",
        "catppuccin-mocha",
        "tokyo-night",
        "dracula",
        "kanagawa-wave",
    ];

    for theme in themes {
        let output = run_oneshot_highlight("rust", theme, &code);
        assert_valid_highlighting(&output, &format!("rust_sample.rs with {}", theme));

        let count = count_highlights(&output);
        assert!(
            count > 50,
            "rust_sample.rs with {} should have substantial highlighting",
            theme
        );
    }
}

#[test]
fn fixture_javascript_with_different_themes() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("javascript_sample.js");

    let code = fs::read_to_string(&fixture_path).expect("failed to read fixture");

    let themes = vec!["catppuccin-frappe", "dracula", "tokyo-night"];

    for theme in themes {
        let output = run_oneshot_highlight("javascript", theme, &code);
        assert_valid_highlighting(
            &output,
            &format!("javascript_sample.js with {}", theme),
        );
    }
}

#[test]
fn fixture_string_highlighting_rust() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("rust_sample.rs");

    let code = fs::read_to_string(&fixture_path).expect("failed to read fixture");
    let output = run_oneshot_highlight("rust", "catppuccin-frappe", &code);

    // Verify that string literals appear in the output
    // The fixture has multiple string types: regular, multiline, and raw strings
    assert_valid_highlighting(&output, "rust_sample.rs (strings)");

    // Parse the output to check if we have string-related highlights
    let ranges_line = output
        .lines()
        .find(|line| line.starts_with("set-option buffer giallo_hl_ranges"))
        .expect("should have ranges line");

    // The ranges should include highlights on lines with strings
    // Line 5: let greeting = "Hello, world!";
    // Line 6-7: multiline string
    // Line 8: raw string
    assert!(
        ranges_line.contains("5.") || ranges_line.contains("6.") || ranges_line.contains("8."),
        "should have highlights on string literal lines"
    );
}

#[test]
fn fixture_keyword_highlighting_rust() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("rust_sample.rs");

    let code = fs::read_to_string(&fixture_path).expect("failed to read fixture");
    let output = run_oneshot_highlight("rust", "catppuccin-frappe", &code);

    assert_valid_highlighting(&output, "rust_sample.rs (keywords)");

    // The fixture has many keywords: fn, let, if, else, return, for, in, match, break, continue
    // Verify we have highlights throughout the file
    let ranges_line = output
        .lines()
        .find(|line| line.starts_with("set-option buffer giallo_hl_ranges"))
        .expect("should have ranges line");

    // Should have highlights on lines with keywords
    // Line 4: fn main()
    // Line 11: if true
    // Line 20: for i in 0..10
    assert!(
        (ranges_line.contains("4.") || ranges_line.contains("11.") || ranges_line.contains("20."))
            && count_highlights(&output) > 50,
        "should have keyword highlights throughout the file"
    );
}

#[test]
fn fixture_empty_file() {
    let code = "";
    let output = run_oneshot_highlight("rust", "catppuccin-frappe", code);

    // Even empty files should produce valid output
    assert!(
        output.contains("set-option buffer giallo_hl_ranges"),
        "empty file should still produce ranges line"
    );
}

#[test]
fn fixture_whitespace_only() {
    let code = "   \n\n   \n\t\t\n";
    let output = run_oneshot_highlight("rust", "catppuccin-frappe", code);

    // Whitespace-only files should produce valid output
    assert!(
        output.contains("set-option buffer giallo_hl_ranges"),
        "whitespace-only file should still produce ranges line"
    );
}

#[test]
fn fixture_comments_only() {
    let code = "// Just a comment\n// Another comment\n/* Block comment */";
    let output = run_oneshot_highlight("rust", "catppuccin-frappe", code);

    assert_valid_highlighting(&output, "comments-only");

    // Comments should be highlighted
    let count = count_highlights(&output);
    assert!(count > 0, "comments should be highlighted");
}

#[test]
fn fixture_nested_strings() {
    let code = r#"const s = "outer 'inner' outer";
const t = 'outer "inner" outer';
const u = `outer "double" 'single' outer`;"#;

    let output = run_oneshot_highlight("javascript", "catppuccin-frappe", code);
    assert_valid_highlighting(&output, "nested-strings");

    let count = count_highlights(&output);
    assert!(count > 10, "nested strings should be highlighted");
}
