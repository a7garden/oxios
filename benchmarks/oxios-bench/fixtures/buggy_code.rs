// Fixture for code-fix regression test
// Bug: this function should return the sum, not the difference
fn add(a: i32, b: i32) -> i32 {
    a - b  // BUG: should be a + b
}

fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_multiply() {
        assert_eq!(multiply(2, 3), 6);
    }
}
