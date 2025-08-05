//! Operator precedence handling

/// Operator precedence levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precedence {
    None = 0,
    Assignment = 1,  // =
    Or = 2,          // ||
    And = 3,         // &&
    Equality = 4,    // == !=
    Comparison = 5,  // < > <= >=
    Term = 6,        // + -
    Factor = 7,      // * /
    Unary = 8,       // ! -
    Call = 9,        // . ()
    Primary = 10,
}