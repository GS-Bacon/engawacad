use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, HashSet};
use thiserror::Error;
use ts_rs::TS;

/// A variable definition with a name and an expression string.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct Variable {
    pub name: String,
    pub expr: String,
}

/// Evaluation error for variable expressions.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum EvalError {
    #[error("circular dependency detected: {cycle:?}")]
    CircularDependency { cycle: Vec<String> },

    #[error("undefined variable: {name}")]
    UndefinedVariable { name: String },

    #[error("empty expression for variable: {name}")]
    EmptyExpression { name: String },

    #[error("parse error in {name}: {reason}")]
    Parse { name: String, reason: String },

    /// 同一スコープ内に同名 Variable が複数存在する。
    /// ADR-015 §2 で許容されるのは Sketch ⇔ Document 間の shadowing のみであり、
    /// 同一スコープ内の duplicate は silent drop を防ぐため明示的に reject する。
    #[error("duplicate variable name in {scope} scope: {name}")]
    DuplicateVariable { scope: String, name: String },

    /// 公開 API `evaluate_scope` 経由で不正な variable name が渡された。
    /// ADR-015 §1 で定義される有効な variable name は:
    /// - 空でない
    /// - ASCII alphanumeric + `_` または `-` のみで構成される
    #[error("invalid variable name in {scope} scope: {name:?} ({reason})")]
    InvalidVariableName {
        scope: String,
        name: String,
        reason: &'static str,
    },
}

/// Evaluate variables with 2-stage lexical scope:
/// - Phase 1: Document variables are evaluated in isolation (doc-only scope)
/// - Phase 2: Sketch variables are evaluated with access to Document results
/// - Final: Sketch results shadow Document results where names overlap
///
/// Returns an IndexMap preserving insertion order for determinism.
pub fn evaluate_scope(
    doc_vars: &[Variable],
    sketch_vars: &[Variable],
) -> Result<IndexMap<String, f64>, EvalError> {
    // R6: 不正な variable name (空/不正文字) を事前検出
    check_valid_names(doc_vars, "document")?;
    check_valid_names(sketch_vars, "sketch")?;
    // R4: 同一スコープ内重複名を検出して reject
    check_unique_names(doc_vars, "document")?;
    check_unique_names(sketch_vars, "sketch")?;

    // Phase 1: Evaluate document variables in isolation
    let doc_result = evaluate_doc_scope(doc_vars)?;

    // Phase 2: Evaluate sketch variables with access to document results
    let sketch_result = evaluate_sketch_scope(&doc_result, sketch_vars)?;

    // Merge: sketch shadows doc
    let mut combined: IndexMap<String, f64> = doc_result;
    for (k, v) in sketch_result {
        combined.insert(k, v);
    }
    Ok(combined)
}

/// Check that all variable names are valid (ADR-015 §1).
/// Valid names: non-empty, ASCII alphanumeric + `_` or `-`.
fn check_valid_names(vars: &[Variable], scope: &str) -> Result<(), EvalError> {
    for v in vars {
        if v.name.is_empty() {
            return Err(EvalError::InvalidVariableName {
                scope: scope.to_string(),
                name: v.name.clone(),
                reason: "variable name must not be empty",
            });
        }
        if !v
            .name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(EvalError::InvalidVariableName {
                scope: scope.to_string(),
                name: v.name.clone(),
                reason: "variable name contains invalid characters",
            });
        }
    }
    Ok(())
}

/// Check that all variable names in a scope are unique.
/// Returns `DuplicateVariable` error if any name appears more than once.
fn check_unique_names(vars: &[Variable], scope: &str) -> Result<(), EvalError> {
    let mut seen = std::collections::HashSet::new();
    for v in vars {
        if !seen.insert(v.name.as_str()) {
            return Err(EvalError::DuplicateVariable {
                scope: scope.to_string(),
                name: v.name.clone(),
            });
        }
    }
    Ok(())
}

/// Phase 1: Evaluate document variables in doc-only scope.
/// Document variables cannot reference sketch variables.
fn evaluate_doc_scope(doc_vars: &[Variable]) -> Result<IndexMap<String, f64>, EvalError> {
    if doc_vars.is_empty() {
        return Ok(IndexMap::new());
    }

    let doc_set: HashSet<&str> = doc_vars.iter().map(|v| v.name.as_str()).collect();

    // Build dependency graph within doc scope only
    let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
    for var in doc_vars {
        let deps = extract_var_refs_doc_only(&var.expr, &doc_set);
        dependencies.insert(var.name.clone(), deps);
    }

    evaluate_vars_topological(doc_vars, &dependencies, &[], &IndexMap::new())
}

