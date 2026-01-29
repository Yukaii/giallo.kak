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

fn assert_contains_highlighting(output: &str) {
    // The output should contain face definitions and highlight ranges
    assert!(
        output.contains("set-face global"),
        "output should contain face definitions"
    );
    assert!(
        output.contains("set-option buffer giallo_hl_ranges"),
        "output should contain highlight ranges"
    );
}

fn assert_has_ranges(output: &str) {
    // Extract the ranges line
    let ranges_line = output
        .lines()
        .find(|line| line.starts_with("set-option buffer giallo_hl_ranges"))
        .expect("should have ranges line");

    // Should have actual ranges (not just timestamp)
    let parts: Vec<&str> = ranges_line.split_whitespace().collect();
    assert!(
        parts.len() > 4,
        "should have highlight ranges, got: {}",
        ranges_line
    );
}

#[test]
fn rust_string_highlighting() {
    let code = r#"fn main() {
    let s = "hello world";
    println!("{}", s);
}"#;

    let output = run_oneshot_highlight("rust", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}

#[test]
fn rust_keyword_highlighting() {
    let code = r#"fn test() {
    if true {
        return;
    } else {
        break;
    }
    for i in 0..10 {
        continue;
    }
}"#;

    let output = run_oneshot_highlight("rust", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}

#[test]
fn javascript_string_highlighting() {
    let code = r#"const greeting = "Hello, world!";
const template = `Template ${greeting}`;
console.log(greeting);"#;

    let output = run_oneshot_highlight("javascript", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}

#[test]
fn javascript_keyword_highlighting() {
    let code = r#"if (true) {
    const x = 42;
} else {
    let y = 0;
}

for (let i = 0; i < 10; i++) {
    if (i === 5) break;
    if (i === 3) continue;
}

async function test() {
    return await fetch("url");
}"#;

    let output = run_oneshot_highlight("javascript", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}

#[test]
fn python_string_highlighting() {
    let code = r#"greeting = "Hello, world!"
multiline = """This is
multiline"""
f_string = f"Hello {name}"
print(greeting)"#;

    let output = run_oneshot_highlight("python", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}

#[test]
fn python_keyword_highlighting() {
    let code = r#"if True:
    pass
elif False:
    break
else:
    continue

for i in range(10):
    if i == 5:
        return

def test():
    async def inner():
        await something()

class MyClass:
    def __init__(self):
        pass"#;

    let output = run_oneshot_highlight("python", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}

#[test]
fn go_string_highlighting() {
    let code = r#"package main

func main() {
    greeting := "Hello, world!"
    multiline := `This is
multiline`
    fmt.Println(greeting)
}"#;

    let output = run_oneshot_highlight("go", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}

#[test]
fn go_keyword_highlighting() {
    let code = r#"package main

func test() {
    if true {
        return
    } else {
        break
    }

    for i := 0; i < 10; i++ {
        continue
    }

    switch x {
    case 0:
        defer cleanup()
    default:
        go asyncWork()
    }
}"#;

    let output = run_oneshot_highlight("go", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}

#[test]
fn typescript_string_highlighting() {
    let code = r#"const greeting: string = "Hello, world!";
const template = `Template ${greeting}`;
type Name = string;
console.log(greeting);"#;

    let output = run_oneshot_highlight("typescript", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}

#[test]
fn typescript_keyword_highlighting() {
    let code = r#"interface Person {
    name: string;
    age: number;
}

class User implements Person {
    constructor(public name: string, public age: number) {}
}

async function fetch(): Promise<string> {
    return await Promise.resolve("data");
}

enum Color {
    Red,
    Green,
    Blue
}

type ID = string | number;
const value: ID = 42;"#;

    let output = run_oneshot_highlight("typescript", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}

#[test]
fn multiline_string_highlighting() {
    let code = r#"const longString = "This is a very long string that \
spans multiple lines in the source code";

const template = `
    This is a multiline
    template literal
    with ${variable} interpolation
`;

console.log(longString);"#;

    let output = run_oneshot_highlight("javascript", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}

#[test]
fn mixed_string_types() {
    let code = r#"// Single quotes
const single = 'single quoted';
// Double quotes
const double = "double quoted";
// Template literals
const template = `template literal`;
// All together
console.log(single, double, template);"#;

    let output = run_oneshot_highlight("javascript", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}

#[test]
fn keywords_in_context() {
    let code = r#"// Keywords in various contexts
if (condition) {
    const result = true;
    return result;
}

while (running) {
    if (shouldBreak) break;
    if (shouldContinue) continue;
}

try {
    throw new Error("test");
} catch (error) {
    console.error(error);
} finally {
    cleanup();
}"#;

    let output = run_oneshot_highlight("javascript", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}

#[test]
fn empty_string_highlighting() {
    let code = r#"const empty = "";
const whitespace = "   ";
const newline = "\n";"#;

    let output = run_oneshot_highlight("javascript", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}

#[test]
fn string_with_escapes() {
    let code = r#"const escaped = "Hello\nWorld\t!";
const unicode = "\u0048\u0065\u006C\u006C\u006F";
const hex = "\x48\x65\x6C\x6C\x6F";"#;

    let output = run_oneshot_highlight("javascript", "catppuccin-frappe", code);
    assert_contains_highlighting(&output);
    assert_has_ranges(&output);
}
