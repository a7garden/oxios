// Fixture for context-file test
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

fn main() {
    let message = greet("Oxios");
    println!("{}", message);
}
