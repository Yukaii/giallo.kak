//! Performance test fixture generator
//!
//! This binary generates synthetic test files of various sizes for performance testing.
//! Run with: cargo run --bin generate_perf_fixtures

use std::fs;
use std::path::PathBuf;

fn generate_rust_file(lines: usize) -> String {
    let mut code = String::with_capacity(lines * 50);

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

        let fn_name = format!("function_{}_{:04}", i, i * 1234 % 10000);
        code.push_str(&format!("fn {}() -> Result<(), String> {{\n", fn_name));

        code.push_str(&format!("    let x_{} = {};\n", i, i * 42));
        code.push_str(&format!("    let y_{} = \"string_{}\";\n", i, i));
        code.push_str(&format!(
            "    let z_{} = vec![{}, {}, {}];\n",
            i,
            i,
            i + 1,
            i + 2
        ));

        code.push_str(&format!("    if x_{} > {} {{\n", i, i * 10));
        code.push_str(&format!("        println!(\"Value: {{}}\", y_{});\n", i));
        code.push_str("    } else {\n");
        code.push_str(&format!(
            "        return Err(\"Error {}\".to_string());\n",
            i
        ));
        code.push_str("    }\n");

        code.push_str(&format!("    for i in 0..{} {{\n", i % 10 + 1));
        code.push_str(&format!("        if i == {} {{\n", i % 5));
        code.push_str("            continue;\n");
        code.push_str("        }\n");
        code.push_str("    }\n");

        code.push_str(&format!("    match x_{} {{\n", i));
        code.push_str(&format!("        {} => println!(\"First\"),\n", i * 42));
        code.push_str(&format!(
            "        {} => println!(\"Second\"),\n",
            i * 42 + 1
        ));
        code.push_str("        _ => println!(\"Other\"),\n");
        code.push_str("    }\n");

        code.push_str("    Ok(())\n");
        code.push_str("}\n\n");
    }

    code.push_str("#[derive(Debug, Clone)]\n");
    code.push_str("struct TestStruct {\n");
    code.push_str("    field1: i32,\n");
    code.push_str("    field2: String,\n");
    code.push_str("    field3: Vec<u8>,\n");
    code.push_str("}\n\n");

    code.push_str("impl TestStruct {\n");
    code.push_str("    fn new(f1: i32, f2: String) -> Self {\n");
    code.push_str("        Self {\n");
    code.push_str("            field1: f1,\n");
    code.push_str("            field2: f2,\n");
    code.push_str("            field3: Vec::new(),\n");
    code.push_str("        }\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");

    while code.lines().count() < lines {
        code.push_str("// padding line\n");
    }

    code
}

fn generate_javascript_file(lines: usize) -> String {
    let mut code = String::with_capacity(lines * 40);

    code.push_str("// Generated JavaScript test file\n");
    code.push_str(&format!("// Lines: {}\n\n", lines));

    let functions_needed = lines / 8 + 1;

    for i in 0..functions_needed {
        if code.lines().count() >= lines {
            break;
        }

        let fn_name = format!("function_{:04}", i);
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

fn generate_python_file(lines: usize) -> String {
    let mut code = String::with_capacity(lines * 35);

    code.push_str("# Generated Python test file\n");
    code.push_str(&format!("# Lines: {}\n\n", lines));

    let functions_needed = lines / 7 + 1;

    for i in 0..functions_needed {
        if code.lines().count() >= lines {
            break;
        }

        let fn_name = format!("function_{:04}", i);
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

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = manifest_dir.join("tests").join("fixtures");

    fs::create_dir_all(&fixtures_dir).expect("failed to create fixtures dir");

    type GeneratorFn = fn(usize) -> String;

    let files_to_generate: Vec<(&str, usize, GeneratorFn)> = vec![
        ("perf_small.rs", 100usize, generate_rust_file as GeneratorFn),
        (
            "perf_medium.rs",
            1000usize,
            generate_rust_file as GeneratorFn,
        ),
        (
            "perf_large.rs",
            10000usize,
            generate_rust_file as GeneratorFn,
        ),
        (
            "perf_small.js",
            100usize,
            generate_javascript_file as GeneratorFn,
        ),
        (
            "perf_medium.js",
            1000usize,
            generate_javascript_file as GeneratorFn,
        ),
        (
            "perf_small.py",
            100usize,
            generate_python_file as GeneratorFn,
        ),
        (
            "perf_medium.py",
            1000usize,
            generate_python_file as GeneratorFn,
        ),
    ];

    println!("Generating performance test fixtures...\n");

    for (filename, lines, generator) in files_to_generate {
        let content = generator(lines);
        let filepath = fixtures_dir.join(filename);
        fs::write(&filepath, content).expect(&format!("failed to write {}", filename));
        let file_size = fs::metadata(&filepath).unwrap().len();
        println!(
            "Generated {} ({} lines, {} bytes)",
            filename, lines, file_size
        );
    }

    println!("\nDone! Fixtures written to: {}", fixtures_dir.display());
}
