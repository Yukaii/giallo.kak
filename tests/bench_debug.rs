use giallo_kak::config::Tuning;
use giallo_kak::highlight::{parse_plan, Plan};

fn generate_rust(lines: usize) -> String {
    let mut s = String::with_capacity(lines * 40);
    for i in 0..lines {
        match i % 4 {
            0 => s.push_str(&format!("fn function_{i}() -> i32 {{\n")),
            1 => s.push_str(&format!("    let value_{i} = {i};\n")),
            2 => s.push_str(&format!("    println!(\"value={{}}\", value_{i});\n")),
            _ => s.push_str("}\n"),
        }
    }
    s
}

#[test]
fn debug_plan() {
    let lines = 200usize;
    let base = generate_rust(lines);
    let n: usize = 4 * (lines / 8) + 1;
    println!("mid={n}");
    for (i, l) in base.lines().enumerate() {
        if i >= 99 && i <= 103 {
            println!("{i}: [{l}]");
        }
    }
    let mut v: Vec<String> = base.lines().map(String::from).collect();
    println!("before: [{}]", v[n]);
    v[n] = format!("    let value_{n} = {};", n + 100_000);
    println!("after:  [{}]", v[n]);
    let ea = v.join("\n") + "\n";
    println!("equal: {}", base == ea);
    match parse_plan(&Tuning::default(), &base, &ea) {
        Plan::Window(sp) => println!("WINDOW {}..{}", sp.window_start, sp.window_end),
        Plan::Full => println!("FULL"),
        Plan::NoChange => println!("NOCHANGE"),
    }
}