/// Phase 2: Evaluate sketch variables with access to doc_result.
/// Sketch variables can reference:
/// - Other sketch variables
/// - Document variables (via doc_result, read-only)
fn evaluate_sketch_scope(
    doc_result: &IndexMap<String, f64>,
    sketch_vars: &[Variable],
) -> Result<IndexMap<String, f64>, EvalError> {
    if sketch_vars.is_empty() {
        return Ok(IndexMap::new());
    }

    let sketch_set: HashSet<&str> = sketch_vars.iter().map(|v| v.name.as_str()).collect();
    let doc_set: HashSet<&str> = doc_result.keys().map(|k| k.as_str()).collect();

    // Build dependency graph within sketch scope (sketch -> sketch only)
    // Outer scope (doc) variables are pre-computed and don't affect topological order
    let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
    for var in sketch_vars {
        let all_deps = extract_var_refs_sketch(&var.expr, &sketch_set, &doc_set);
        // Filter out dependencies from outer scope (they're already computed)
        let local_deps: Vec<String> = all_deps
            .into_iter()
            .filter(|d| sketch_set.contains(d.as_str()))
            .collect();
        dependencies.insert(var.name.clone(), local_deps);
    }

    evaluate_vars_topological(sketch_vars, &dependencies, &[], doc_result)
}

/// Common topological evaluation for a set of variables.
/// `available_vars` contains the pre-computed values from outer scopes (e.g., doc_result for sketch evaluation).
fn evaluate_vars_topological(
    vars: &[Variable],
    dependencies: &HashMap<String, Vec<String>>,
    available_vars: &[Variable],
    outer_scope: &IndexMap<String, f64>,
) -> Result<IndexMap<String, f64>, EvalError> {
    // Build reverse graph: var -> list of variables that depend on it
    let mut reverse_graph: HashMap<String, Vec<String>> = HashMap::new();
    for (var, deps) in dependencies {
        for dep in deps {
            if dep != var {
                reverse_graph
                    .entry(dep.clone())
                    .or_default()
                    .push(var.clone());
            }
        }
    }

    // Kahn's algorithm: compute indegree
    let mut indegree: HashMap<String, usize> = HashMap::new();
    for (var, deps) in dependencies {
        indegree.insert(var.clone(), deps.len());
    }
    for var in vars {
        indegree.entry(var.name.clone()).or_insert(0);
    }

    // Use max-heap for lexicographic tiebreak, then reverse
    let mut heap: BinaryHeap<std::cmp::Reverse<String>> = BinaryHeap::new();
    for var in vars {
        if *indegree.get(&var.name).unwrap_or(&0) == 0 {
            heap.push(std::cmp::Reverse(var.name.clone()));
        }
    }

    let mut result = IndexMap::new();
    while let Some(std::cmp::Reverse(current)) = heap.pop() {
        let var = vars.iter().find(|v| v.name == current).unwrap();
        let value = eval_expr_with_outer(
            &var.expr,
            &result,
            outer_scope,
            vars,
            available_vars,
            &current,
            &mut HashSet::new(),
        )?;
        result.insert(current.clone(), value);

        if let Some(deps) = reverse_graph.get(&current) {
            for dep in deps {
                if let Some(deg) = indegree.get_mut(dep) {
                    *deg -= 1;
                    if *deg == 0 {
                        heap.push(std::cmp::Reverse(dep.clone()));
                    }
                }
            }
        }
    }

    // Check for cycle
    if result.len() != vars.len() {
        let mut sorted_names: Vec<String> = vars.iter().map(|v| v.name.clone()).collect();
        sorted_names.sort();
        detect_cycle(dependencies, &sorted_names)?;
    }

    Ok(result)
}

