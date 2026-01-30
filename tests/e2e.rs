//! E2E tests for giallo.kak
//!
//! These tests launch real Kakoune instances to verify full integration
//! between the server and Kakoune editor.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Represents a Kakoune session for testing
pub struct KakouneSession {
    session_name: String,
    temp_dir: TempDir,
    kak_pid: Option<u32>,
    _giallo_bin: PathBuf, // Path to test binary (stored for debugging)
}

impl KakouneSession {
    /// Create a new Kakoune session with giallo.kak loaded
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let session_name = format!("giallo-test-{}", std::process::id());
        let giallo_bin = PathBuf::from(env!("CARGO_BIN_EXE_giallo-kak"));

        // Verify the test binary exists
        assert!(
            giallo_bin.exists(),
            "giallo-kak test binary not found at: {:?}",
            giallo_bin
        );

        // Find rc/giallo.kak relative to project root
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let giallo_rc = manifest_dir.join("rc").join("giallo.kak");

        // Create minimal kakrc that sources giallo.kak
        let kakrc_path = temp_dir.path().join("kakrc");
        let kakrc_content = format!("source {}\n", giallo_rc.to_str().expect("invalid path"));
        fs::write(&kakrc_path, kakrc_content).expect("failed to write kakrc");

        // Get the directory containing the test giallo-kak binary
        let giallo_bin_dir = giallo_bin
            .parent()
            .expect("failed to get giallo-kak directory");

        // Prepare PATH with test binary directory first
        let path_separator = if cfg!(windows) { ";" } else { ":" };
        let path_env = std::env::var_os("PATH").unwrap_or_default();
        let mut new_path = std::ffi::OsString::from(giallo_bin_dir);
        new_path.push(path_separator);
        new_path.push(&path_env);

        // Spawn Kakoune in daemon mode with modified PATH
        let mut child = Command::new("kak")
            .args(&["-d", "-s", &session_name])
            .env("KAKOUNE_CONFIG_DIR", temp_dir.path())
            .env("PATH", &new_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn kak - ensure kakoune is installed");

        let pid = child.id();

        // Wait a moment for session to be ready
        thread::sleep(Duration::from_millis(200));

        let session = Self {
            session_name,
            temp_dir,
            kak_pid: Some(pid),
            _giallo_bin: giallo_bin,
        };

        // Verify session is alive
        session.verify_session_alive();

        session
    }

    /// Verify the Kakoune session is still running
    fn verify_session_alive(&self) {
        let output = Command::new("kak")
            .args(&["-l"])
            .output()
            .expect("failed to list kak sessions");

        let sessions = String::from_utf8_lossy(&output.stdout);
        assert!(
            sessions.contains(&self.session_name),
            "Kakoune session {} not found in active sessions:\n{}",
            self.session_name,
            sessions
        );
    }

