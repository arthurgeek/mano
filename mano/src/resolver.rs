use std::collections::HashMap;

use crate::INITIALIZER_NAME;
use crate::ast::{Expr, Span, Stmt};
use crate::error::ManoError;
use crate::token::{Literal, Token, TokenType};

/// Maps expression spans to their resolved (distance, slot) pair
/// - distance: how many scopes to walk up
/// - slot: index within that scope's variable array
pub type Resolutions = HashMap<Span, (usize, usize)>;

/// Tracks function context for validation (return statements)
#[derive(Clone, Copy, PartialEq)]
enum FunctionType {
    None,
    Function,
    Method,
    Initializer,
}

/// Tracks class context for validation (this/oCara and super/mestre usage)
#[derive(Clone, Copy, PartialEq)]
enum ClassType {
    None,
    Class,
    Subclass,
    StaticMethod,
}

/// Info tracked for each variable in a scope
#[derive(Clone)]
struct VarInfo {
    defined: bool,
    used: bool,
    span: Span,
    slot: usize,
}

pub struct Resolver {
    /// Stack of scopes. Each scope maps variable names to their info.
    scopes: Vec<HashMap<String, VarInfo>>,
    /// Resolved variable distances
    resolutions: Resolutions,
    /// Current function context
    current_function: FunctionType,
    /// Current class context
    current_class: ClassType,
    /// Accumulated errors
    errors: Vec<ManoError>,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            scopes: Vec::new(),
            resolutions: HashMap::new(),
            current_function: FunctionType::None,
            current_class: ClassType::None,
            errors: Vec::new(),
        }
    }

    /// Main entry point - resolve all statements
    pub fn resolve(mut self, statements: &[Stmt]) -> Result<Resolutions, Vec<ManoError>> {
        for stmt in statements {
            self.resolve_stmt(stmt);
        }
        if self.errors.is_empty() {
            Ok(self.resolutions)
        } else {
            Err(self.errors)
        }
    }

    fn begin_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn end_scope(&mut self) {
        if let Some(scope) = self.scopes.pop() {
            for (name, info) in scope {
                // Variables starting with _ are intentionally unused (like Rust)
                if !info.used && !name.starts_with('_') {
                    self.errors.push(ManoError::Resolution {
                        message: format!(
                            "E aí, mano? A variável '{}' nunca foi usada! Se é de propósito, chama ela de '_{}'.",
                            name, name
                        ),
                        span: info.span,
                    });
                }
            }
        }
    }

    fn declare(&mut self, name: &Token) {
        if let Some(scope) = self.scopes.last_mut() {
            if scope.contains_key(&name.lexeme) {
                self.errors.push(ManoError::Resolution {
                    message: format!(
                        "Já tem uma '{}' aqui, chapa! Tá querendo confundir o corre?",
                        name.lexeme
                    ),
                    span: name.span.clone(),
                });
            }
            // Assign slot index based on current scope size
            let slot = scope.len();
            scope.insert(
                name.lexeme.clone(),
                VarInfo {
                    defined: false,
                    used: false,
                    span: name.span.clone(),
                    slot,
                },
            );
        }
    }

    fn define(&mut self, name: &Token) {
        if let Some(scope) = self.scopes.last_mut()
            && let Some(info) = scope.get_mut(&name.lexeme)
        {
            info.defined = true;
        }
    }

    fn resolve_local(&mut self, name: &Token) {
        let len = self.scopes.len();
        for i in 0..len {
            let scope_idx = len - 1 - i;
            if self.scopes[scope_idx].contains_key(&name.lexeme) {
                // Mark variable as used and get its slot
                if let Some(info) = self.scopes[scope_idx].get_mut(&name.lexeme) {
                    info.used = true;
                    self.resolutions.insert(name.span.clone(), (i, info.slot));
                }
                return;
            }
        }
        // Not found = global variable (looked up dynamically at runtime)
    }

    fn resolve_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Block { statements, .. } => {
                self.begin_scope();
                for s in statements {
                    self.resolve_stmt(s);
                }
                self.end_scope();
            }
            Stmt::Var {
                name, initializer, ..
            } => {
                self.declare(name);
                if let Some(init) = initializer {
                    self.resolve_expr_checking_self_ref(init, name);
                }
                self.define(name);
            }
            Stmt::Print { expression, .. } => {
                self.resolve_expr(expression);
            }
            Stmt::Function {
                name, params, body, ..
            } => {
                self.declare(name);
                self.define(name);
                self.resolve_function(params, body, FunctionType::Function);
            }
            Stmt::Return { keyword, value, .. } => {
                if self.current_function == FunctionType::None {
                    self.errors.push(ManoError::Resolution {
                        message: "Toma sem fita? Só pode dar toma dentro de uma função, tio!"
                            .to_string(),
                        span: keyword.span.clone(),
                    });
                }
                if let Some(v) = value {
                    if self.current_function == FunctionType::Initializer {
                        self.errors.push(ManoError::Resolution {
                            message: "E aí, mano? Não pode retornar valor do bora! Já retorna oCara automaticamente."
                                .to_string(),
                            span: keyword.span.clone(),
                        });
                    }
                    self.resolve_expr(v);
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.resolve_expr(condition);
                self.resolve_stmt(then_branch);
                if let Some(eb) = else_branch {
                    self.resolve_stmt(eb);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.resolve_expr(condition);
                self.resolve_stmt(body);
            }
            Stmt::Expression { expression, .. } => {
                self.resolve_expr(expression);
            }
            Stmt::Else { body, .. } => {
                self.resolve_stmt(body);
            }
            Stmt::Break { .. } => {}
            Stmt::Class {
                name,
                superclass,
                methods,
                ..
            } => {
                self.declare(name);
                self.define(name);

                let enclosing_class = self.current_class;

                // Resolve superclass if present
                if let Some(superclass_expr) = superclass {
                    // Check for self-inheritance
                    if let Expr::Variable {
                        name: superclass_name,
                    } = superclass_expr.as_ref()
                        && superclass_name.lexeme == name.lexeme
                    {
                        self.errors.push(ManoError::Resolution {
                            message: "Não dá pra ser cria de si mesmo, mano!".to_string(),
                            span: superclass_name.span.clone(),
                        });
                    }
                    self.resolve_expr(superclass_expr);

                    self.current_class = ClassType::Subclass;

                    // Create a scope for "mestre" that wraps oCara and methods
                    self.begin_scope();
                    if let Some(scope) = self.scopes.last_mut() {
                        scope.insert(
                            "mestre".to_string(),
                            VarInfo {
                                defined: true,
                                used: true, // Don't warn about unused mestre
                                span: name.span.clone(),
                                slot: 0,
                            },
                        );
                    }
                } else {
                    self.current_class = ClassType::Class;
                }

                // Create a scope for "oCara" that wraps all methods
                self.begin_scope();
                // Define "oCara" in this scope (slot 0, first in scope)
                if let Some(scope) = self.scopes.last_mut() {
                    scope.insert(
                        "oCara".to_string(),
                        VarInfo {
                            defined: true,
                            used: true, // Don't warn about unused oCara
                            span: name.span.clone(),
                            slot: 0,
                        },
                    );
                }

                for method in methods {
                    if let Stmt::Function {
                        name,
                        params,
                        body,
                        is_static,
                        ..
                    } = method
                    {
                        let fn_type = if name.lexeme == INITIALIZER_NAME {
                            FunctionType::Initializer
                        } else {
                            FunctionType::Method
                        };

                        // Static methods don't have access to oCara
                        let saved_class = self.current_class;
                        if *is_static {
                            self.current_class = ClassType::StaticMethod;
                        }

                        self.resolve_function(params, body, fn_type);

                        if *is_static {
                            self.current_class = saved_class;
                        }
                    }
                }

                self.end_scope(); // oCara scope

                // Close mestre scope if we created one
                if superclass.is_some() {
                    self.end_scope();
                }

                self.current_class = enclosing_class;
            }
        }
    }

    fn resolve_function(&mut self, params: &[Token], body: &[Stmt], fn_type: FunctionType) {
        let enclosing_function = self.current_function;
        self.current_function = fn_type;

        self.begin_scope();
        for param in params {
            self.declare(param);
            self.define(param);
        }
        for stmt in body {
            self.resolve_stmt(stmt);
        }
        self.end_scope();

        self.current_function = enclosing_function;
    }

    fn resolve_expr(&mut self, expr: &Expr) {
        self.resolve_expr_inner(expr, None);
    }

    fn resolve_expr_checking_self_ref(&mut self, expr: &Expr, declaring: &Token) {
        self.resolve_expr_inner(expr, Some(declaring));
    }

    fn resolve_expr_inner(&mut self, expr: &Expr, declaring: Option<&Token>) {
        match expr {
            Expr::Variable { name } => {
                // Check for self-reference in initializer
                if let Some(decl) = declaring
                    && decl.lexeme == name.lexeme
                    && let Some(scope) = self.scopes.last()
                    && scope.get(&name.lexeme).is_some_and(|info| !info.defined)
                {
                    self.errors.push(ManoError::Resolution {
                        message: format!(
                            "E aí, mano? Não pode usar '{}' enquanto tá declarando ela!",
                            name.lexeme
                        ),
                        span: name.span.clone(),
                    });
                }
                self.resolve_local(name);
            }
            Expr::Assign { name, value } => {
                self.resolve_expr(value);
                self.resolve_local(name);
            }
            Expr::Binary {
                left,
                operator,
                right,
            } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
                self.check_binary_literal_types(left, operator, right);
            }
            Expr::Logical { left, right, .. } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }
            Expr::Unary { operator, right } => {
                self.resolve_expr(right);
                self.check_unary_literal_type(operator, right);
            }
            Expr::Grouping { expression } => {
                self.resolve_expr(expression);
            }
            Expr::Ternary {
                condition,
                then_branch,
                else_branch,
            } => {
                self.resolve_expr(condition);
                self.resolve_expr(then_branch);
                self.resolve_expr(else_branch);
            }
            Expr::Call {
                callee, arguments, ..
            } => {
                self.resolve_expr(callee);
                for arg in arguments {
                    self.resolve_expr(arg);
                }
            }
            Expr::Lambda { params, body } => {
                self.resolve_function(params, body, FunctionType::Function);
            }
            Expr::Literal { .. } => {}
            Expr::Get { object, .. } => {
                self.resolve_expr(object);
            }
            Expr::Set { object, value, .. } => {
                self.resolve_expr(value);
                self.resolve_expr(object);
            }
            Expr::This { keyword } => {
                if self.current_class == ClassType::None {
                    self.errors.push(ManoError::Resolution {
                        message: "E aí, mano? Não pode usar 'oCara' fora de um bagulho!"
                            .to_string(),
                        span: keyword.span.clone(),
                    });
                } else if self.current_class == ClassType::StaticMethod {
                    self.errors.push(ManoError::Resolution {
                        message: "E aí, mano? Não pode usar 'oCara' em fita estática!".to_string(),
                        span: keyword.span.clone(),
                    });
                }
                self.resolve_local(keyword);
            }
            Expr::Super { keyword, .. } => {
                if self.current_class == ClassType::None {
                    self.errors.push(ManoError::Resolution {
                        message: "E aí, mano? Não pode usar 'mestre' fora de um bagulho!"
                            .to_string(),
                        span: keyword.span.clone(),
                    });
                } else if self.current_class == ClassType::Class {
                    self.errors.push(ManoError::Resolution {
                        message: "E aí, mano? Não pode usar 'mestre' num bagulho sem coroa!"
                            .to_string(),
                        span: keyword.span.clone(),
                    });
                }
                self.resolve_local(keyword);
            }
        }
    }

    /// Check unary operator type compatibility for literals
    fn check_unary_literal_type(&mut self, operator: &Token, operand: &Expr) {
        // Only check if operand is a literal
        if let Expr::Literal { value } = operand
            && operator.token_type == TokenType::Minus
            && !matches!(value, Literal::Number(_))
        {
            self.errors.push(ManoError::Resolution {
                message: format!(
                    "E aí, chapa! Menos unário só funciona com número, não com {}!",
                    Self::literal_type_name(value)
                ),
                span: operator.span.clone(),
            });
        }
        // Note: Bang (!) works with any type (truthiness)
    }

    /// Check binary operator type compatibility for literals
    fn check_binary_literal_types(&mut self, left: &Expr, operator: &Token, right: &Expr) {
        // Only check if both operands are literals
        let (left_lit, right_lit) = match (left, right) {
            (Expr::Literal { value: l }, Expr::Literal { value: r }) => (l, r),
            _ => return,
        };

        match operator.token_type {
            // Arithmetic: -, *, /, % require numbers
            TokenType::Minus | TokenType::Star | TokenType::Slash | TokenType::Percent => {
                if !matches!(left_lit, Literal::Number(_))
                    || !matches!(right_lit, Literal::Number(_))
                {
                    self.errors.push(ManoError::Resolution {
                        message: format!(
                            "Ô, parceiro! '{}' só funciona com números, não com {} e {}!",
                            operator.lexeme,
                            Self::literal_type_name(left_lit),
                            Self::literal_type_name(right_lit)
                        ),
                        span: operator.span.clone(),
                    });
                }
            }
            // Comparison: <, >, <=, >= require numbers
            TokenType::Less
            | TokenType::Greater
            | TokenType::LessEqual
            | TokenType::GreaterEqual => {
                if !matches!(left_lit, Literal::Number(_))
                    || !matches!(right_lit, Literal::Number(_))
                {
                    self.errors.push(ManoError::Resolution {
                        message: format!(
                            "Pô, mano! Comparação '{}' só rola com números, não com {} e {}!",
                            operator.lexeme,
                            Self::literal_type_name(left_lit),
                            Self::literal_type_name(right_lit)
                        ),
                        span: operator.span.clone(),
                    });
                }
            }
            // Plus: either both numbers or both strings
            TokenType::Plus => {
                let both_numbers = matches!(left_lit, Literal::Number(_))
                    && matches!(right_lit, Literal::Number(_));
                let both_strings = matches!(left_lit, Literal::String(_))
                    && matches!(right_lit, Literal::String(_));

                if !both_numbers && !both_strings {
                    self.errors.push(ManoError::Resolution {
                        message: format!(
                            "Aí não dá, mano! '+' só funciona com dois números ou duas strings, não com {} e {}!",
                            Self::literal_type_name(left_lit),
                            Self::literal_type_name(right_lit)
                        ),
                        span: operator.span.clone(),
                    });
                }
            }
            // Equality: ==, != work with any types
            _ => {}
        }
    }

    /// Get human-readable type name for a literal
    fn literal_type_name(lit: &Literal) -> &'static str {
        match lit {
            Literal::Number(_) => "número",
            Literal::String(_) => "string",
            Literal::Bool(_) => "booleano",
            Literal::Nil => "nadaNão",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Expr;
    use crate::token::{Literal, Token, TokenType};

    fn make_token(lexeme: &str, span: Span) -> Token {
        Token {
            token_type: TokenType::Identifier,
            lexeme: lexeme.to_string(),
            literal: None,
            span,
        }
    }

    #[test]
    fn resolver_creates_empty_resolutions() {
        let resolver = Resolver::new();
        assert!(resolver.resolutions.is_empty());
    }

    #[test]
    fn resolver_handles_empty_program() {
        let resolver = Resolver::new();
        let result = resolver.resolve(&[]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn resolver_resolves_local_variable_distance_0() {
        // { seLiga x = 1; salve x; }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token("x", 10..11),
                    initializer: Some(Expr::Literal {
                        value: Literal::Number(1.0),
                    }),
                    span: 0..15,
                },
                Stmt::Print {
                    expression: Expr::Variable {
                        name: make_token("x", 20..21),
                    },
                    span: 16..25,
                },
            ],
            span: 0..30,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
        let resolutions = result.unwrap();
        // The variable access at span 20..21 should resolve to distance 0
        assert_eq!(resolutions.get(&(20..21)), Some(&(0, 0)));
    }

    #[test]
    fn resolver_resolves_enclosing_variable_distance_1() {
        // { seLiga x = 1; { salve x; } }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token("x", 10..11),
                    initializer: Some(Expr::Literal {
                        value: Literal::Number(1.0),
                    }),
                    span: 0..15,
                },
                Stmt::Block {
                    statements: vec![Stmt::Print {
                        expression: Expr::Variable {
                            name: make_token("x", 30..31),
                        },
                        span: 25..35,
                    }],
                    span: 20..40,
                },
            ],
            span: 0..50,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
        let resolutions = result.unwrap();
        // The variable access at span 30..31 should resolve to distance 1
        assert_eq!(resolutions.get(&(30..31)), Some(&(1, 0)));
    }

    #[test]
    fn resolver_does_not_resolve_global() {
        // seLiga x = 1; salve x;  (at global scope, no block)
        let resolver = Resolver::new();
        let stmts = vec![
            Stmt::Var {
                name: make_token("x", 10..11),
                initializer: Some(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                span: 0..15,
            },
            Stmt::Print {
                expression: Expr::Variable {
                    name: make_token("x", 20..21),
                },
                span: 16..25,
            },
        ];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
        let resolutions = result.unwrap();
        // Global variables are not in the resolutions map
        assert!(!resolutions.contains_key(&(20..21)));
    }

    #[test]
    fn resolver_errors_on_self_reference_in_initializer() {
        // { seLiga a = a; }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![Stmt::Var {
                name: make_token("a", 10..11),
                initializer: Some(Expr::Variable {
                    name: make_token("a", 14..15),
                }),
                span: 0..20,
            }],
            span: 0..25,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        if let ManoError::Resolution { message, .. } = &errors[0] {
            assert!(message.contains("enquanto tá declarando"));
        } else {
            panic!("Expected Resolution error");
        }
    }

    #[test]
    fn resolver_errors_on_duplicate_in_same_scope() {
        // { seLiga _a; seLiga _a; }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token("_a", 10..12),
                    initializer: None,
                    span: 0..15,
                },
                Stmt::Var {
                    name: make_token("_a", 25..27),
                    initializer: None,
                    span: 20..30,
                },
            ],
            span: 0..35,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        if let ManoError::Resolution { message, .. } = &errors[0] {
            assert!(message.contains("Já tem uma"));
        } else {
            panic!("Expected Resolution error");
        }
    }

    #[test]
    fn resolver_allows_shadowing_in_nested_scope() {
        // { seLiga _a; { seLiga _a; } }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token("_a", 10..12),
                    initializer: None,
                    span: 0..15,
                },
                Stmt::Block {
                    statements: vec![Stmt::Var {
                        name: make_token("_a", 30..32),
                        initializer: None,
                        span: 25..35,
                    }],
                    span: 20..40,
                },
            ],
            span: 0..50,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_errors_on_return_outside_function() {
        // toma 1;
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Return {
            keyword: Token {
                token_type: TokenType::Return,
                lexeme: "toma".to_string(),
                literal: None,
                span: 0..4,
            },
            value: Some(Expr::Literal {
                value: Literal::Number(1.0),
            }),
            span: 0..10,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        if let ManoError::Resolution { message, .. } = &errors[0] {
            assert!(message.contains("Toma sem fita"));
        } else {
            panic!("Expected Resolution error");
        }
    }

    #[test]
    fn resolver_allows_return_inside_function() {
        // olhaEssaFita foo() { toma 1; }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Function {
            name: make_token("foo", 15..18),
            params: vec![],
            body: vec![Stmt::Return {
                keyword: Token {
                    token_type: TokenType::Return,
                    lexeme: "toma".to_string(),
                    literal: None,
                    span: 25..29,
                },
                value: Some(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                span: 25..35,
            }],
            is_static: false,
            is_getter: false,
            span: 0..40,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_resolves_function_params() {
        // olhaEssaFita foo(a) { salve a; }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Function {
            name: make_token("foo", 15..18),
            params: vec![make_token("a", 19..20)],
            body: vec![Stmt::Print {
                expression: Expr::Variable {
                    name: make_token("a", 30..31),
                },
                span: 25..35,
            }],
            is_static: false,
            is_getter: false,
            span: 0..40,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
        let resolutions = result.unwrap();
        // Parameter 'a' at 30..31 should resolve to distance 0
        assert_eq!(resolutions.get(&(30..31)), Some(&(0, 0)));
    }

    #[test]
    fn resolver_resolves_closure_variable() {
        // olhaEssaFita outer() { seLiga x = 1; olhaEssaFita _inner() { salve x; } }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Function {
            name: make_token("outer", 15..20),
            params: vec![],
            body: vec![
                Stmt::Var {
                    name: make_token("x", 30..31),
                    initializer: Some(Expr::Literal {
                        value: Literal::Number(1.0),
                    }),
                    span: 25..40,
                },
                Stmt::Function {
                    name: make_token("_inner", 55..61),
                    params: vec![],
                    body: vec![Stmt::Print {
                        expression: Expr::Variable {
                            name: make_token("x", 75..76),
                        },
                        span: 70..80,
                    }],
                    is_static: false,
                    is_getter: false,
                    span: 45..90,
                },
            ],
            is_static: false,
            is_getter: false,
            span: 0..100,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
        let resolutions = result.unwrap();
        // x at 75..76 is in _inner's body, x is declared in outer's scope
        // _inner's scope is 1 hop away from outer's scope
        assert_eq!(resolutions.get(&(75..76)), Some(&(1, 0)));
    }

    #[test]
    fn resolver_resolves_lambda() {
        // { seLiga x = 1; seLiga _f = olhaEssaFita(a) { salve x + a; }; }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token("x", 10..11),
                    initializer: Some(Expr::Literal {
                        value: Literal::Number(1.0),
                    }),
                    span: 0..15,
                },
                Stmt::Var {
                    name: make_token("_f", 25..27),
                    initializer: Some(Expr::Lambda {
                        params: vec![make_token("a", 45..46)],
                        body: vec![Stmt::Print {
                            expression: Expr::Binary {
                                left: Box::new(Expr::Variable {
                                    name: make_token("x", 60..61),
                                }),
                                operator: Token {
                                    token_type: TokenType::Plus,
                                    lexeme: "+".to_string(),
                                    literal: None,
                                    span: 62..63,
                                },
                                right: Box::new(Expr::Variable {
                                    name: make_token("a", 64..65),
                                }),
                            },
                            span: 55..70,
                        }],
                    }),
                    span: 20..80,
                },
            ],
            span: 0..90,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
        let resolutions = result.unwrap();
        // x at 60..61 is in lambda body, x declared in outer block (distance 1)
        assert_eq!(resolutions.get(&(60..61)), Some(&(1, 0)));
        // a at 64..65 is in lambda body, a is param (distance 0)
        assert_eq!(resolutions.get(&(64..65)), Some(&(0, 0)));
    }

    #[test]
    fn resolver_resolves_variable_in_if_condition() {
        // { seLiga x = 1; sePá (x) { } }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token("x", 10..11),
                    initializer: Some(Expr::Literal {
                        value: Literal::Number(1.0),
                    }),
                    span: 0..15,
                },
                Stmt::If {
                    condition: Expr::Variable {
                        name: make_token("x", 25..26),
                    },
                    then_branch: Box::new(Stmt::Block {
                        statements: vec![],
                        span: 30..35,
                    }),
                    else_branch: None,
                    span: 20..40,
                },
            ],
            span: 0..50,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
        let resolutions = result.unwrap();
        assert_eq!(resolutions.get(&(25..26)), Some(&(0, 0)));
    }

    #[test]
    fn resolver_resolves_variable_in_while_condition() {
        // { seLiga x = firmeza; segueOFluxo (x) { } }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token("x", 10..11),
                    initializer: Some(Expr::Literal {
                        value: Literal::Bool(true),
                    }),
                    span: 0..15,
                },
                Stmt::While {
                    condition: Expr::Variable {
                        name: make_token("x", 30..31),
                    },
                    body: Box::new(Stmt::Block {
                        statements: vec![],
                        span: 35..40,
                    }),
                    span: 20..45,
                },
            ],
            span: 0..50,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
        let resolutions = result.unwrap();
        assert_eq!(resolutions.get(&(30..31)), Some(&(0, 0)));
    }

    #[test]
    fn resolver_resolves_variable_in_expression_statement() {
        // { seLiga x = 1; x; }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token("x", 10..11),
                    initializer: Some(Expr::Literal {
                        value: Literal::Number(1.0),
                    }),
                    span: 0..15,
                },
                Stmt::Expression {
                    expression: Expr::Variable {
                        name: make_token("x", 20..21),
                    },
                    span: 16..25,
                },
            ],
            span: 0..30,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
        let resolutions = result.unwrap();
        assert_eq!(resolutions.get(&(20..21)), Some(&(0, 0)));
    }

    #[test]
    fn resolver_resolves_variable_in_else_body() {
        // { seLiga x = 1; sePá (treta) { } vacilou { salve x; } }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token("x", 10..11),
                    initializer: Some(Expr::Literal {
                        value: Literal::Number(1.0),
                    }),
                    span: 0..15,
                },
                Stmt::If {
                    condition: Expr::Literal {
                        value: Literal::Bool(false),
                    },
                    then_branch: Box::new(Stmt::Block {
                        statements: vec![],
                        span: 30..35,
                    }),
                    else_branch: Some(Box::new(Stmt::Else {
                        body: Box::new(Stmt::Print {
                            expression: Expr::Variable {
                                name: make_token("x", 50..51),
                            },
                            span: 45..55,
                        }),
                        span: 40..60,
                    })),
                    span: 20..65,
                },
            ],
            span: 0..70,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
        let resolutions = result.unwrap();
        assert_eq!(resolutions.get(&(50..51)), Some(&(0, 0)));
    }

    #[test]
    fn resolver_handles_break_statement() {
        // { segueOFluxo (firmeza) { saiFora; } }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![Stmt::While {
                condition: Expr::Literal {
                    value: Literal::Bool(true),
                },
                body: Box::new(Stmt::Block {
                    statements: vec![Stmt::Break { span: 30..37 }],
                    span: 25..40,
                }),
                span: 10..45,
            }],
            span: 0..50,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_resolves_logical_expression() {
        // { seLiga x = firmeza; seLiga _y = x tamoJunto firmeza; }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token("x", 10..11),
                    initializer: Some(Expr::Literal {
                        value: Literal::Bool(true),
                    }),
                    span: 0..20,
                },
                Stmt::Var {
                    name: make_token("_y", 30..32),
                    initializer: Some(Expr::Logical {
                        left: Box::new(Expr::Variable {
                            name: make_token("x", 40..41),
                        }),
                        operator: Token {
                            token_type: TokenType::And,
                            lexeme: "tamoJunto".to_string(),
                            literal: None,
                            span: 42..51,
                        },
                        right: Box::new(Expr::Literal {
                            value: Literal::Bool(true),
                        }),
                    }),
                    span: 25..60,
                },
            ],
            span: 0..65,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
        let resolutions = result.unwrap();
        // x at 40..41 resolves to distance 0
        assert_eq!(resolutions.get(&(40..41)), Some(&(0, 0)));
    }

    #[test]
    fn resolver_errors_on_unused_local_variable() {
        // { seLiga x = 1; } -- x is never used
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![Stmt::Var {
                name: make_token("x", 10..11),
                initializer: Some(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                span: 0..15,
            }],
            span: 0..20,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Resolution { .. }));
        if let ManoError::Resolution { message, .. } = &errors[0] {
            assert!(message.contains("nunca foi usada"));
        }
    }

    #[test]
    fn resolver_no_error_when_variable_starts_with_underscore() {
        // { seLiga _x = 1; } -- _x is intentionally unused
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![Stmt::Var {
                name: make_token("_x", 10..12),
                initializer: Some(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                span: 0..15,
            }],
            span: 0..20,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_no_error_when_variable_is_used() {
        // { seLiga x = 1; salve x; } -- x is used
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token("x", 10..11),
                    initializer: Some(Expr::Literal {
                        value: Literal::Number(1.0),
                    }),
                    span: 0..15,
                },
                Stmt::Print {
                    expression: Expr::Variable {
                        name: make_token("x", 25..26),
                    },
                    span: 20..30,
                },
            ],
            span: 0..35,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_resolves_ternary_expression() {
        // { seLiga x = 1; seLiga _y = x ? x : x; }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token("x", 10..11),
                    initializer: Some(Expr::Literal {
                        value: Literal::Number(1.0),
                    }),
                    span: 0..20,
                },
                Stmt::Var {
                    name: make_token("_y", 30..32),
                    initializer: Some(Expr::Ternary {
                        condition: Box::new(Expr::Variable {
                            name: make_token("x", 40..41),
                        }),
                        then_branch: Box::new(Expr::Variable {
                            name: make_token("x", 50..51),
                        }),
                        else_branch: Box::new(Expr::Variable {
                            name: make_token("x", 60..61),
                        }),
                    }),
                    span: 25..70,
                },
            ],
            span: 0..75,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
        let resolutions = result.unwrap();
        // All x references resolve to distance 0
        assert_eq!(resolutions.get(&(40..41)), Some(&(0, 0)));
        assert_eq!(resolutions.get(&(50..51)), Some(&(0, 0)));
        assert_eq!(resolutions.get(&(60..61)), Some(&(0, 0)));
    }

    #[test]
    fn resolver_assigns_slot_indices_to_variables() {
        // { seLiga a = 1; seLiga b = 2; salve a; salve b; }
        // a should be slot 0, b should be slot 1
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Block {
            statements: vec![
                Stmt::Var {
                    name: make_token("a", 10..11),
                    initializer: Some(Expr::Literal {
                        value: Literal::Number(1.0),
                    }),
                    span: 0..15,
                },
                Stmt::Var {
                    name: make_token("b", 25..26),
                    initializer: Some(Expr::Literal {
                        value: Literal::Number(2.0),
                    }),
                    span: 20..35,
                },
                Stmt::Print {
                    expression: Expr::Variable {
                        name: make_token("a", 45..46),
                    },
                    span: 40..50,
                },
                Stmt::Print {
                    expression: Expr::Variable {
                        name: make_token("b", 55..56),
                    },
                    span: 50..60,
                },
            ],
            span: 0..65,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
        let resolutions = result.unwrap();
        // a at 45..46: distance 0, slot 0
        assert_eq!(resolutions.get(&(45..46)), Some(&(0, 0)));
        // b at 55..56: distance 0, slot 1
        assert_eq!(resolutions.get(&(55..56)), Some(&(0, 1)));
    }

    // === Literal type checking tests ===

    #[test]
    fn resolver_errors_on_unary_minus_string() {
        // -"salve"
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Unary {
                operator: Token {
                    token_type: TokenType::Minus,
                    lexeme: "-".to_string(),
                    literal: None,
                    span: 0..1,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::String("salve".to_string()),
                }),
            },
            span: 0..10,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Resolution { .. }));
    }

    #[test]
    fn resolver_errors_on_unary_minus_bool() {
        // -firmeza
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Unary {
                operator: Token {
                    token_type: TokenType::Minus,
                    lexeme: "-".to_string(),
                    literal: None,
                    span: 0..1,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::Bool(true),
                }),
            },
            span: 0..10,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
    }

    #[test]
    fn resolver_errors_on_unary_minus_nil() {
        // -nadaNão
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Unary {
                operator: Token {
                    token_type: TokenType::Minus,
                    lexeme: "-".to_string(),
                    literal: None,
                    span: 0..1,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::Nil,
                }),
            },
            span: 0..10,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
    }

    #[test]
    fn resolver_allows_unary_minus_number() {
        // -42
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Unary {
                operator: Token {
                    token_type: TokenType::Minus,
                    lexeme: "-".to_string(),
                    literal: None,
                    span: 0..1,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::Number(42.0),
                }),
            },
            span: 0..5,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_errors_on_string_minus_string() {
        // "e ai" - "parca"
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::String("e ai".to_string()),
                }),
                operator: Token {
                    token_type: TokenType::Minus,
                    lexeme: "-".to_string(),
                    literal: None,
                    span: 7..8,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::String("parca".to_string()),
                }),
            },
            span: 0..20,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
    }

    #[test]
    fn resolver_errors_on_number_plus_string() {
        // 1 + "truta"
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                operator: Token {
                    token_type: TokenType::Plus,
                    lexeme: "+".to_string(),
                    literal: None,
                    span: 2..3,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::String("truta".to_string()),
                }),
            },
            span: 0..15,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
    }

    #[test]
    fn resolver_errors_on_number_less_string() {
        // 1 < "vei"
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                operator: Token {
                    token_type: TokenType::Less,
                    lexeme: "<".to_string(),
                    literal: None,
                    span: 2..3,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::String("vei".to_string()),
                }),
            },
            span: 0..10,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
    }

    #[test]
    fn resolver_errors_on_number_greater_string() {
        // 1 > "vei"
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                operator: Token {
                    token_type: TokenType::Greater,
                    lexeme: ">".to_string(),
                    literal: None,
                    span: 2..3,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::String("vei".to_string()),
                }),
            },
            span: 0..10,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
    }

    #[test]
    fn resolver_errors_on_number_less_equal_string() {
        // 1 <= "vei"
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                operator: Token {
                    token_type: TokenType::LessEqual,
                    lexeme: "<=".to_string(),
                    literal: None,
                    span: 2..4,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::String("vei".to_string()),
                }),
            },
            span: 0..11,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
    }

    #[test]
    fn resolver_errors_on_number_greater_equal_string() {
        // 1 >= "vei"
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                operator: Token {
                    token_type: TokenType::GreaterEqual,
                    lexeme: ">=".to_string(),
                    literal: None,
                    span: 2..4,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::String("vei".to_string()),
                }),
            },
            span: 0..11,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
    }

    #[test]
    fn resolver_allows_number_plus_number() {
        // 1 + 2
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                operator: Token {
                    token_type: TokenType::Plus,
                    lexeme: "+".to_string(),
                    literal: None,
                    span: 2..3,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::Number(2.0),
                }),
            },
            span: 0..5,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_allows_string_plus_string() {
        // "e ai" + "parca"
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::String("e ai".to_string()),
                }),
                operator: Token {
                    token_type: TokenType::Plus,
                    lexeme: "+".to_string(),
                    literal: None,
                    span: 7..8,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::String("parca".to_string()),
                }),
            },
            span: 0..20,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_allows_number_less_number() {
        // 1 < 2
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                operator: Token {
                    token_type: TokenType::Less,
                    lexeme: "<".to_string(),
                    literal: None,
                    span: 2..3,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::Number(2.0),
                }),
            },
            span: 0..5,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_allows_number_greater_number() {
        // 1 > 2
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                operator: Token {
                    token_type: TokenType::Greater,
                    lexeme: ">".to_string(),
                    literal: None,
                    span: 2..3,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::Number(2.0),
                }),
            },
            span: 0..5,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_allows_number_less_equal_number() {
        // 1 <= 2
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                operator: Token {
                    token_type: TokenType::LessEqual,
                    lexeme: "<=".to_string(),
                    literal: None,
                    span: 2..4,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::Number(2.0),
                }),
            },
            span: 0..6,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_allows_number_greater_equal_number() {
        // 1 >= 2
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                operator: Token {
                    token_type: TokenType::GreaterEqual,
                    lexeme: ">=".to_string(),
                    literal: None,
                    span: 2..4,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::Number(2.0),
                }),
            },
            span: 0..6,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_allows_equality_with_mixed_types() {
        // 1 == "a" is allowed (equality works on any types)
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                operator: Token {
                    token_type: TokenType::EqualEqual,
                    lexeme: "==".to_string(),
                    literal: None,
                    span: 2..4,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::String("a".to_string()),
                }),
            },
            span: 0..8,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_allows_bang_equal_with_mixed_types() {
        // 1 != "a" is allowed
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Binary {
                left: Box::new(Expr::Literal {
                    value: Literal::Number(1.0),
                }),
                operator: Token {
                    token_type: TokenType::BangEqual,
                    lexeme: "!=".to_string(),
                    literal: None,
                    span: 2..4,
                },
                right: Box::new(Expr::Literal {
                    value: Literal::String("a".to_string()),
                }),
            },
            span: 0..8,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_resolves_variable_in_method_body() {
        let resolver = Resolver::new();
        // bagulho Pessoa { falar(msg) { salve msg; } }
        let stmts = vec![Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Pessoa".to_string(),
                literal: None,
                span: 8..14,
            },
            superclass: None,
            methods: vec![Stmt::Function {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "falar".to_string(),
                    literal: None,
                    span: 17..22,
                },
                params: vec![Token {
                    token_type: TokenType::Identifier,
                    lexeme: "msg".to_string(),
                    literal: None,
                    span: 23..26,
                }],
                body: vec![Stmt::Print {
                    expression: Expr::Variable {
                        name: Token {
                            token_type: TokenType::Identifier,
                            lexeme: "msg".to_string(),
                            literal: None,
                            span: 35..38,
                        },
                    },
                    span: 29..39,
                }],
                is_static: false,
                is_getter: false,
                span: 17..42,
            }],
            span: 0..44,
        }];

        let resolutions = resolver.resolve(&stmts).unwrap();

        // The variable 'msg' at span 35..38 should be resolved
        // It's in the method scope (distance 0) at slot 0 (first param)
        assert_eq!(resolutions.get(&(35..38)), Some(&(0, 0)));
    }

    #[test]
    fn resolver_errors_on_duplicate_class_in_same_scope() {
        let mut resolver = Resolver::new();
        resolver.begin_scope();

        let stmts = vec![
            Stmt::Class {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "Pessoa".to_string(),
                    literal: None,
                    span: 8..14,
                },
                superclass: None,
                methods: vec![],
                span: 0..17,
            },
            Stmt::Class {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "Pessoa".to_string(),
                    literal: None,
                    span: 26..32,
                },
                superclass: None,
                methods: vec![],
                span: 18..35,
            },
        ];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], ManoError::Resolution { message, .. } if message.contains("Já tem uma"))
        );
    }

    #[test]
    fn resolver_resolves_get_expression() {
        let resolver = Resolver::new();
        // pessoa.nome;
        let stmts = vec![Stmt::Expression {
            expression: Expr::Get {
                object: Box::new(Expr::Variable {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "pessoa".to_string(),
                        literal: None,
                        span: 0..6,
                    },
                }),
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "nome".to_string(),
                    literal: None,
                    span: 7..11,
                },
            },
            span: 0..12,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_resolves_set_expression() {
        let resolver = Resolver::new();
        // pessoa.nome = "João";
        let stmts = vec![Stmt::Expression {
            expression: Expr::Set {
                object: Box::new(Expr::Variable {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "pessoa".to_string(),
                        literal: None,
                        span: 0..6,
                    },
                }),
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "nome".to_string(),
                    literal: None,
                    span: 7..11,
                },
                value: Box::new(Expr::Literal {
                    value: Literal::String("João".to_string()),
                }),
            },
            span: 0..20,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_resolves_chained_get_expression() {
        let resolver = Resolver::new();
        // pessoa.endereco.cidade;
        let stmts = vec![Stmt::Expression {
            expression: Expr::Get {
                object: Box::new(Expr::Get {
                    object: Box::new(Expr::Variable {
                        name: Token {
                            token_type: TokenType::Identifier,
                            lexeme: "pessoa".to_string(),
                            literal: None,
                            span: 0..6,
                        },
                    }),
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "endereco".to_string(),
                        literal: None,
                        span: 7..15,
                    },
                }),
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "cidade".to_string(),
                    literal: None,
                    span: 16..22,
                },
            },
            span: 0..23,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
    }

    #[test]
    fn resolver_resolves_this_in_method() {
        // bagulho Pessoa { getOCara() { toma oCara; } }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Pessoa".to_string(),
                literal: None,
                span: 8..14,
            },
            superclass: None,
            methods: vec![Stmt::Function {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "getOCara".to_string(),
                    literal: None,
                    span: 17..25,
                },
                params: vec![],
                body: vec![Stmt::Return {
                    keyword: Token {
                        token_type: TokenType::Return,
                        lexeme: "toma".to_string(),
                        literal: None,
                        span: 30..34,
                    },
                    value: Some(Expr::This {
                        keyword: Token {
                            token_type: TokenType::This,
                            lexeme: "oCara".to_string(),
                            literal: None,
                            span: 35..40,
                        },
                    }),
                    span: 30..41,
                }],
                is_static: false,
                is_getter: false,
                span: 17..45,
            }],
            span: 0..50,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok(), "Got errors: {:?}", result.unwrap_err());
    }

    #[test]
    fn resolver_errors_on_this_outside_class() {
        // Using oCara at top level should error
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::This {
                keyword: Token {
                    token_type: TokenType::This,
                    lexeme: "oCara".to_string(),
                    literal: None,
                    span: 0..5,
                },
            },
            span: 0..6,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(
            |e| matches!(e, ManoError::Resolution { message, .. } if message.contains("oCara"))
        ));
    }

    #[test]
    fn resolver_errors_on_this_in_function() {
        // Using oCara in a regular function (not method) should error
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Function {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "teste".to_string(),
                literal: None,
                span: 0..5,
            },
            params: vec![],
            body: vec![Stmt::Expression {
                expression: Expr::This {
                    keyword: Token {
                        token_type: TokenType::This,
                        lexeme: "oCara".to_string(),
                        literal: None,
                        span: 20..25,
                    },
                },
                span: 20..26,
            }],
            is_static: false,
            is_getter: false,
            span: 0..30,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(
            |e| matches!(e, ManoError::Resolution { message, .. } if message.contains("oCara"))
        ));
    }

    // === static methods ===

    #[test]
    fn resolver_errors_on_this_in_static_method() {
        // Using oCara in a static method should error
        // bagulho Pessoa { bagulho teste() { toma oCara; } }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Pessoa".to_string(),
                literal: None,
                span: 8..14,
            },
            superclass: None,
            methods: vec![Stmt::Function {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "teste".to_string(),
                    literal: None,
                    span: 24..29,
                },
                params: vec![],
                body: vec![Stmt::Return {
                    keyword: Token {
                        token_type: TokenType::Return,
                        lexeme: "toma".to_string(),
                        literal: None,
                        span: 35..39,
                    },
                    value: Some(Expr::This {
                        keyword: Token {
                            token_type: TokenType::This,
                            lexeme: "oCara".to_string(),
                            literal: None,
                            span: 40..45,
                        },
                    }),
                    span: 35..46,
                }],
                is_static: true,
                is_getter: false,
                span: 24..50,
            }],
            span: 0..55,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err(), "should error on oCara in static method");
        let errors = result.unwrap_err();
        assert!(errors.iter().any(
            |e| matches!(e, ManoError::Resolution { message, .. } if message.contains("oCara"))
        ));
    }

    #[test]
    fn resolver_errors_on_class_inheriting_from_itself() {
        // bagulho Foo < Foo { }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Foo".to_string(),
                literal: None,
                span: 8..11,
            },
            superclass: Some(Box::new(Expr::Variable {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "Foo".to_string(),
                    literal: None,
                    span: 14..17,
                },
            })),
            methods: vec![],
            span: 0..20,
        }];

        let result = resolver.resolve(&stmts);
        assert!(
            result.is_err(),
            "should error when class inherits from itself"
        );
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], ManoError::Resolution { message, .. } if message.contains("cria de si mesmo"))
        );
    }

    // === mestre (super) tests ===

    #[test]
    fn resolver_errors_on_super_outside_class() {
        // mestre.falar();
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Expression {
            expression: Expr::Super {
                keyword: Token {
                    token_type: TokenType::Super,
                    lexeme: "mestre".to_string(),
                    literal: None,
                    span: 0..6,
                },
                method: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "falar".to_string(),
                    literal: None,
                    span: 7..12,
                },
            },
            span: 0..14,
        }];

        let result = resolver.resolve(&stmts);
        assert!(result.is_err(), "should error on mestre outside class");
        let errors = result.unwrap_err();
        assert!(errors.iter().any(
            |e| matches!(e, ManoError::Resolution { message, .. } if message.contains("mestre"))
        ));
    }

    #[test]
    fn resolver_errors_on_super_in_class_without_superclass() {
        // bagulho Foo { test() { mestre.bar(); } }
        let resolver = Resolver::new();
        let stmts = vec![Stmt::Class {
            name: Token {
                token_type: TokenType::Identifier,
                lexeme: "Foo".to_string(),
                literal: None,
                span: 8..11,
            },
            superclass: None,
            methods: vec![Stmt::Function {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "test".to_string(),
                    literal: None,
                    span: 14..18,
                },
                params: vec![],
                body: vec![Stmt::Expression {
                    expression: Expr::Super {
                        keyword: Token {
                            token_type: TokenType::Super,
                            lexeme: "mestre".to_string(),
                            literal: None,
                            span: 24..30,
                        },
                        method: Token {
                            token_type: TokenType::Identifier,
                            lexeme: "bar".to_string(),
                            literal: None,
                            span: 31..34,
                        },
                    },
                    span: 24..37,
                }],
                is_static: false,
                is_getter: false,
                span: 14..40,
            }],
            span: 0..45,
        }];

        let result = resolver.resolve(&stmts);
        assert!(
            result.is_err(),
            "should error on mestre in class without superclass"
        );
        let errors = result.unwrap_err();
        assert!(errors.iter().any(
            |e| matches!(e, ManoError::Resolution { message, .. } if message.contains("mestre"))
        ));
    }

    #[test]
    fn resolver_allows_super_in_subclass() {
        // bagulho Pai { } bagulho Filho < Pai { test() { mestre.foo(); } }
        let resolver = Resolver::new();
        let stmts = vec![
            Stmt::Class {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "Pai".to_string(),
                    literal: None,
                    span: 8..11,
                },
                superclass: None,
                methods: vec![],
                span: 0..14,
            },
            Stmt::Class {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "Filho".to_string(),
                    literal: None,
                    span: 23..28,
                },
                superclass: Some(Box::new(Expr::Variable {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "Pai".to_string(),
                        literal: None,
                        span: 31..34,
                    },
                })),
                methods: vec![Stmt::Function {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "test".to_string(),
                        literal: None,
                        span: 37..41,
                    },
                    params: vec![],
                    body: vec![Stmt::Expression {
                        expression: Expr::Super {
                            keyword: Token {
                                token_type: TokenType::Super,
                                lexeme: "mestre".to_string(),
                                literal: None,
                                span: 47..53,
                            },
                            method: Token {
                                token_type: TokenType::Identifier,
                                lexeme: "foo".to_string(),
                                literal: None,
                                span: 54..57,
                            },
                        },
                        span: 47..60,
                    }],
                    is_static: false,
                    is_getter: false,
                    span: 37..63,
                }],
                span: 15..66,
            },
        ];

        let result = resolver.resolve(&stmts);
        assert!(
            result.is_ok(),
            "should allow mestre in subclass: {:?}",
            result.unwrap_err()
        );
    }

    #[test]
    fn resolver_resolves_super_to_correct_distance() {
        // bagulho Pai { } bagulho Filho < Pai { test() { mestre.foo(); } }
        // mestre should resolve to the scope just outside the oCara scope
        let resolver = Resolver::new();
        let stmts = vec![
            Stmt::Class {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "Pai".to_string(),
                    literal: None,
                    span: 8..11,
                },
                superclass: None,
                methods: vec![],
                span: 0..14,
            },
            Stmt::Class {
                name: Token {
                    token_type: TokenType::Identifier,
                    lexeme: "Filho".to_string(),
                    literal: None,
                    span: 23..28,
                },
                superclass: Some(Box::new(Expr::Variable {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "Pai".to_string(),
                        literal: None,
                        span: 31..34,
                    },
                })),
                methods: vec![Stmt::Function {
                    name: Token {
                        token_type: TokenType::Identifier,
                        lexeme: "test".to_string(),
                        literal: None,
                        span: 37..41,
                    },
                    params: vec![],
                    body: vec![Stmt::Expression {
                        expression: Expr::Super {
                            keyword: Token {
                                token_type: TokenType::Super,
                                lexeme: "mestre".to_string(),
                                literal: None,
                                span: 47..53,
                            },
                            method: Token {
                                token_type: TokenType::Identifier,
                                lexeme: "foo".to_string(),
                                literal: None,
                                span: 54..57,
                            },
                        },
                        span: 47..60,
                    }],
                    is_static: false,
                    is_getter: false,
                    span: 37..63,
                }],
                span: 15..66,
            },
        ];

        let result = resolver.resolve(&stmts);
        assert!(result.is_ok());
        let resolutions = result.unwrap();
        // mestre at 47..53 should resolve to distance 2:
        // - method scope (params)
        // - oCara scope
        // - mestre scope <- here
        assert_eq!(resolutions.get(&(47..53)), Some(&(2, 0)));
    }
}