/// Extract variable references from expression for doc-only scope.
/// Only considers variables in doc_set.
fn extract_var_refs_doc_only(expr: &str, doc_set: &HashSet<&str>) -> Vec<String> {
    let mut refs = Vec::new();
    let mut chars = expr.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut name = String::new();
            while let Some(&ch) = chars.peek() {
                if ch == '}' {
                    chars.next(); // consume '}'
                    if !name.is_empty() {
                        let name_str = name.as_str();
                        if doc_set.contains(name_str) {
                            refs.push(name);
                        }
                    }
                    break;
                }
                name.push(chars.next().unwrap());
            }
        }
    }
    refs
}

/// Extract variable references from expression for sketch scope.
/// Considers both sketch_set and doc_set (sketch can reference doc).
fn extract_var_refs_sketch(
    expr: &str,
    sketch_set: &HashSet<&str>,
    doc_set: &HashSet<&str>,
) -> Vec<String> {
    let mut refs = Vec::new();
    let mut chars = expr.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut name = String::new();
            while let Some(&ch) = chars.peek() {
                if ch == '}' {
                    chars.next(); // consume '}'
                    if !name.is_empty() {
                        let name_str = name.as_str();
                        // In sketch scope: check sketch first (self-ref), then doc
                        if sketch_set.contains(name_str) || doc_set.contains(name_str) {
                            refs.push(name);
                        }
                    }
                    break;
                }
                name.push(chars.next().unwrap());
            }
        }
    }
    refs
}

/// Evaluate a single expression with outer scope support.
/// `outer_scope` contains pre-computed values from enclosing scopes (e.g., doc_result).
fn eval_expr_with_outer(
    expr: &str,
    computed: &IndexMap<String, f64>,
    outer_scope: &IndexMap<String, f64>,
    local_vars: &[Variable],
    available_vars: &[Variable],
    self_name: &str,
    eval_stack: &mut HashSet<String>,
) -> Result<f64, EvalError> {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return Err(EvalError::EmptyExpression {
            name: self_name.to_string(),
        });
    }

    let mut parser = ParserWithOuter::new(
        trimmed,
        computed,
        outer_scope,
        local_vars,
        available_vars,
        self_name,
        eval_stack,
    );
    let value = parser.parse_expr()?;

    // Check for trailing characters
    parser.skip_whitespace();
    if parser.peek_char().is_some() {
        return Err(EvalError::Parse {
            name: self_name.to_string(),
            reason: "unexpected trailing characters in expression".to_string(),
        });
    }

    Ok(value)
}

/// Detect and return a cycle in the dependency graph using DFS.
/// Takes ordered var_names for deterministic cycle detection.
fn detect_cycle(
    dependencies: &HashMap<String, Vec<String>>,
    var_names: &[String],
) -> Result<(), EvalError> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut rec_stack: HashSet<String> = HashSet::new();
    let mut path: Vec<String> = Vec::new();

    for var in var_names {
        if !visited.contains(var)
            && dfs_detect_cycle(var, dependencies, &mut visited, &mut rec_stack, &mut path)?
        {
            return Err(EvalError::CircularDependency { cycle: path });
        }
    }

    Ok(())
}

fn dfs_detect_cycle(
    var: &str,
    dependencies: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    rec_stack: &mut HashSet<String>,
    path: &mut Vec<String>,
) -> Result<bool, EvalError> {
    visited.insert(var.to_string());
    rec_stack.insert(var.to_string());
    path.push(var.to_string());

    if let Some(deps) = dependencies.get(var) {
        for dep in deps {
            if !visited.contains(dep) {
                if dfs_detect_cycle(dep, dependencies, visited, rec_stack, path)? {
                    return Ok(true);
                }
            } else if rec_stack.contains(dep) {
                // Found cycle: complete the cycle path
                let cycle_start = path.iter().position(|v| v == dep).unwrap();
                let mut cycle = path[cycle_start..].to_vec();
                cycle.push(dep.clone()); // close the cycle
                return Err(EvalError::CircularDependency { cycle });
            }
        }
    }

    rec_stack.remove(var);
    path.pop();
    Ok(false)
}

/// Recursive-descent parser for arithmetic expressions with outer scope support.
struct ParserWithOuter<'a> {
    input: &'a str,
    pos: usize,
    computed: &'a IndexMap<String, f64>,
    outer_scope: &'a IndexMap<String, f64>,
    local_vars: &'a [Variable],
    available_vars: &'a [Variable],
    self_name: &'a str,
    eval_stack: &'a mut HashSet<String>,
}