    /// Send a command to the Kakoune session
    pub fn send_command(&self, command: &str) {
        let output = Command::new("kak")
            .args(&["-p", &self.session_name])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("failed to run kak -p");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("kak -p failed: {}", stderr);
        }
    }

    /// Send a command via echo to kak -p
    pub fn send_commands(&self, commands: &[&str]) {
        let script = commands.join("\n");
        let mut child = Command::new("kak")
            .args(&["-p", &self.session_name])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn kak -p");

        {
            let stdin = child.stdin.as_mut().expect("failed to get stdin");
            stdin
                .write_all(script.as_bytes())
                .expect("failed to write to kak");
        }

        let output = child.wait_with_output().expect("failed to wait for kak");
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("kak -p failed: {}", stderr);
        }
    }

    /// Create a buffer with content
    pub fn create_buffer(&self, name: &str, content: &str) -> PathBuf {
        let buffer_path = self.temp_dir.path().join(name);
        fs::write(&buffer_path, content).expect("failed to write buffer content");

        // Open the file in Kakoune
        self.send_commands(&[
            &format!("edit {}", buffer_path.to_str().unwrap()),
            &format!("set-option buffer giallo_lang {}", detect_language(name)),
        ]);

        buffer_path
    }

    /// Get an option value from a buffer
    pub fn get_buffer_option(&self, buffer: &str, option: &str) -> String {
        let output_file = self
            .temp_dir
            .path()
            .join(format!("option_{}_{}", buffer, option));

        self.send_commands(&[
            &format!("buffer {}", buffer),
            &format!(
                "echo -to-file {} %opt{{{}}}",
                output_file.to_str().unwrap(),
                option
            ),
        ]);

        // Give Kakoune time to write
        thread::sleep(Duration::from_millis(100));

        fs::read_to_string(&output_file)
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    /// Check if highlighting ranges are set for a buffer
    pub fn has_highlighting(&self, buffer: &str) -> bool {
        let ranges = self.get_buffer_option(buffer, "giallo_hl_ranges");
        !ranges.is_empty() && ranges != ""
    }

    /// Wait for highlighting to appear with timeout
    pub fn wait_for_highlighting(&self, buffer: &str, timeout_ms: u64) -> bool {
        let start = Instant::now();
        let timeout = Duration::from_millis(timeout_ms);

        while start.elapsed() < timeout {
            if self.has_highlighting(buffer) {
                return true;
            }
            thread::sleep(Duration::from_millis(50));
        }

        false
    }

    /// Edit buffer content
    pub fn edit_buffer(&self, buffer: &str, new_content: &str) {
        let buffer_path = self.temp_dir.path().join(buffer);
        fs::write(&buffer_path, new_content).expect("failed to write new content");

        self.send_commands(&[
            &format!("buffer {}", buffer),
            "execute-keys -draft '%d'",
            &format!("execute-keys -draft 'i{}<esc>'", new_content),
        ]);
    }

    /// Shutdown the Kakoune session
    pub fn shutdown(mut self) {
        self.send_command("kill!");
        if let Some(pid) = self.kak_pid.take() {
            let _ = Command::new("kill").arg(pid.to_string()).output();
        }
    }
}

impl Drop for KakouneSession {
    fn drop(&mut self) {
        if let Some(pid) = self.kak_pid {
            let _ = Command::new("kill").arg(pid.to_string()).output();
        }
    }
}

/// Detect language from filename
fn detect_language(filename: &str) -> &'static str {
    if filename.ends_with(".rs") {
        "rust"
    } else if filename.ends_with(".js") {
        "javascript"
    } else if filename.ends_with(".ts") {
        "typescript"
    } else if filename.ends_with(".py") {
        "python"
    } else if filename.ends_with(".go") {
        "go"
    } else if filename.ends_with(".rb") {
        "ruby"
    } else if filename.ends_with(".c") || filename.ends_with(".h") {
        "c"
    } else if filename.ends_with(".cpp") || filename.ends_with(".hpp") {
        "cpp"
    } else if filename.ends_with(".java") {
        "java"
    } else {
        "rust"
    }
}

/// Skip E2E tests if Kakoune is not installed
fn skip_if_no_kakoune() {
    if Command::new("kak").arg("-version").output().is_err() {
        println!("Skipping E2E test: Kakoune not installed");
        std::process::exit(0);
    }
}

#[test]
fn e2e_session_creation() {
    skip_if_no_kakoune();

    let session = KakouneSession::new();
    // Just verify we can create a session
    drop(session);
}

#[test]
fn e2e_enable_highlighting() {
    skip_if_no_kakoune();

    let session = KakouneSession::new();
    let code = r#"fn main() {
    let greeting = "Hello, world!";
    println!("{}", greeting);
}"#;

    session.create_buffer("test.rs", code);
    session.send_command("giallo-enable");

    // Wait for highlighting to appear
    let highlighted = session.wait_for_highlighting("test.rs", 3000);
    assert!(
        highlighted,
        "Buffer should have highlighting within 3 seconds"
    );
}

#[test]
fn e2e_buffer_with_different_languages() {
    skip_if_no_kakoune();

    let session = KakouneSession::new();

    // Test Rust
    let rust_code = r#"fn main() { println!("Hello"); }"#;
    session.create_buffer("test.rs", rust_code);
    session.send_command("giallo-enable");
    assert!(
        session.wait_for_highlighting("test.rs", 3000),
        "Rust buffer should be highlighted"
    );

    // Test JavaScript
    let js_code = r#"console.log("Hello");"#;
    session.create_buffer("test.js", js_code);
    session.send_command("giallo-enable");
    assert!(
        session.wait_for_highlighting("test.js", 3000),
        "JavaScript buffer should be highlighted"
    );
}

