---
name: code-review
description: Guidelines for reviewing code changes and providing constructive feedback
---

# Code Review Skill

## Overview

When reviewing code, systematically evaluate the changes across multiple dimensions. Be thorough but constructive — your goal is to improve the code while respecting the author's intent.

## Review Checklist

### Correctness
- [ ] Does the code do what it claims?
- [ ] Are there edge cases or boundary conditions that are not handled?
- [ ] Are there potential bugs (off-by-one errors, null checks, etc.)?
- [ ] Is the logic sound and free of contradictions?

### Security
- [ ] Is user input properly validated and sanitized?
- [ ] Are there potential injection vulnerabilities?
- [ ] Are secrets or sensitive data handled appropriately?
- [ ] Are permissions and access controls correctly implemented?

### Performance
- [ ] Are there unnecessary repeated computations?
- [ ] Could database queries be optimized?
- [ ] Are there memory leaks or unbounded growth?
- [ ] Is the time complexity appropriate for the use case?

### Style & Readability
- [ ] Is the code easy to understand?
- [ ] Are names descriptive and consistent?
- [ ] Is the code organized logically?
- [ ] Are comments used appropriately (not cluttering)?

### Testing
- [ ] Are there adequate tests for the new behavior?
- [ ] Do existing tests still pass?
- [ ] Are edge cases covered?
- [ ] Are test cases meaningful and maintainable?

## Feedback Guidelines

1. **Distinguish blocking from non-blocking issues:**
   - Blocking: Must be fixed before merge (bugs, security issues, broken tests)
   - Non-blocking: Suggestions for improvement (style, minor optimizations)

2. **Be specific:**
   - Point to exact lines or functions
   - Explain the concern clearly
   - Provide a concrete suggestion when possible

3. **Be kind:**
   - Acknowledge good work
   - Focus on the code, not the person
   - Suggest alternatives, don't dictate

## Example Feedback

```
## Review: PR #42 — Add user authentication

### ✅ What's Good
- Clean separation of concerns in auth service
- Good use of bcrypt for password hashing
- Comprehensive test coverage

### ⚠️ Non-blocking Suggestions
- Consider extracting magic strings into constants
- The refresh token logic could use a comment explaining the strategy

### 🔴 Blocking Issues
- Line 47: SQL injection vulnerability — user input not sanitized
- Missing test coverage for expired token handling

### Recommendation
Please address the blocking issue before merging. Happy to re-review after the fix!
```