impl<'a> ParserWithOuter<'a> {
    fn new(
        input: &'a str,
        computed: &'a IndexMap<String, f64>,
        outer_scope: &'a IndexMap<String, f64>,
        local_vars: &'a [Variable],
        available_vars: &'a [Variable],
        self_name: &'a str,
        eval_stack: &'a mut HashSet<String>,
    ) -> Self {
        Self {
            input,
            pos: 0,
            computed,
            outer_scope,
            local_vars,
            available_vars,
            self_name,
            eval_stack,
        }
    }

    fn parse_expr(&mut self) -> Result<f64, EvalError> {
        let mut left = self.parse_term()?;

        loop {
            self.skip_whitespace();
            match self.peek_char() {
                Some('+') => {
                    self.consume_char();
                    let right = self.parse_term()?;
                    left += right;
                }
                Some('-') => {
                    self.consume_char();
                    let right = self.parse_term()?;
                    left -= right;
                }
                _ => break,
            }
        }

        Ok(left)
    }

    fn parse_term(&mut self) -> Result<f64, EvalError> {
        let mut left = self.parse_factor()?;

        loop {
            self.skip_whitespace();
            match self.peek_char() {
                Some('*') => {
                    self.consume_char();
                    let right = self.parse_factor()?;
                    left *= right;
                }
                Some('/') => {
                    self.consume_char();
                    let right = self.parse_factor()?;
                    left /= right;
                }
                _ => break,
            }
        }

        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<f64, EvalError> {
        self.skip_whitespace();
        let is_neg = match self.peek_char() {
            Some('-') => {
                self.consume_char();
                true
            }
            _ => false,
        };
        let mut value = self.parse_primary()?;
        if is_neg {
            value = -value;
        }
        Ok(value)
    }

    fn parse_primary(&mut self) -> Result<f64, EvalError> {
        self.skip_whitespace();

        match self.peek_char() {
            Some('(') => {
                self.consume_char();
                let value = self.parse_expr()?;
                self.skip_whitespace();
                if self.peek_char() != Some(')') {
                    return Err(EvalError::Parse {
                        name: self.self_name.to_string(),
                        reason: "expected ')'".to_string(),
                    });
                }
                self.consume_char();
                Ok(value)
            }
            Some('$') => self.parse_var_ref(),
            Some(c) if c.is_ascii_digit() || c == '.' => self.parse_number(),
            Some(c) => Err(EvalError::Parse {
                name: self.self_name.to_string(),
                reason: format!("unexpected character: {}", c),
            }),
            None => Err(EvalError::Parse {
                name: self.self_name.to_string(),
                reason: "unexpected end of input".to_string(),
            }),
        }
    }

    fn parse_var_ref(&mut self) -> Result<f64, EvalError> {
        if self.peek_char() != Some('$') {
            return Err(EvalError::Parse {
                name: self.self_name.to_string(),
                reason: "expected '$'".to_string(),
            });
        }
        self.consume_char();

        if self.peek_char() != Some('{') {
            return Err(EvalError::Parse {
                name: self.self_name.to_string(),
                reason: "expected '{' after '$'".to_string(),
            });
        }
        self.consume_char();

        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c == '}' {
                let name = &self.input[start..self.pos];
                self.consume_char(); // consume '}'
                return self.lookup_var(name);
            }
            self.consume_char();
        }

        Err(EvalError::Parse {
            name: self.self_name.to_string(),
            reason: "unterminated variable reference".to_string(),
        })
    }

    fn lookup_var(&self, name: &str) -> Result<f64, EvalError> {
        // 1. Check local scope (already computed in this phase)
        if let Some(&value) = self.computed.get(name) {
            return Ok(value);
        }

        // 2. Check outer scope (pre-computed from enclosing scope, e.g., doc_result)
        if let Some(&value) = self.outer_scope.get(name) {
            return Ok(value);
        }

        // 3. Check for circular reference
        if self.eval_stack.contains(name) {
            let cycle = vec![name.to_string(), name.to_string()];
            return Err(EvalError::CircularDependency { cycle });
        }

        // 4. Find variable definition and evaluate on-the-fly
        // Search local scope first, then available vars
        for var in self.local_vars.iter().chain(self.available_vars.iter()) {
            if var.name == name {
                let mut new_stack = self.eval_stack.clone();
                new_stack.insert(name.to_string());

                let mut parser = ParserWithOuter::new(
                    &var.expr,
                    self.computed,
                    self.outer_scope,
                    self.local_vars,
                    self.available_vars,
                    &var.name,
                    &mut new_stack,
                );
                return parser.parse_expr();
            }
        }

        // Truly undefined
        Err(EvalError::UndefinedVariable {
            name: name.to_string(),
        })
    }

