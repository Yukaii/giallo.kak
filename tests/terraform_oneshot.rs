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

fn write_config(config_dir: &Path, grammars_dir: &Path) {
    let cfg_dir = config_dir.join("giallo.kak");
    fs::create_dir_all(&cfg_dir).expect("failed to create config dir");
    let config_path = cfg_dir.join("config.toml");
    let contents = format!(
        "theme = \"catppuccin-frappe\"\n\n[language_map]\nterraform = \"terraform\"\n\n# Custom grammars\ngrammars_path = \"{}\"\n",
        grammars_dir.display()
    );
    fs::write(&config_path, contents).expect("failed to write config");
}

fn write_terraform_grammar(grammars_dir: &Path) {
    fs::create_dir_all(grammars_dir).expect("failed to create grammars dir");
    let grammar_path = grammars_dir.join("terraform.json");
    let contents = r#"{
  "scopeName": "source.terraform",
  "name": "terraform",
  "fileTypes": ["tf"],
  "patterns": [
    { "match": "\\b(terraform|provider|resource)\\b", "name": "keyword.control.terraform" }
  ]
}"#;
    fs::write(&grammar_path, contents).expect("failed to write grammar");
}

#[test]
fn oneshot_terraform_grammar_highlights() {
    let config_home = make_temp_dir("giallo-kak-test-config");
    let grammars_dir = config_home.join("grammars");
    write_terraform_grammar(&grammars_dir);
    write_config(&config_home, &grammars_dir);

    let sample = r#"terraform {
  required_version = ">= 1.3"
}

resource "aws_s3_bucket" "example" {
  bucket = "giallo-sample-bucket"
}
"#;
    let payload = sample.as_bytes();
    let header = format!("H terraform catppuccin-frappe {}\n", payload.len());

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

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("giallo_hl_ranges %val{timestamp} "));
}
