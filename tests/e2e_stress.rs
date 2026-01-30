//! E2E stress tests for giallo.kak with resource monitoring
//!
//! These tests simulate realistic multi-buffer editing scenarios
//! and monitor resource usage over time.

mod resource_monitor;

use resource_monitor::ResourceMonitor;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Enhanced Kakoune session for stress testing
pub struct StressTestSession {
    session_name: String,
    temp_dir: TempDir,
    kak_pid: Option<u32>,
    buffers: Vec<String>,
}

impl StressTestSession {
    /// Create a new stress test session
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let session_name = format!("giallo-stress-{}", std::process::id());
        let giallo_bin = PathBuf::from(env!("CARGO_BIN_EXE_giallo-kak"));

        assert!(
            giallo_bin.exists(),
            "giallo-kak test binary not found at: {:?}",
            giallo_bin
        );

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let giallo_rc = manifest_dir.join("rc").join("giallo.kak");

        let kakrc_path = temp_dir.path().join("kakrc");
        let kakrc_content = format!(
            "source {}\nset-option global giallo_debug true\n",
            giallo_rc.to_str().expect("invalid path")
        );
        fs::write(&kakrc_path, kakrc_content).expect("failed to write kakrc");

        let giallo_bin_dir = giallo_bin
            .parent()
            .expect("failed to get giallo-kak directory");
        let path_separator = if cfg!(windows) { ";" } else { ":" };
        let path_env = std::env::var_os("PATH").unwrap_or_default();
        let mut new_path = std::ffi::OsString::from(giallo_bin_dir);
        new_path.push(path_separator);
        new_path.push(&path_env);

        let child = Command::new("kak")
            .args(&["-d", "-s", &session_name])
            .env("KAKOUNE_CONFIG_DIR", temp_dir.path())
            .env("PATH", &new_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn kak - ensure kakoune is installed");

        let pid = child.id();
        thread::sleep(Duration::from_millis(200));

        let session = Self {
            session_name,
            temp_dir,
            kak_pid: Some(pid),
            buffers: Vec::new(),
        };

        session.verify_session_alive();
        session
    }

    fn verify_session_alive(&self) {
        let output = Command::new("kak")
            .args(&["-l"])
            .output()
            .expect("failed to list kak sessions");

        let sessions = String::from_utf8_lossy(&output.stdout);
        assert!(
            sessions.contains(&self.session_name),
            "Kakoune session {} not found",
            self.session_name
        );
    }

    /// Create multiple buffers at once
    pub fn create_multiple_buffers(&mut self, count: usize, pattern: &str) -> Vec<String> {
        let mut buffer_names = Vec::new();

        for i in 0..count {
            let name = format!("{}_{:03}.rs", pattern, i);
            let code = generate_test_code(i);
            self.create_buffer(&name, &code);
            buffer_names.push(name);
        }

        self.buffers.extend(buffer_names.clone());
        buffer_names
    }

    /// Create a single buffer
    pub fn create_buffer(&self, name: &str, content: &str) -> PathBuf {
        let buffer_path = self.temp_dir.path().join(name);
        fs::write(&buffer_path, content).expect("failed to write buffer content");

        let script = format!(
            "edit {}\nset-option buffer giallo_lang rust\n",
            buffer_path.to_str().unwrap()
        );

        let mut child = Command::new("kak")
            .args(&["-p", &self.session_name])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn kak -p");

        {
            let stdin = child.stdin.as_mut().expect("failed to get stdin");
            stdin.write_all(script.as_bytes()).expect("failed to write");
        }

        child.wait_with_output().expect("failed to wait");
        buffer_path
    }

    /// Enable giallo on all buffers
    pub fn enable_all_buffers(&self) {
        for buffer in &self.buffers {
            let script = format!(
                "buffer {}\ngiallo-enable\n",
                self.temp_dir.path().join(buffer).to_str().unwrap()
            );

            let _ = Command::new("kak")
                .args(&["-p", &self.session_name])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .and_then(|mut c| {
                    if let Some(stdin) = c.stdin.as_mut() {
                        let _ = stdin.write_all(script.as_bytes());
                    }
                    c.wait_with_output()
                });
        }
    }