    fn parse_number(&mut self) -> Result<f64, EvalError> {
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() || c == '.' {
                self.consume_char();
            } else {
                break;
            }
        }
        let s = &self.input[start..self.pos];
        s.parse::<f64>().map_err(|_| EvalError::Parse {
            name: self.self_name.to_string(),
            reason: format!("invalid number: {}", s),
        })
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn consume_char(&mut self) {
        if let Some(c) = self.input[self.pos..].chars().next() {
            self.pos += c.len_utf8();
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_ascii_whitespace() {
                self.consume_char();
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t01_deterministic_evaluation() {
        let doc_vars = vec![
            Variable {
                name: "a".to_string(),
                expr: "10".to_string(),
            },
            Variable {
                name: "b".to_string(),
                expr: "${a} * 2".to_string(),
            },
        ];

        let sketch_vars: Vec<Variable> = vec![];

        let result1 = evaluate_scope(&doc_vars, &sketch_vars).unwrap();
        let result2 = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

        assert_eq!(result1, result2);
        assert_eq!(result1.len(), 2);
        assert_eq!(result1.get("a"), Some(&10.0));
        assert_eq!(result1.get("b"), Some(&20.0));
    }

    #[test]
    fn t02_document_only_evaluation() {
        let doc_vars = vec![
            Variable {
                name: "a".to_string(),
                expr: "10".to_string(),
            },
            Variable {
                name: "b".to_string(),
                expr: "${a} * 2".to_string(),
            },
            Variable {
                name: "c".to_string(),
                expr: "${b} + ${a}".to_string(),
            },
        ];

        let sketch_vars: Vec<Variable> = vec![];
        let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

        assert_eq!(result.get("a"), Some(&10.0));
        assert_eq!(result.get("b"), Some(&20.0));
        assert_eq!(result.get("c"), Some(&30.0));
    }

    #[test]
    fn t03_shadowing_document_variable() {
        let doc_vars = vec![Variable {
            name: "a".to_string(),
            expr: "10".to_string(),
        }];

        let sketch_vars = vec![Variable {
            name: "b".to_string(),
            expr: "${a} + 5".to_string(),
        }];

        let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

        assert_eq!(result.get("a"), Some(&10.0));
        assert_eq!(result.get("b"), Some(&15.0));
    }

    #[test]
    fn t03b_shadowing_override() {
        let doc_vars = vec![Variable {
            name: "a".to_string(),
            expr: "10".to_string(),
        }];

        let sketch_vars = vec![
            Variable {
                name: "a".to_string(),
                expr: "20".to_string(),
            },
            Variable {
                name: "b".to_string(),
                expr: "${a} + 5".to_string(),
            },
        ];

        let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

        assert_eq!(result.get("a"), Some(&20.0));
        assert_eq!(result.get("b"), Some(&25.0));
    }

    #[test]
    fn t03c_doc_only_when_no_sketch() {
        let doc_vars = vec![Variable {
            name: "a".to_string(),
            expr: "10".to_string(),
        }];
        let sketch_vars: Vec<Variable> = vec![];

        let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result.get("a"), Some(&10.0));
    }

    #[test]
    fn t03d_sketch_only_when_no_doc() {
        let doc_vars: Vec<Variable> = vec![];
        let sketch_vars = vec![Variable {
            name: "a".to_string(),
            expr: "10".to_string(),
        }];

        let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result.get("a"), Some(&10.0));
    }

    #[test]
    fn t05_arithmetic_precedence() {
        let doc_vars = vec![Variable {
            name: "a".to_string(),
            expr: "2 + 3 * 4".to_string(),
        }];

        let result = evaluate_scope(&doc_vars, &[]).unwrap();
        assert_eq!(result.get("a"), Some(&14.0));
    }

    #[test]
    fn t06_parentheses() {
        let doc_vars = vec![Variable {
            name: "a".to_string(),
            expr: "(2 + 3) * 4".to_string(),
        }];

        let result = evaluate_scope(&doc_vars, &[]).unwrap();
        assert_eq!(result.get("a"), Some(&20.0));
    }

    #[test]
    fn t07_unary_minus() {
        let doc_vars = vec![
            Variable {
                name: "a".to_string(),
                expr: "-5".to_string(),
            },
            Variable {
                name: "b".to_string(),
                expr: "${a} * 2".to_string(),
            },
        ];

        let result = evaluate_scope(&doc_vars, &[]).unwrap();
        assert_eq!(result.get("a"), Some(&-5.0));
        assert_eq!(result.get("b"), Some(&-10.0));
    }

    #[test]
    fn t08_division() {
        let doc_vars = vec![Variable {
            name: "a".to_string(),
            expr: "10 / 4".to_string(),
        }];

        let result = evaluate_scope(&doc_vars, &[]).unwrap();
        assert_eq!(result.get("a"), Some(&2.5));
    }

    #[test]
    fn t_deg_circular() {
        let doc_vars = vec![
            Variable {
                name: "a".to_string(),
                expr: "${b}".to_string(),
            },
            Variable {
                name: "b".to_string(),
                expr: "${a}".to_string(),
            },
        ];

        let result = evaluate_scope(&doc_vars, &[]);
        assert!(matches!(result, Err(EvalError::CircularDependency { .. })));
    }

    #[test]
    fn t_deg_circular_self() {
        let doc_vars = vec![Variable {
            name: "a".to_string(),
            expr: "${a}".to_string(),
        }];

        let result = evaluate_scope(&doc_vars, &[]);
        match result {
            Err(EvalError::CircularDependency { ref cycle }) => {
                assert_eq!(cycle, &["a", "a"]);
            }
            other => panic!("expected CircularDependency, got {:?}", other),
        }
    }

    #[test]
    fn t_deg_undefined() {
        let doc_vars = vec![Variable {
            name: "a".to_string(),
            expr: "${missing}".to_string(),
        }];

        let result = evaluate_scope(&doc_vars, &[]);
        assert!(matches!(
            result,
            Err(EvalError::UndefinedVariable { name })
            if name == "missing"
        ));
    }

    #[test]
    fn t_boundary_empty_expr() {
        let doc_vars = vec![Variable {
            name: "a".to_string(),
            expr: "".to_string(),
        }];

        let result = evaluate_scope(&doc_vars, &[]);
        assert!(matches!(result, Err(EvalError::EmptyExpression { .. })));
    }

    #[test]
    fn t_boundary_whitespace_expr() {
        let doc_vars = vec![Variable {
            name: "a".to_string(),
            expr: "   ".to_string(),
        }];

        let result = evaluate_scope(&doc_vars, &[]);
        assert!(matches!(result, Err(EvalError::EmptyExpression { .. })));
    }

    #[test]
    fn t_deg_sketch_self_shadow_self_ref() {
        let doc_vars = vec![Variable {
            name: "a".to_string(),
            expr: "10".to_string(),
        }];

        let sketch_vars = vec![Variable {
            name: "a".to_string(),
            expr: "${a} + 1".to_string(),
        }];

        let result = evaluate_scope(&doc_vars, &sketch_vars);
        assert!(matches!(result, Err(EvalError::CircularDependency { .. })));
    }

    #[test]
    fn t_deg_parse_invalid() {
        let doc_vars = vec![Variable {
            name: "a".to_string(),
            expr: "2 + + 3".to_string(),
        }];

        let result = evaluate_scope(&doc_vars, &[]);
        assert!(matches!(result, Err(EvalError::Parse { .. })));
    }

    #[test]
    fn t_deg_div_by_zero() {
        let doc_vars = vec![Variable {
            name: "a".to_string(),
            expr: "1 / 0".to_string(),
        }];

        let result = evaluate_scope(&doc_vars, &[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().get("a"), Some(&f64::INFINITY));
    }

    /// T_REGRESSION_doc_not_shadowed_by_sketch_scope (M-F01 回帰防止):
    /// Document 変数の式は Sketch shadowing の影響を受けない (2-stage lexical scope)。
    #[test]
    fn t_regression_doc_not_shadowed_by_sketch_scope() {
        let doc_vars = vec![
            Variable {
                name: "a".to_string(),
                expr: "10".to_string(),
            },
            Variable {
                name: "b".to_string(),
                expr: "${a}".to_string(),
            }, // doc 内で a を参照
        ];
        let sketch_vars = vec![Variable {
            name: "a".to_string(),
            expr: "20".to_string(),
        }]; // sketch で shadowing

        let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

        // sketch scope では a=20 (shadow)
        assert_eq!(
            result.get("a"),
            Some(&20.0),
            "sketch.a shadows doc.a in combined result"
        );
        // **重要**: doc.b は doc.a を参照すべき (sketch.a ではない) → b=10
        assert_eq!(
            result.get("b"),
            Some(&10.0),
            "doc.b must evaluate against doc.a (=10), not sketch.a (=20)"
        );
    }

    /// T_DEG_doc_duplicate_name (R4 regression):
    /// 同一 Document scope に同名 Variable が複数あれば EvalError::DuplicateVariable
    #[test]
    fn t_deg_doc_duplicate_name() {
        let doc_vars = vec![
            Variable {
                name: "a".into(),
                expr: "1".into(),
            },
            Variable {
                name: "a".into(),
                expr: "2".into(),
            },
        ];
        let result = evaluate_scope(&doc_vars, &[]);
        match result {
            Err(EvalError::DuplicateVariable { scope, name }) => {
                assert_eq!(scope, "document");
                assert_eq!(name, "a");
            }
            other => panic!("expected DuplicateVariable in document, got {other:?}"),
        }
    }

    /// T_DEG_sketch_duplicate_name (R4 regression):
    /// 同一 Sketch scope に同名 Variable が複数あれば EvalError::DuplicateVariable
    /// (Document との shadowing は OK、Sketch 内重複は NG)
    #[test]
    fn t_deg_sketch_duplicate_name() {
        let doc_vars = vec![];
        let sketch_vars = vec![
            Variable {
                name: "x".into(),
                expr: "10".into(),
            },
            Variable {
                name: "x".into(),
                expr: "20".into(),
            },
        ];
        let result = evaluate_scope(&doc_vars, &sketch_vars);
        match result {
            Err(EvalError::DuplicateVariable { scope, name }) => {
                assert_eq!(scope, "sketch");
                assert_eq!(name, "x");
            }
            other => panic!("expected DuplicateVariable in sketch, got {other:?}"),
        }
    }

    /// T_OK_cross_scope_shadowing_not_duplicate (R4 regression):
    /// Document.a と Sketch.a の同名は重複ではなく shadowing (ADR-015 §2) → OK
    #[test]
    fn t_ok_cross_scope_shadowing_not_duplicate() {
        let doc_vars = vec![Variable {
            name: "a".into(),
            expr: "10".into(),
        }];
        let sketch_vars = vec![Variable {
            name: "a".into(),
            expr: "20".into(),
        }];
        let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();
        assert_eq!(result.get("a"), Some(&20.0)); // sketch shadows doc
    }

    /// T_DEG_doc_var_empty_name (R6 regression):
    /// evaluate_scope 直呼びでも doc.variables の空名 → InvalidVariableName(document)
    #[test]
    fn t_deg_doc_var_empty_name() {
        let doc_vars = vec![Variable {
            name: "".into(),
            expr: "1".into(),
        }];
        let result = evaluate_scope(&doc_vars, &[]);
        match result {
            Err(EvalError::InvalidVariableName { scope, name, .. }) => {
                assert_eq!(scope, "document");
                assert_eq!(name, "");
            }
            other => panic!("expected InvalidVariableName, got {other:?}"),
        }
    }

    /// T_DEG_sketch_var_invalid_char (R6 regression):
    /// evaluate_scope 直呼びでも sketch.variables の不正文字 → InvalidVariableName(sketch)
    #[test]
    fn t_deg_sketch_var_invalid_char() {
        let sketch_vars = vec![Variable {
            name: "a b".into(),
            expr: "1".into(),
        }];
        let result = evaluate_scope(&[], &sketch_vars);
        match result {
            Err(EvalError::InvalidVariableName { scope, name, .. }) => {
                assert_eq!(scope, "sketch");
                assert_eq!(name, "a b");
            }
            other => panic!("expected InvalidVariableName, got {other:?}"),
        }
    }
}