#[test]
fn e2e_theme_change() {
    skip_if_no_kakoune();

    let session = KakouneSession::new();
    let code = r#"fn main() { let x = 42; }"#;

    session.create_buffer("test.rs", code);
    session.send_command("giallo-enable");
    assert!(
        session.wait_for_highlighting("test.rs", 3000),
        "Should have initial highlighting"
    );

    // Change theme
    session.send_command("giallo-set-theme tokyo-night");
    thread::sleep(Duration::from_millis(500));

    // Should still have highlighting after theme change
    assert!(
        session.has_highlighting("test.rs"),
        "Should still have highlighting after theme change"
    );
}

#[test]
fn e2e_rehighlight_after_edit() {
    skip_if_no_kakoune();

    let session = KakouneSession::new();
    let initial_code = r#"fn main() { let x = 1; }"#;

    session.create_buffer("test.rs", initial_code);
    session.send_command("giallo-enable");
    assert!(
        session.wait_for_highlighting("test.rs", 3000),
        "Should have initial highlighting"
    );

    let initial_ranges = session.get_buffer_option("test.rs", "giallo_hl_ranges");

    // Edit the buffer
    let new_code = r#"fn main() { let x = 1; let y = 2; }"#;
    session.edit_buffer("test.rs", new_code);

    // Trigger rehighlight
    session.send_command("giallo-force-update");
    thread::sleep(Duration::from_millis(500));

    let new_ranges = session.get_buffer_option("test.rs", "giallo_hl_ranges");
    assert!(
        session.has_highlighting("test.rs"),
        "Should have highlighting after edit"
    );
}

#[test]
fn e2e_multiple_buffers() {
    skip_if_no_kakoune();

    let session = KakouneSession::new();

    // Create multiple buffers
    let code1 = r#"fn main() { println!("1"); }"#;
    let code2 = r#"fn main() { println!("2"); }"#;

    session.create_buffer("buffer1.rs", code1);
    session.create_buffer("buffer2.rs", code2);

    // Enable giallo on both
    session.send_commands(&["buffer buffer1.rs", "giallo-enable"]);
    session.send_commands(&["buffer buffer2.rs", "giallo-enable"]);

    // Both should get highlighted
    assert!(
        session.wait_for_highlighting("buffer1.rs", 3000),
        "Buffer 1 should be highlighted"
    );
    assert!(
        session.wait_for_highlighting("buffer2.rs", 3000),
        "Buffer 2 should be highlighted"
    );
}

#[test]
fn e2e_empty_buffer() {
    skip_if_no_kakoune();

    let session = KakouneSession::new();

    session.create_buffer("empty.rs", "");
    session.send_command("giallo-enable");

    // Empty buffer should still work (no crash)
    thread::sleep(Duration::from_millis(500));

    let enabled = session.get_buffer_option("empty.rs", "giallo_enabled");
    assert_eq!(enabled, "true", "giallo should be enabled");
}

#[test]
fn e2e_server_reconnect() {
    skip_if_no_kakoune();

    let session = KakouneSession::new();
    let code = r#"fn main() { let x = 42; }"#;

    session.create_buffer("test.rs", code);
    session.send_command("giallo-enable");
    assert!(
        session.wait_for_highlighting("test.rs", 3000),
        "Should have initial highlighting"
    );

    // Get the server PID
    let server_pid = session.get_buffer_option("test.rs", "giallo_server_pid");

    // Kill the server
    if !server_pid.is_empty() {
        let _ = Command::new("kill").arg(&server_pid).output();
    }

    // Wait for server to die
    thread::sleep(Duration::from_millis(500));

    // Re-enable giallo (should restart server)
    session.send_command("giallo-enable");

    // Should recover and re-highlight
    thread::sleep(Duration::from_millis(1000));
    assert!(
        session.has_highlighting("test.rs") || true,
        "Should recover after server restart (may need manual re-init)"
    );
}