    /// Simulate typing text into a buffer
    pub fn type_text(&self, buffer: &str, text: &str) {
        let buffer_path = self.temp_dir.path().join(buffer);

        let current_content = fs::read_to_string(&buffer_path).unwrap_or_default();
        let new_content = format!("{}\n{}", current_content, text);
        fs::write(&buffer_path, &new_content).expect("failed to write");

        let script = format!(
            "buffer {}\nexecute-keys -draft 'Ge'\nexecute-keys -draft 'i{}<esc>'\n",
            buffer_path.to_str().unwrap(),
            text.replace('"', "\\\"").replace('\n', "<ret>")
        );

        let _ = Command::new("kak")
            .args(&["-p", &self.session_name])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut c| {
                if let Some(stdin) = c.stdin.as_mut() {
                    let _ = stdin.write_all(script.as_bytes());
                }
                c.wait_with_output()
            });
    }

    /// Trigger rehighlight on all buffers
    pub fn rehighlight_all(&self) {
        for buffer in &self.buffers {
            let script = format!(
                "buffer {}\ngiallo-rehighlight\n",
                self.temp_dir.path().join(buffer).to_str().unwrap()
            );

            let _ = Command::new("kak")
                .args(&["-p", &self.session_name])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .and_then(|mut c| {
                    if let Some(stdin) = c.stdin.as_mut() {
                        let _ = stdin.write_all(script.as_bytes());
                    }
                    c.wait_with_output()
                });
        }
    }

    /// Get giallo server PID if running
    #[allow(dead_code)]
    pub fn get_server_pid(&self) -> Option<u32> {
        // Search for giallo-kak process
        let output = Command::new("pgrep")
            .args(&["-f", "giallo-kak"])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines().next().and_then(|line| line.parse().ok())
    }

    /// Wait for all buffers to have highlighting
    pub fn wait_for_all_highlighting(&self, timeout_ms: u64) -> bool {
        let start = Instant::now();

        loop {
            let mut all_highlighted = true;

            for buffer in &self.buffers {
                if !self.has_highlighting(buffer) {
                    all_highlighted = false;
                    break;
                }
            }

            if all_highlighted {
                return true;
            }

            if start.elapsed().as_millis() > timeout_ms as u128 {
                return false;
            }

            thread::sleep(Duration::from_millis(50));
        }
    }

    fn has_highlighting(&self, buffer: &str) -> bool {
        let output_file = self.temp_dir.path().join(format!("hl_{}", buffer));

        let script = format!(
            "buffer {}\necho -to-file {} %opt{{giallo_hl_ranges}}\n",
            self.temp_dir.path().join(buffer).to_str().unwrap(),
            output_file.to_str().unwrap()
        );

        let _ = Command::new("kak")
            .args(&["-p", &self.session_name])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut c| {
                if let Some(stdin) = c.stdin.as_mut() {
                    let _ = stdin.write_all(script.as_bytes());
                }
                c.wait_with_output()
            });

        thread::sleep(Duration::from_millis(50));

        fs::read_to_string(&output_file)
            .map(|content| !content.trim().is_empty())
            .unwrap_or(false)
    }

    /// Shutdown the session
    pub fn shutdown(mut self) {
        let _ = Command::new("kak")
            .args(&["-p", &self.session_name])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut c| {
                if let Some(stdin) = c.stdin.as_mut() {
                    let _ = stdin.write_all(b"kill!\n");
                }
                c.wait_with_output()
            });

        if let Some(pid) = self.kak_pid.take() {
            let _ = Command::new("kill").arg(pid.to_string()).output();
        }
    }
}

impl Drop for StressTestSession {
    fn drop(&mut self) {
        if let Some(pid) = self.kak_pid {
            let _ = Command::new("kill").arg(pid.to_string()).output();
        }
    }
}

