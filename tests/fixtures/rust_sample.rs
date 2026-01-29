// Test fixture for Rust syntax highlighting
// Should test: strings, keywords, comments

fn main() {
    // String literals
    let greeting = "Hello, world!";
    let multiline = "This is a \
                     multiline string";
    let raw_string = r#"Raw string with "quotes""#;

    // Keywords
    if true {
        println!("{}", greeting);
    } else {
        return;
    }

    // More keywords
    for i in 0..10 {
        match i {
            0 => continue,
            5 => break,
            _ => {}
        }
    }

    // Structs and impl
    struct Point {
        x: i32,
        y: i32,
    }

    impl Point {
        fn new(x: i32, y: i32) -> Self {
            Self { x, y }
        }
    }

    // Control flow keywords
    let result = if let Some(value) = Some(42) {
        value
    } else {
        0
    };

    // Async/await keywords
    async fn fetch_data() -> Result<String, ()> {
        Ok(String::from("data"))
    }
}
