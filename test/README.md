# Seen Language Test Suite

This directory contains all tests for the Seen programming language compiler and runtime.

## Structure

### `/unit/`
Unit tests for individual compiler components:
- `compiler/` - Core compiler functionality tests
- `optimization/` - Optimization pipeline tests (E-graph, ML, Superopt, PGO, Memory, Architecture)
- `language/` - Language feature and syntax tests

### `/integration/`
Integration tests that verify end-to-end functionality:
- `syntax/` - Comprehensive syntax and language feature testing
- `memory/` - Memory model and safety testing
- `compilation/` - Full compilation pipeline testing

### `/examples/`
Example programs demonstrating language features:
- `basic/` - Basic language constructs and Hello World examples
- `advanced/` - Advanced features like memory model, generics, etc.
- `performance/` - Performance demonstration programs

### `/fixtures/`
Test data and fixture files used by various tests.

## Running Tests

From the project root:

```bash
# Run all tests
./target-wsl/debug/seen test

# Run specific test categories
./target-wsl/debug/seen test --unit
./target-wsl/debug/seen test --integration

# Run with verbose output
./target-wsl/debug/seen test --verbose
```

From within compiler_seen:

```bash
# Use the comprehensive test runner
../target-wsl/debug/seen run run_tests.seen
```

## Test Organization Principles

- **Consolidation**: All tests are centralized in this single `/test` directory
- **Logical Grouping**: Tests are organized by type and scope
- **Reusability**: Example programs serve as both tests and documentation
- **Maintainability**: Clear structure makes adding new tests straightforward