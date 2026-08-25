//! Direct benchmark of the highlight path, isolated from process spawn and
//! registry-load noise that the performance suite measures.
//!
//! Not part of normal test runs; execute explicitly with:
//!
//!   cargo test --release --test highlight_bench -- --ignored --nocapture
//!
//! Measures per file size:
//! - full parse + command build (cold-cache / fallback cost)
//! - windowed incremental update cost through the real splice+delta path,
//!   mutating one persistent cache exactly like production
//! - identical-update cost (plan only)

use std::hint::black_box;
use std::time::Instant;

use giallo::HighlightOptions;
use giallo_kak::config::Tuning;
use giallo_kak::highlight::{
    build_delta_commands, escalate_to_full, line_slice, parse_plan, HighlightCache, Plan,
};
use giallo_kak::highlighting::{build_line_tokens, build_kakoune_commands};

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

/// Edit the `let value_N` line at index `n` by bumping its number literal.
fn edit_line(text: &str, n: usize, add: usize) -> String {
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    lines[n] = format!("    let value_{n} = {};", n + add);
    lines.join("\n") + "\n"
}

fn time_iters<F: FnMut()>(mut f: F, iters: usize) -> f64 {
    f(); // warmup
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    start.elapsed().as_secs_f64() / iters as f64 * 1000.0
}

#[test]
#[ignore = "benchmark; run with --ignored"]
fn bench_highlight_path() {
    let registry = giallo::Registry::builtin().expect("registry loads");
    let tuning = Tuning::default();

    println!("\n{:=<76}", "=");
    println!(
        "{:>6} {:>14} {:>14} {:>14}",
        "lines", "full ms", "window ms", "identical us"
    );
    println!("{:-<76}", "-");

    for lines in [200usize, 1000, 5000] {
        let base = generate_rust(lines);
        let options =
            HighlightOptions::new("rust", giallo::ThemeVariant::Single("catppuccin-frappe"));

        let full_ms = time_iters(
            || {
                if let Ok(h) = registry.highlight(&base, &options) {
                    black_box(build_kakoune_commands(&h));
                }
            },
            if lines >= 5000 { 5 } else { 20 },
        );

        // Persistent cache mutated in place, ping-ponging an edit between two
        // variants of a middle `let value_N` line (indices % 4 == 1).
        let mid = 4 * (lines / 8) + 1;
        let edit_a = edit_line(&base, mid, 100_000);
        let edit_b = edit_line(&base, mid, 200_000);

        let mut cache = HighlightCache::default();
        cache.reset_for("rust", "catppuccin-frappe");
        {
            let h = registry.highlight(&base, &options).expect("parses");
            let mut faces = Vec::new();
            cache.line_tokens = build_line_tokens(&h, &mut cache.allocator, &mut faces);
            cache.text.push_str(&base);
        }

        let window_ms = time_iters(
            || {
                let next = if cache.text != edit_a {
                    // base or already edited to b: switch to a
                    edit_a.clone()
                } else {
                    edit_b.clone()
                };
                black_box(&next);
                let plan = parse_plan(&tuning, &cache.text.clone(), &next);
                match escalate_to_full(plan, 0, tuning.full_refresh_interval) {
                    Plan::Window(splice) => {
                        let slice = line_slice(&next, splice.window_start, splice.window_end);
                        let Ok(h) = registry.highlight(&slice, &options) else {
                            panic!("window slice failed to parse");
                        };
                        let mut old_tokens = std::mem::take(&mut cache.line_tokens);
                        let mut new_faces = Vec::new();
                        let window_tokens =
                            build_line_tokens(&h, &mut cache.allocator, &mut new_faces);
                        let skip = splice.dirty_start - splice.window_start;
                        let mut replacement: Vec<_> =
                            window_tokens.into_iter().skip(skip).collect();
                        let avail = next.matches('\n').count() + 1 - splice.dirty_start;
                        replacement.truncate(avail);
                        let we_old = (splice.dirty_end_old + tuning.margin_lines)
                            .min(old_tokens.len().saturating_sub(1));
                        let mut tail =
                            old_tokens.split_off((we_old + 1).min(old_tokens.len()));
                        let pre_splice = old_tokens.clone();
                        cache.line_tokens = old_tokens;
                        cache.line_tokens.truncate(splice.dirty_start);
                        cache.line_tokens.extend(replacement);
                        cache.line_tokens.append(&mut tail);
                        black_box(build_delta_commands(
                            &tuning,
                            &pre_splice,
                            &cache.line_tokens,
                            &new_faces,
                            &mut cache.ensured_chunks,
                        ));
                    }
                    other => panic!(
                    "expected windowed plan, got {:?} (iter text len {} -> {})",
                    other,
                    cache.text.len(),
                    next.len()
                ),
                }
                cache.text.clear();
                cache.text.push_str(&next);
            },
            50,
        );

        let identical_us = time_iters(
            || {
                black_box(parse_plan(&tuning, &cache.text.clone(), &cache.text));
            },
            200,
        ) * 1000.0;

        println!("{:>6} {:>14.2} {:>14.2} {:>14.1}", lines, full_ms, window_ms, identical_us);
    }

    println!("{:=<76}\n", "=");
}