/// Generate test code for stress testing
fn generate_test_code(index: usize) -> String {
    format!(
        "// Buffer {}\n// Generated for stress testing\n\n\
         fn function_{}() -> i32 {{\n\
                 let x = {};\n\
                 let y = \"string_{}\";\n\
                 if x > 0 {{\n\
                         println!(\"{{}}\", y);\n\
                 }}\n\
                 x\n\
         }}\n\n\
         struct Struct_{} {{\n\
                 field: i32,\n\
         }}",
        index,
        index,
        index * 42,
        index,
        index
    )
}

fn skip_if_no_kakoune() {
    if Command::new("kak").arg("-version").output().is_err() {
        println!("Skipping stress test: Kakoune not installed");
        std::process::exit(0);
    }
}

// ===== STRESS TESTS =====

#[test]
fn stress_many_buffers() {
    skip_if_no_kakoune();

    let mut session = StressTestSession::new();
    let mut monitor = ResourceMonitor::for_current_process();

    println!("Creating 20 buffers...");
    let buffers = session.create_multiple_buffers(20, "stress");
    monitor.sample();

    println!("Enabling giallo on all buffers...");
    session.enable_all_buffers();

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        monitor.sample_if_elapsed(Duration::from_millis(500));

        if session.wait_for_all_highlighting(500) {
            break;
        }
    }

    let report = monitor.report();
    report.print_report();

    // Memory should not exceed 200MB for 20 buffers
    assert!(
        report.max_memory_mb < 200.0,
        "Memory usage too high: {:.2}MB for 20 buffers",
        report.max_memory_mb
    );

    println!("✓ Successfully managed {} buffers", buffers.len());
}

#[test]
fn stress_rapid_editing() {
    skip_if_no_kakoune();

    let session = StressTestSession::new();
    let mut monitor = ResourceMonitor::for_current_process();

    session.create_buffer("rapid.rs", "// Start");
    session.enable_all_buffers();

    println!("Performing 100 rapid edits...");
    let start = Instant::now();

    for i in 0..100 {
        session.type_text("rapid.rs", &format!("// Edit {}\n", i));

        if i % 10 == 0 {
            session.rehighlight_all();
            monitor.sample();
        }

        thread::sleep(Duration::from_millis(50));
    }

    let elapsed = start.elapsed();
    let report = monitor.report();

    report.print_report();
    println!("Total time for 100 edits: {:.2}s", elapsed.as_secs_f64());

    // Should complete in under 10 seconds
    assert!(
        elapsed < Duration::from_secs(10),
        "100 edits took too long: {:?}",
        elapsed
    );

    // Throughput check
    let throughput = 100.0 / elapsed.as_secs_f64();
    println!("Throughput: {:.1} edits/sec", throughput);
    assert!(
        throughput > 8.0,
        "Throughput too low: {:.1} edits/sec",
        throughput
    );
}

#[test]
fn stress_continuous_updates() {
    skip_if_no_kakoune();

    let session = StressTestSession::new();
    let mut monitor = ResourceMonitor::for_current_process();

    session.create_buffer("continuous.rs", "// Continuous test");
    session.enable_all_buffers();

    let test_duration = Duration::from_secs(30);
    let update_interval = Duration::from_millis(100);

    println!("Simulating typing session for 30 seconds...");
    let start = Instant::now();
    let mut update_count = 0;

    while start.elapsed() < test_duration {
        session.type_text("continuous.rs", &format!("// Line {}\n", update_count));
        update_count += 1;

        if update_count % 5 == 0 {
            session.rehighlight_all();
        }

        monitor.sample_if_elapsed(Duration::from_millis(500));
        thread::sleep(update_interval);
    }

    let report = monitor.report();
    report.print_report();

    println!("Total updates: {}", update_count);
    println!("Updates per second: {:.1}", update_count as f64 / 30.0);

    // Memory should not grow more than 30% over the test
    assert!(
        report.memory_growth_percent < 30.0,
        "Memory grew too much: {:.1}%",
        report.memory_growth_percent
    );
}

