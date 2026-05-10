---
name: refactor
description: Guidelines and patterns for refactoring code safely and effectively
---

# Refactoring Skill

## Overview

Refactoring improves code structure without changing its behavior. Do it incrementally, with tests, and for a clear reason. Never refactor while also adding features — each should be a separate change.

## When to Refactor

### Signs Code Needs Refactoring
- The same logic is duplicated in multiple places
- A function is doing too many things (low cohesion)
- Classes/modules are too tightly coupled
- Names are confusing or misleading
- Dead code exists (unused functions, imports, etc.)
- Tests are hard to write or maintain

### When NOT to Refactor
- When there's no test coverage
- In the middle of a critical deadline
- If the "current" code is working well enough
- Instead of understanding why it's written that way first

## The Refactoring Process

### 1. Understand Before Changing
- Read the existing code thoroughly
- Talk to the original author if possible
- Run existing tests to establish a baseline
- Identify all callers and side effects

### 2. Make a Plan
- Break large refactors into small steps
- Each step should be independently testable
- Keep refactoring and behavior changes separate

### 3. Do It Incrementally
- Change one thing at a time
- Run tests after each small change
- Commit between steps

### 4. Verify
- All existing tests still pass
- New tests cover the refactored code
- Performance hasn't degraded
- No new linting warnings

## Common Refactoring Patterns

### Extract Method
**Before:**
```rust
fn process_order(order: &Order) {
    // 50 lines of validation, calculation, and saving
}
```

**After:**
```rust
fn process_order(order: &Order) -> Result<()> {
    validate_order(order)?;
    let total = calculate_total(order)?;
    save_order(order, total)?;
    send_confirmation(order)?;
    Ok(())
}
```

### Replace Conditional with Polymorphism
**Before:**
```rust
fn calculate_area(shape: &Shape) -> f64 {
    match shape {
        Shape::Circle(r) => std::f64::consts::PI * r * r,
        Shape::Rectangle(w, h) => w * h,
        Shape::Triangle(b, h) => 0.5 * b * h,
    }
}
```

**After:**
```rust
trait Area {
    fn area(&self) -> f64;
}

struct Circle { radius: f64 }
impl Area for Circle {
    fn area(&self) -> f64 { std::f64::consts::PI * self.radius * self.radius }
}
```

### Introduce Parameter Object
**Before:**
```rust
fn create_user(name: String, email: String, age: u32, city: String, country: String) {
    // ...
}
```

**After:**
```rust
struct UserProfile {
    name: String,
    email: String,
    age: u32,
    location: Location,
}

struct Location { city: String, country: String }
```

### Rename
When renaming, update all references:
- Variables and functions
- Types and traits
- Files (if the type name is the filename)
- Tests (if relevant)

## Safety Checklist

- [ ] All existing tests pass
- [ ] New behavior has test coverage
- [ ] No temporary debugging code left behind
- [ ] Commit message explains WHY, not just WHAT
- [ ] No merge conflicts introduced
- [ ] Documentation updated if public API changed
