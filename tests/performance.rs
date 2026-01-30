//! Performance tests for giallo.kak
//!
//! These tests benchmark highlighting performance across various file sizes
//! and measure memory usage, CPU overhead, and throughput.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;
use sysinfo::{get_current_pid, System};

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn get_unique_id() -> usize {
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

/// Results from a highlighting benchmark
#[derive(Debug)]
pub struct BenchmarkResult {
    pub file_lines: usize,
    pub file_bytes: usize,
    pub highlight_time_ms: f64,
    pub memory_delta_mb: f64,
    pub output_size_bytes: usize,
}

/// Make a temporary directory with a unique name
fn make_temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let unique = format!(
        "{}-{}-{}-{}",
        prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        get_unique_id()
    );
    dir.push(unique);
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

/// Write a config file to the temp directory
fn write_config(config_dir: &Path, theme: &str) {
    let cfg_dir = config_dir.join("giallo.kak");
    fs::create_dir_all(&cfg_dir).expect("failed to create config dir");
    let config_path = cfg_dir.join("config.toml");
    let contents = format!("theme = \"{}\"\n", theme);
    fs::write(&config_path, contents).expect("failed to write config");
}

/// Run a oneshot highlight and measure performance
fn benchmark_oneshot_highlight(lang: &str, theme: &str, code: &str) -> BenchmarkResult {
    let config_home = make_temp_dir("giallo-kak-perf");
    write_config(&config_home, theme);

    let file_lines = code.lines().count();
    let file_bytes = code.len();

    // Initialize system info
    let mut system = System::new_all();
    system.refresh_all();
    let pid = get_current_pid().expect("failed to get current pid");
    let process = system.process(pid).expect("failed to get current process");
    let memory_before = process.memory() as f64 / 1024.0 / 1024.0; // MB

    // Prepare input
    let payload = code.as_bytes();
    let header = format!("H {} {} {}\n", lang, theme, payload.len());

    let bin = env!("CARGO_BIN_EXE_giallo-kak");

    // Run highlighting
    let start = Instant::now();

    let mut child = Command::new(bin)
        .arg("--oneshot")
        .env("XDG_CONFIG_HOME", &config_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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

    let highlight_time = start.elapsed().as_secs_f64() * 1000.0; // Convert to ms

    assert!(
        output.status.success(),
        "giallo-kak failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Measure memory after
    system.refresh_all();
    let process = system.process(pid).expect("failed to get current process");
    let memory_after = process.memory() as f64 / 1024.0 / 1024.0;
    let memory_delta = memory_after - memory_before;

    let output_size = output.stdout.len();

    BenchmarkResult {
        file_lines,
        file_bytes,
        highlight_time_ms: highlight_time,
        memory_delta_mb: memory_delta,
        output_size_bytes: output_size,
    }
}

/// Generate synthetic Rust code of specified line count
pub fn generate_rust_file(lines: usize) -> String {
    let mut code = String::with_capacity(lines * 50);

    // Header
    code.push_str("// Generated test file for performance testing\n");
    code.push_str("// Lines: ");
    code.push_str(&lines.to_string());
    code.push('\n');
    code.push_str("// Purpose: Benchmark highlighting performance\n\n");

    let lines_remaining = lines.saturating_sub(4);
    let functions_needed = lines_remaining / 10 + 1;

    for i in 0..functions_needed {
        if code.lines().count() >= lines {
            break;
        }

        // Generate a function with various complexity
        let fn_name = format!("function_{}_{:04}", i, i * 1234 % 10000);
        code.push_str(&format!("fn {}() -> Result<(), String> {{\n", fn_name));

        // Add some variables
        code.push_str(&format!("    let x_{} = {};\n", i, i * 42));
        code.push_str(&format!("    let y_{} = \"string_{}\";\n", i, i));
        code.push_str(&format!(
            "    let z_{} = vec![{}, {}, {}];\n",
            i,
            i,
            i + 1,
            i + 2
        ));

        // Add control flow
        code.push_str(&format!("    if x_{} > {} {{\n", i, i * 10));
        code.push_str(&format!("        println!(\"Value: {{}}\", y_{});\n", i));
        code.push_str("    } else {\n");
        code.push_str(&format!(
            "        return Err(\"Error {}\".to_string());\n",
            i
        ));
        code.push_str("    }\n");

        // Add a loop
        code.push_str(&format!("    for i in 0..{} {{\n", i % 10 + 1));
        code.push_str(&format!("        if i == {} {{\n", i % 5));
        code.push_str("            continue;\n");
        code.push_str("        }\n");
        code.push_str("    }\n");

        // Add match expression
        code.push_str(&format!("    match x_{} {{\n", i));
        code.push_str(&format!("        {} => println!(\"First\"),\n", i * 42));
        code.push_str(&format!(
            "        {} => println!(\"Second\"),\n",
            i * 42 + 1
        ));
        code.push_str("        _ => println!(\"Other\"),\n");
        code.push_str("    }\n");

        // Return
        code.push_str("    Ok(())\n");
        code.push_str("}\n\n");
    }

    // Add a struct
    code.push_str("#[derive(Debug, Clone)]\n");
    code.push_str("struct TestStruct {\n");
    code.push_str("    field1: i32,\n");
    code.push_str("    field2: String,\n");
    code.push_str("    field3: Vec<u8>,\n");
    code.push_str("}\n\n");

    // Add impl block
    code.push_str("impl TestStruct {\n");
    code.push_str("    fn new(f1: i32, f2: String) -> Self {\n");
    code.push_str("        Self {\n");
    code.push_str("            field1: f1,\n");
    code.push_str("            field2: f2,\n");
    code.push_str("            field3: Vec::new(),\n");
    code.push_str("        }\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");

    // Pad to reach target line count if needed
    while code.lines().count() < lines {
        code.push_str("// padding line\n");
    }

    code
}

/// Generate JavaScript code for testing
pub fn generate_javascript_file(lines: usize) -> String {
    let mut code = String::with_capacity(lines * 40);

    code.push_str("// Generated JavaScript test file\n");
    code.push_str(&format!("// Lines: {}\n\n", lines));

    let functions_needed = lines / 8 + 1;

    for i in 0..functions_needed {
        if code.lines().count() >= lines {
            break;
        }

        let fn_name = format!("function_{}", i);
        code.push_str(&format!("function {}(arg{}) {{\n", fn_name, i));
        code.push_str(&format!("    const x = {};\n", i * 100));
        code.push_str(&format!("    const y = 'string_{}';\n", i));
        code.push_str(&format!("    const arr = [{}, {}, {}];\n", i, i + 1, i + 2));

        code.push_str(&format!("    if (x > {}) {{\n", i * 50));
        code.push_str(&format!("        console.log(`Value: ${{y}}`);\n",));
        code.push_str("    } else {\n");
        code.push_str(&format!(
            "        throw new Error('Error in {}');\n",
            fn_name
        ));
        code.push_str("    }\n");

        code.push_str(&format!(
            "    for (let j = 0; j < {}; j++) {{\n",
            i % 10 + 1
        ));
        code.push_str("        if (j === 5) continue;\n");
        code.push_str("        console.log(j);\n");
        code.push_str("    }\n");

        code.push_str("    return x;\n");
        code.push_str("}\n\n");
    }

    // Add class
    code.push_str("class TestClass {\n");
    code.push_str("    constructor(value) {\n");
    code.push_str("        this.value = value;\n");
    code.push_str("    }\n");
    code.push_str("    getValue() {\n");
    code.push_str("        return this.value;\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");

    while code.lines().count() < lines {
        code.push_str("// padding\n");
    }

    code
}

/// Generate Python code for testing
pub fn generate_python_file(lines: usize) -> String {
    let mut code = String::with_capacity(lines * 35);

    code.push_str("# Generated Python test file\n");
    code.push_str(&format!("# Lines: {}\n\n", lines));

    let functions_needed = lines / 7 + 1;

    for i in 0..functions_needed {
        if code.lines().count() >= lines {
            break;
        }

        let fn_name = format!("function_{}", i);
        code.push_str(&format!("def {}(arg{}):\n", fn_name, i));
        code.push_str(&format!("    x = {}\n", i * 100));
        code.push_str(&format!("    y = 'string_{}'\n", i));
        code.push_str(&format!("    arr = [{}, {}, {}]\n", i, i + 1, i + 2));

        code.push_str(&format!("    if x > {}:\n", i * 50));
        code.push_str(&format!("        print(f'Value: {{y}}')\n",));
        code.push_str("    else:\n");
        code.push_str(&format!(
            "        raise ValueError('Error in {}')\n",
            fn_name
        ));

        code.push_str(&format!("    for j in range({}):\n", i % 10 + 1));
        code.push_str("        if j == 5:\n");
        code.push_str("            continue\n");
        code.push_str("        print(j)\n");

        code.push_str("    return x\n\n");
    }

    // Add class
    code.push_str("class TestClass:\n");
    code.push_str("    def __init__(self, value):\n");
    code.push_str("        self.value = value\n");
    code.push_str("    def get_value(self):\n");
    code.push_str("        return self.value\n\n");

    while code.lines().count() < lines {
        code.push_str("# padding\n");
    }

    code
}

#[test]
fn perf_highlight_small_file_rust() {
    let code = generate_rust_file(100);
    let result = benchmark_oneshot_highlight("rust", "catppuccin-frappe", &code);

    println!(
        "Small file ({} lines, {} bytes): {:.2}ms, memory delta: {:.2}MB, output: {} bytes",
        result.file_lines,
        result.file_bytes,
        result.highlight_time_ms,
        result.memory_delta_mb,
        result.output_size_bytes
    );

    // Conservative threshold: < 300ms for small files (accounts for initial registry load)
    assert!(
        result.highlight_time_ms < 300.0,
        "Small file should highlight in <300ms, took {:.2}ms",
        result.highlight_time_ms
    );

    // Memory threshold: < 20MB delta
    assert!(
        result.memory_delta_mb < 20.0,
        "Memory delta should be <20MB, was {:.2}MB",
        result.memory_delta_mb
    );
}

#[test]
fn perf_highlight_medium_file_rust() {
    let code = generate_rust_file(1000);
    let result = benchmark_oneshot_highlight("rust", "catppuccin-frappe", &code);

    println!(
        "Medium file ({} lines, {} bytes): {:.2}ms, memory delta: {:.2}MB, output: {} bytes",
        result.file_lines,
        result.file_bytes,
        result.highlight_time_ms,
        result.memory_delta_mb,
        result.output_size_bytes
    );

    // Conservative threshold: < 1000ms for medium files
    assert!(
        result.highlight_time_ms < 1000.0,
        "Medium file should highlight in <1000ms, took {:.2}ms",
        result.highlight_time_ms
    );

    // Memory threshold: < 50MB delta
    assert!(
        result.memory_delta_mb < 50.0,
        "Memory delta should be <50MB, was {:.2}MB",
        result.memory_delta_mb
    );
}

#[test]
fn perf_highlight_large_file_rust() {
    let code = generate_rust_file(10000);
    let result = benchmark_oneshot_highlight("rust", "catppuccin-frappe", &code);

    println!(
        "Large file ({} lines, {} bytes): {:.2}ms, memory delta: {:.2}MB, output: {} bytes",
        result.file_lines,
        result.file_bytes,
        result.highlight_time_ms,
        result.memory_delta_mb,
        result.output_size_bytes
    );

    // Conservative threshold: < 5000ms for large files
    assert!(
        result.highlight_time_ms < 5000.0,
        "Large file should highlight in <5000ms, took {:.2}ms",
        result.highlight_time_ms
    );

    // Memory threshold: < 150MB delta
    assert!(
        result.memory_delta_mb < 150.0,
        "Memory delta should be <150MB, was {:.2}MB",
        result.memory_delta_mb
    );
}

#[test]
fn perf_highlight_small_file_javascript() {
    let code = generate_javascript_file(100);
    let result = benchmark_oneshot_highlight("javascript", "catppuccin-frappe", &code);

    println!(
        "JS small file ({} lines): {:.2}ms, memory delta: {:.2}MB",
        result.file_lines, result.highlight_time_ms, result.memory_delta_mb
    );

    assert!(
        result.highlight_time_ms < 300.0,
        "JS small file should highlight in <300ms, took {:.2}ms",
        result.highlight_time_ms
    );
}

#[test]
fn perf_highlight_medium_file_javascript() {
    let code = generate_javascript_file(1000);
    let result = benchmark_oneshot_highlight("javascript", "catppuccin-frappe", &code);

    println!(
        "JS medium file ({} lines): {:.2}ms, memory delta: {:.2}MB",
        result.file_lines, result.highlight_time_ms, result.memory_delta_mb
    );

    assert!(
        result.highlight_time_ms < 1000.0,
        "JS medium file should highlight in <1000ms, took {:.2}ms",
        result.highlight_time_ms
    );
}

#[test]
fn perf_highlight_small_file_python() {
    let code = generate_python_file(100);
    let result = benchmark_oneshot_highlight("python", "catppuccin-frappe", &code);

    println!(
        "Python small file ({} lines): {:.2}ms, memory delta: {:.2}MB",
        result.file_lines, result.highlight_time_ms, result.memory_delta_mb
    );

    assert!(
        result.highlight_time_ms < 300.0,
        "Python small file should highlight in <300ms, took {:.2}ms",
        result.highlight_time_ms
    );
}

#[test]
fn perf_highlight_medium_file_python() {
    let code = generate_python_file(1000);
    let result = benchmark_oneshot_highlight("python", "catppuccin-frappe", &code);

    println!(
        "Python medium file ({} lines): {:.2}ms, memory delta: {:.2}MB",
        result.file_lines, result.highlight_time_ms, result.memory_delta_mb
    );

    assert!(
        result.highlight_time_ms < 300.0,
        "Python medium file should highlight in <300ms, took {:.2}ms",
        result.highlight_time_ms
    );
}

#[test]
fn perf_compare_themes() {
    let code = generate_rust_file(500);
    let themes = vec![
        "catppuccin-frappe",
        "catppuccin-mocha",
        "tokyo-night",
        "dracula",
        "kanagawa-wave",
    ];

    println!("\nTheme comparison (500 lines):");
    println!(
        "{:<20} {:>12} {:>15}",
        "Theme", "Time (ms)", "Output (bytes)"
    );
    println!("{}", "-".repeat(50));

    for theme in themes {
        let result = benchmark_oneshot_highlight("rust", theme, &code);
        println!(
            "{:<20} {:>12.2} {:>15}",
            theme, result.highlight_time_ms, result.output_size_bytes
        );
    }
}

#[test]
fn perf_compare_languages() {
    let lines = 500;
    let languages = vec![
        ("rust", generate_rust_file(lines)),
        ("javascript", generate_javascript_file(lines)),
        ("python", generate_python_file(lines)),
    ];

    println!("\nLanguage comparison ({} lines each):", lines);
    println!(
        "{:<15} {:>12} {:>15} {:>15}",
        "Language", "Time (ms)", "Output (bytes)", "Memory (MB)"
    );
    println!("{}", "-".repeat(60));

    for (lang, code) in languages {
        let result = benchmark_oneshot_highlight(lang, "catppuccin-frappe", &code);
        println!(
            "{:<15} {:>12.2} {:>15} {:>15.2}",
            lang, result.highlight_time_ms, result.output_size_bytes, result.memory_delta_mb
        );
    }
}

#[test]
fn perf_realistic_code() {
    // Test with actual complex code patterns
    let complex_rust = r#"
use std::collections::{HashMap, HashSet, BTreeMap};
use std::sync::{Arc, Mutex, RwLock};
use std::future::Future;
use std::pin::Pin;

/// A complex generic struct with multiple type parameters
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComplexDataStructure<K, V, const N: usize>
where
    K: std::fmt::Display + Send + Sync + 'static,
    V: std::fmt::Debug + Send + Sync + 'static,
{
    inner: Arc<RwLock<HashMap<K, Vec<V>>>>,
    cache: Arc<Mutex<BTreeMap<K, V>>>,
    config: Config<N>,
}

impl<K, V, const N: usize> ComplexDataStructure<K, V, N>
where
    K: std::fmt::Display + Send + Sync + 'static + std::cmp::Ord,
    V: std::fmt::Debug + Send + Sync + 'static + Clone,
{
    pub async fn new(config: Config<N>) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            inner: Arc::new(RwLock::new(HashMap::with_capacity(N))),
            cache: Arc::new(Mutex::new(BTreeMap::new())),
            config,
        })
    }

    pub async fn process_batch<T, F>(&self, items: Vec<T>, processor: F) -> Result<Vec<V>, String>
    where
        T: Send + 'static,
        F: Fn(T) -> Pin<Box<dyn Future<Output = Result<V, String>> + Send>> + Send + Sync + 'static,
    {
        let mut results = Vec::with_capacity(items.len());
        
        for item in items {
            match processor(item).await {
                Ok(v) => results.push(v),
                Err(e) => {
                    log::error!("Processing failed: {}", e);
                    return Err(e);
                }
            }
        }
        
        Ok(results)
    }

    pub fn blocking_operation(&self) -> impl Future<Output = Result<(), String>> + '_ {
        async move {
            let guard = self.inner.read().map_err(|e| e.to_string())?;
            
            for (key, values) in guard.iter() {
                if values.len() > self.config.max_items {
                    log::warn!("Too many items for key: {}", key);
                }
            }
            
            drop(guard);
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Config<const N: usize> {
    max_items: usize,
    timeout_ms: u64,
    enable_caching: bool,
}

impl<const N: usize> Config<N> {
    pub const fn default() -> Self {
        Self {
            max_items: N,
            timeout_ms: 1000,
            enable_caching: true,
        }
    }
}

// Macro example
macro_rules! complex_macro {
    ($name:ident, $type:ty, $default:expr) => {
        pub struct $name {
            value: $type,
        }
        
        impl $name {
            pub fn new() -> Self {
                Self { value: $default }
            }
        }
    };
}

complex_macro!(MyStruct1, i32, 42);
complex_macro!(MyStruct2, String, String::from("default"));

// Unsafe block example (for highlighting purposes only)
unsafe fn raw_pointer_operation(ptr: *const u8, len: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(len);
    
    for i in 0..len {
        result.push(*ptr.add(i));
    }
    
    result
}

// Attribute macros and derives
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(dead_code)]
#[warn(clippy::all)]
pub struct PlainOldData {
    pub x: i32,
    pub y: i32,
}

// Match with guards and complex patterns
pub fn complex_match(value: Option<Result<Vec<i32>, String>>) -> i32 {
    match value {
        Some(Ok(vec)) if vec.len() > 10 && vec[0] > 0 => vec.iter().sum(),
        Some(Ok(vec)) => vec.len() as i32,
        Some(Err(msg)) if msg.contains("error") => -1,
        Some(Err(_)) => -2,
        None => 0,
    }
}

// Closure with complex captures
pub fn closure_example() -> impl Fn(i32) -> i32 {
    let x = 10;
    let y = String::from("captured");
    let z: Box<dyn Fn(i32) -> i32> = Box::new(move |n| {
        println!("{}", y);
        x + n
    });
    
    z
}
"#;

    let result = benchmark_oneshot_highlight("rust", "catppuccin-frappe", complex_rust);

    println!(
        "\nRealistic complex code ({} lines): {:.2}ms, memory delta: {:.2}MB",
        complex_rust.lines().count(),
        result.highlight_time_ms,
        result.memory_delta_mb
    );

    assert!(
        result.highlight_time_ms < 200.0,
        "Complex code should highlight in <200ms, took {:.2}ms",
        result.highlight_time_ms
    );
}

#[test]
fn perf_throughput_multiple_updates() {
    // Test throughput with multiple rapid updates
    let code = generate_rust_file(200);
    let iterations = 50;

    let start = Instant::now();

    for _ in 0..iterations {
        let _result = benchmark_oneshot_highlight("rust", "catppuccin-frappe", &code);
    }

    let total_time = start.elapsed().as_secs_f64() * 1000.0;
    let avg_time = total_time / iterations as f64;
    let throughput = iterations as f64 / (total_time / 1000.0); // highlights per second

    println!(
        "\nThroughput test: {} iterations of {} lines",
        iterations,
        code.lines().count()
    );
    println!(
        "Total time: {:.2}ms, Average: {:.2}ms, Throughput: {:.1} highlights/sec",
        total_time, avg_time, throughput
    );

    // Conservative: should handle at least 10 highlights per second
    assert!(
        throughput > 10.0,
        "Throughput should be >10 highlights/sec, was {:.1}",
        throughput
    );
}
