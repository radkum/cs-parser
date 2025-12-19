use std::{collections::BTreeSet, fmt::Display};

use super::program_result::PsValue;

/// Represents a parsed PowerShell method call token.
///
/// Stores the original token string, the method name, and its arguments as
/// `PsValue`s. Useful for analyzing and reconstructing method calls in scripts.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodToken {
    token: String,
    self_: PsValue,
    name: String,
    arguments: Vec<PsValue>,
}

impl MethodToken {
    pub fn new(token: String, self_: PsValue, name: String, arguments: Vec<PsValue>) -> Self {
        Self {
            token,
            self_,
            name,
            arguments,
        }
    }

    pub fn token(&self) -> &String {
        &self.token
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn self_(&self) -> &PsValue {
        &self.self_
    }

    pub fn args(&self) -> &Vec<PsValue> {
        &self.arguments
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionToken {
    token: String,
    name: String,
    arguments: Vec<String>,
}

/// Represents a parsed PowerShell command token.
///
/// Stores the original token string, the command name, and its arguments as
/// strings. Useful for identifying and reconstructing command invocations.
impl FunctionToken {
    pub fn new(token: String, name: String, arguments: Vec<String>) -> Self {
        Self {
            token,
            name,
            arguments,
        }
    }

    pub fn token(&self) -> &String {
        &self.token
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn args(&self) -> &Vec<String> {
        &self.arguments
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDeclarationToken {
    token: String,
    name: String,
    params: Vec<String>,
}

/// Represents a parsed PowerShell command token.
///
/// Stores the original token string, the command name, and its arguments as
/// strings. Useful for identifying and reconstructing command invocations.
impl FunctionDeclarationToken {
    pub fn new(token: String, name: String, params: Vec<String>) -> Self {
        Self {
            token,
            name,
            params,
        }
    }

    pub fn token(&self) -> &String {
        &self.token
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn params(&self) -> &Vec<String> {
        &self.params
    }
}

/// Represents a parsed PowerShell expression token.
///
/// Stores the original token string and its evaluated value as `PsValue`.
/// Useful for deobfuscation and analysis of expressions.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionToken {
    token: String,
    value: PsValue,
}

impl ExpressionToken {
    pub fn new(token: String, value: PsValue) -> Self {
        Self { token, value }
    }
}

/// Represents a double-quoted PowerShell string with variable expansion.
///
/// Stores the original token string and its expanded value.
/// Useful for tracking and reconstructing expandable strings in scripts.
#[derive(Debug, Clone, PartialEq)]
pub struct StringExpandableToken {
    token: String,
    value: String,
}

impl StringExpandableToken {
    pub fn new(token: String, value: String) -> Self {
        Self { token, value }
    }

    pub fn token(&self) -> &String {
        &self.token
    }

    pub fn value(&self) -> &String {
        &self.value
    }
}

/// Represents a parsed PowerShell expression token.
///
/// Stores the original token string and its evaluated value as `PsValue`.
/// Useful for deobfuscation and analysis of expressions.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeToken {
    token: String,
    args: Vec<(String, PsValue)>,
}

impl AttributeToken {
    pub fn new(token: String, args: Vec<(String, PsValue)>) -> Self {
        Self { token, args }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    StringExpandable(StringExpandableToken),
    String(String),
    Expression(ExpressionToken),
    Method(MethodToken),
    Function(FunctionToken),
    FunctionDeclaration(FunctionDeclarationToken),
    Attribute(AttributeToken),
}
impl Token {
    pub fn method(token: String, self_: PsValue, name: String, arguments: Vec<PsValue>) -> Self {
        Token::Method(MethodToken {
            token,
            self_,
            name,
            arguments,
        })
    }

    pub fn function(token: String, name: String, arguments: Vec<String>) -> Self {
        Token::Function(FunctionToken {
            token,
            name,
            arguments,
        })
    }

    pub fn function_declaration(token: String, name: String, params: Vec<String>) -> Self {
        Token::FunctionDeclaration(FunctionDeclarationToken {
            token,
            name,
            params,
        })
    }

    pub fn expression(token: String, value: PsValue) -> Self {
        Token::Expression(ExpressionToken { token, value })
    }

    pub fn string_expandable(token: String, value: String) -> Self {
        Token::StringExpandable(StringExpandableToken { token, value })
    }

    pub fn attribute(token: String, args: Vec<(String, PsValue)>) -> Self {
        Token::Attribute(AttributeToken { token, args })
    }
}
impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Tokens(Vec<Token>);
impl Tokens {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, token: Token) {
        self.0.push(token)
    }

    pub fn all(&self) -> Vec<Token> {
        self.0.clone()
    }

    pub fn literal_strings(&self) -> Vec<String> {
        self.0
            .iter()
            .filter_map(|token| match token {
                Token::String(literal) => Some(literal.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn string_set(&self) -> BTreeSet<String> {
        let mut string_set = BTreeSet::new();
        for token in self.0.iter() {
            match token {
                Token::String(deobfuscated)
                | Token::StringExpandable(StringExpandableToken {
                    value: deobfuscated,
                    ..
                }) => {
                    let _ = string_set.insert(deobfuscated.to_string());
                }
                _ => {}
            }
        }
        string_set
    }

    pub fn lowercased_string_set(&self) -> BTreeSet<String> {
        let mut string_set = BTreeSet::new();
        for token in self.0.iter() {
            match token {
                Token::String(deobfuscated)
                | Token::StringExpandable(StringExpandableToken {
                    value: deobfuscated,
                    ..
                }) => {
                    let _ = string_set.insert(deobfuscated.to_ascii_lowercase());
                }
                _ => {}
            }
        }
        string_set
    }

    pub fn expandable_strings(&self) -> Vec<StringExpandableToken> {
        self.0
            .iter()
            .filter_map(|token| match token {
                Token::StringExpandable(expandable) => Some(expandable.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn expressions(&self) -> Vec<ExpressionToken> {
        self.0
            .iter()
            .filter_map(|token| match token {
                Token::Expression(expr) => Some(expr.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn methods(&self) -> Vec<MethodToken> {
        self.0
            .iter()
            .filter_map(|token| match token {
                Token::Method(method) => Some(method.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn functions(&self) -> Vec<FunctionToken> {
        self.0
            .iter()
            .filter_map(|token| match token {
                Token::Function(command) => Some(command.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn function_declarations(&self) -> Vec<FunctionDeclarationToken> {
        self.0
            .iter()
            .filter_map(|token| match token {
                Token::FunctionDeclaration(command) => Some(command.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn attributes(&self) -> Vec<AttributeToken> {
        self.0
            .iter()
            .filter_map(|token| match token {
                Token::Attribute(attr) => Some(attr.clone()),
                _ => None,
            })
            .collect()
    }
}