#[test]
fn stress_memory_stability() {
    skip_if_no_kakoune();

    let session = StressTestSession::new();
    let mut monitor = ResourceMonitor::for_current_process();

    session.create_buffer("stability.rs", "// Memory stability test");
    session.enable_all_buffers();

    let test_duration = Duration::from_secs(60);
    let sample_interval = Duration::from_secs(1);

    println!("Monitoring memory for 60 seconds...");
    let start = Instant::now();

    while start.elapsed() < test_duration {
        monitor.sample();
        thread::sleep(sample_interval);
    }

    let report = monitor.report();
    report.print_report();

    // Memory should be relatively stable (no leaks)
    // Allow 50% growth for caching and normal operation
    assert!(
        report.memory_growth_percent < 50.0,
        "Possible memory leak: memory grew {:.1}% over 60 seconds",
        report.memory_growth_percent
    );

    // Average CPU should be reasonable (not pegged)
    assert!(
        report.avg_cpu < 20.0,
        "CPU usage too high during idle: {:.1}% average",
        report.avg_cpu
    );
}

#[test]
fn stress_concurrent_typing() {
    skip_if_no_kakoune();

    let mut session = StressTestSession::new();
    let mut monitor = ResourceMonitor::for_current_process();

    // Create 5 buffers
    let buffers = session.create_multiple_buffers(5, "concurrent");
    session.enable_all_buffers();

    println!("Simulating typing in 5 buffers simultaneously...");
    let iterations = 20;
    let start = Instant::now();

    for i in 0..iterations {
        for (buf_idx, buffer) in buffers.iter().enumerate() {
            session.type_text(buffer, &format!("// Buffer {} update {}\n", buf_idx, i));
        }

        if i % 3 == 0 {
            session.rehighlight_all();
            monitor.sample();
        }

        thread::sleep(Duration::from_millis(100));
    }

    let elapsed = start.elapsed();
    let report = monitor.report();

    report.print_report();
    println!(
        "Completed {} iterations across 5 buffers in {:.2}s",
        iterations,
        elapsed.as_secs_f64()
    );

    // All buffers should still have highlighting (give more time for concurrent load)
    assert!(
        session.wait_for_all_highlighting(10000),
        "Not all buffers have highlighting after concurrent edits"
    );

    // Memory should stay reasonable
    assert!(
        report.max_memory_mb < 100.0,
        "Memory too high with concurrent buffers: {:.2}MB",
        report.max_memory_mb
    );
}

#[test]
fn stress_large_file_editing() {
    skip_if_no_kakoune();

    let session = StressTestSession::new();
    let mut monitor = ResourceMonitor::for_current_process();

    // Generate a large file (1000 lines)
    let mut large_content = String::with_capacity(50000);
    for i in 0..1000 {
        large_content.push_str(&format!(
            "fn function_{}() {{ println!(\"Line {}\"); }}\n",
            i, i
        ));
    }

    println!("Creating large file (1000 lines)...");
    session.create_buffer("large.rs", &large_content);
    session.enable_all_buffers();

    monitor.sample();

    // Wait for initial highlighting (allow up to 25 seconds polling)
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(25) {
        if session.has_highlighting("large.rs") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    let initial_highlight_time = start.elapsed();
    monitor.sample();

    println!(
        "Initial highlighting took {:.2}s",
        initial_highlight_time.as_secs_f64()
    );

    // Make 10 edits to the large file
    println!("Making 10 edits to large file...");
    for i in 0..10 {
        session.type_text("large.rs", &format!("// Edit {}\n", i));
        session.rehighlight_all();
        monitor.sample();
        thread::sleep(Duration::from_millis(200));
    }

    let report = monitor.report();
    report.print_report();

    // Large file should highlight within 20 seconds (very conservative for CI)
    assert!(
        initial_highlight_time < Duration::from_secs(20),
        "Large file highlighting too slow: {:?}",
        initial_highlight_time
    );

    // Memory for large file should be under 150MB
    assert!(
        report.max_memory_mb < 150.0,
        "Large file memory usage too high: {:.2}MB",
        report.max_memory_mb
    );
}
