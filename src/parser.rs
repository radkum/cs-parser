mod attributes;
mod error;
mod flow_control;
mod predicates;
mod program_result;
mod stream_message;
mod token;
mod value;
mod variables;
use std::collections::HashMap;

use attributes::{Attribute, AttributeArg};
use flow_control::FlowControl;
pub(crate) use stream_message::StreamMessage;
use value::{
    ClassProperties, ClassType, FunctionHeader, MethodName, Param, RuntimeObjectTrait,
    RuntimeTypeTrait, ScriptBlock, ValResult,
};
type ParserResult<T> = core::result::Result<T, ParserError>;
use error::ParserError;
type PestError = pest::error::Error<Rule>;
use pest::Parser;
use pest_derive::Parser;
use predicates::{ArithmeticPred, BitwisePred, LogicalPred, StringPred};
pub use program_result::{ProgramResult, PsValue};
pub use token::{
    ExpressionToken, FunctionToken, MethodToken, StringExpandableToken, Token, Tokens,
};
pub(crate) use value::Val;
use value::{StringBuilderType, ValType};
pub use variables::Variables;
use variables::{VarName, VariableError};

use crate::parser::value::RuntimeError;

type Pair<'i> = ::pest::iterators::Pair<'i, Rule>;
type Pairs<'i> = ::pest::iterators::Pairs<'i, Rule>;

pub(crate) const NEWLINE: &str = "\n";

macro_rules! unexpected_token {
    ($pair:expr) => {
        panic!("Unexpected token: {:?}", $pair.as_rule())
    };
}

macro_rules! check_rule {
    ($pair:expr, $rule:pat) => {
        if !matches!($pair.as_rule(), $rule) {
            panic!(
                "Unexpected token: {:?}, instead of {}",
                $pair.as_rule(),
                stringify!($rule)
            );
        }
    };
}

macro_rules! not_implemented {
    ($token:expr) => {
        Err(ParserError::NotImplemented(format!(
            "Not implemented: {:?}",
            $token.as_rule()
        )))
    };
}

#[derive(Default, Clone)]
pub(crate) struct Results {
    output: Vec<StreamMessage>,
    deobfuscated: Vec<String>,
}

impl Results {
    fn new() -> Self {
        Self {
            output: Vec::new(),
            deobfuscated: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
enum Access<'a> {
    Member(&'a str),
    Element(Val),
    Method(&'a str, Vec<Val>),
    Function(Vec<Val>),
}

#[derive(Parser)]
#[grammar = "csharp.pest"]
pub struct CSharpSession {
    variables: Variables,
    tokens: Tokens,
    errors: Vec<ParserError>,
    results: Vec<Results>,
    skip_error: u32,
    types_map: HashMap<String, Box<dyn RuntimeTypeTrait>>,
}

impl Default for CSharpSession {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> CSharpSession {
    fn set_this(&mut self, val: ClassType) {
        self.variables.set_this(val);
    }

    /// Creates a new CSharp parsing session with default settings.
    ///
    /// The session is initialized with built-in variables like `$true`,
    /// `$false`, `$null`, and special variables like `$?` for error status
    /// tracking.
    ///
    /// # Returns
    ///
    /// A new `CSharpSession` instance ready for script evaluation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cs_parser::CSharpSession;
    ///
    /// let mut session = CSharpSession::new();
    /// let result = session.safe_eval("$true").unwrap();
    /// assert_eq!(result, "True");
    /// ```
    pub fn new() -> Self {
        let mut types_map = HashMap::new();
        types_map.insert(
            StringBuilderType::nname(),
            Box::new(StringBuilderType {}) as _,
        );
        Self {
            variables: Variables::new(),
            tokens: Tokens::new(),
            errors: Vec::new(),
            results: Vec::new(),
            skip_error: 0,
            types_map,
        }
    }

    /// Creates a new CSharp session with the provided variables.
    ///
    /// This constructor allows you to initialize the session with a custom set
    /// of variables, such as environment variables or variables loaded from
    /// configuration files.
    ///
    /// # Arguments
    ///
    /// * `variables` - A `Variables` instance containing the initial variable
    ///   set.
    ///
    /// # Returns
    ///
    /// A new `CSharpSession` instance with the provided variables.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cs_parser::{CSharpSession, Variables};
    ///
    /// let env_vars = Variables::env();
    /// let mut session = CSharpSession::new().with_variables(env_vars);
    /// let username = session.safe_eval("$env:USERNAME").unwrap();
    /// ```
    pub fn with_variables(mut self, variables: Variables) -> Self {
        self.variables = variables;
        self
    }

    /// Safely evaluates a CSharp script and returns the output as a string.
    ///
    /// This method parses and evaluates the provided CSharp script,
    /// handling errors gracefully and returning the result as a formatted
    /// string. It's the recommended method for simple script evaluation.
    ///
    /// # Arguments
    ///
    /// * `script` - A string slice containing the CSharp script to evaluate.
    ///
    /// # Returns
    ///
    /// * `Result<String, ParserError>` - The output of the script evaluation,
    ///   or an error if parsing/evaluation fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cs_parser::CSharpSession;
    ///
    /// let mut session = CSharpSession::new();
    ///
    /// // Simple arithmetic
    /// let result = session.safe_eval("1 + 2 * 3").unwrap();
    /// assert_eq!(result, "7");
    ///
    /// // Variable assignment and retrieval
    /// let result = session.safe_eval("$name = 'World'; \"Hello $name\"").unwrap();
    /// assert_eq!(result, "Hello World");
    /// ```
    pub fn safe_eval_statements(&mut self, script: &str) -> Result<PsValue, ParserError> {
        let program_res = self.parse_statements_string_as_program(script)?;
        Ok(program_res.result())
    }

    pub fn deobfuscate_script(&mut self, script: &str) -> Result<String, ParserError> {
        self.push_scope_session();
        let program_res = self.parse_input(script)?;
        self.pop_scope_session();
        Ok(program_res.deobfuscated().to_string())
    }

    pub fn session_variables(&self) -> HashMap<String, PsValue> {
        self.variables
            .get_global()
            .into_iter()
            .map(|(k, v)| (k, v.into()))
            .collect()
    }

    /// Parses and evaluates a CSharp script, returning detailed results.
    ///
    /// This method provides comprehensive information about the parsing and
    /// evaluation process, including the final result, generated output,
    /// any errors encountered, and the tokenized representation of the
    /// script. It's particularly useful for debugging and deobfuscation.
    ///
    /// # Arguments
    ///
    /// * `input` - A string slice containing the CSharp script to parse and
    ///   evaluate.
    ///
    /// # Returns
    ///
    /// * `Result<ScriptResult, ParserError>` - A detailed result containing the
    ///   evaluation outcome, output, errors, and tokens, or a parsing error if
    ///   the script is malformed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cs_parser::CSharpSession;
    ///
    /// let mut session = CSharpSession::new();
    /// let program_result = session.parse_input("$a = 42; Write-Output $a").unwrap();
    ///
    /// println!("Final result: {:?}", program_result.result());
    /// println!("Generated output: {:?}", program_result.output());
    /// println!("Parsing errors: {:?}", program_result.errors());
    /// println!("Deobfuscated code: {:?}", program_result.deobfuscated());
    /// ```
    pub fn parse_input(&mut self, input: &str) -> Result<ProgramResult, ParserError> {
        let input = input.trim();
        let mut pairs = CSharpSession::parse(Rule::program, input)?;
        let program_token = pairs.next().expect("");
        check_rule!(program_token, Rule::program);

        //create new scope for program
        self.results.push(Results::new());
        self.variables.init();

        let program_content_token = program_token.into_inner().next().unwrap();
        self.parse_program_content(program_content_token)?;

        let mut result = self.results.pop().unwrap_or_default();
        Ok(ProgramResult::new(
            Val::Null,
            std::mem::take(&mut result.output),
            std::mem::take(&mut result.deobfuscated),
            std::mem::take(&mut self.tokens),
            std::mem::take(&mut self.errors),
            self.variables
                .get_current_scope()
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
        ))
    }

    pub fn run(&mut self) -> Result<(), ParserError> {
        todo!();
    }

    // pub(crate) fn parse_statements_string(
    //     &mut self,
    //     input: &str,
    // ) -> Result<(Val, Results), ParserError> {
    //     let mut pairs = CSharpSession::parse(Rule::statements, input)?;
    //     let statements_token = pairs.next().expect("");

    //     self.parse_statements_token(statements_token)
    // }

    // pub(crate) fn parse_statement_block_string(
    //     &mut self,
    //     input: &str,
    // ) -> Result<(Val, Results), ParserError> {
    //     let mut pairs = CSharpSession::parse(Rule::statement_block, input)?;
    //     let statement_block_token = pairs.next().expect("");
    //     check_rule!(statement_block_token, Rule::statement_block);

    //     let statements_token =
    // statement_block_token.into_inner().next().unwrap();     check_rule!
    // (statements_token, Rule::statements);

    //     self.parse_statements_token(statements_token)
    // }

    pub(crate) fn parse_statements_string_as_program(
        &mut self,
        input: &str,
    ) -> Result<ProgramResult, ParserError> {
        let mut pairs = CSharpSession::parse(Rule::statements, input)?;
        let statements_token = pairs.next().expect("");

        let (script_last_output, mut result) = self.parse_statements_token(statements_token)?;
        Ok(ProgramResult::new(
            script_last_output,
            std::mem::take(&mut result.output),
            std::mem::take(&mut result.deobfuscated),
            std::mem::take(&mut self.tokens),
            std::mem::take(&mut self.errors),
            self.variables
                .get_current_scope()
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
        ))
    }

    fn parse_statements_token(
        &mut self,
        statements_token: Pair<'a>,
    ) -> Result<(Val, Results), ParserError> {
        check_rule!(statements_token, Rule::statements);
        let pairs = statements_token.into_inner();

        //create new scope for program
        self.results.push(Results::new());

        let mut script_last_output = Val::default();

        self.variables.init();
        for token in pairs {
            let token_str = token.as_str();
            match token.as_rule() {
                Rule::statement_terminator => continue,
                Rule::EOI => break,
                _ => {}
            };
            let (result, flow_control) = self.eval_statement(token.clone());
            if let Ok(Val::NonDisplayed(_)) = &result {
                continue;
            }

            script_last_output = match result {
                Ok(val) => {
                    if val != Val::Null {
                        self.add_output_statement(val.display().into());
                        self.add_deobfuscated_statement(val.cast_to_script());
                    }

                    val
                }
                Err(e) => {
                    self.errors.push(e);
                    self.add_deobfuscated_statement(token_str.into());
                    Val::Null
                }
            };
            if let Some(FlowControl::Break) = flow_control {
                break;
            }
        }

        Ok((script_last_output, self.results.pop().unwrap_or_default()))
    }

    fn parse_program_content(&mut self, token: Pair<'a>) -> ParserResult<()> {
        check_rule!(token, Rule::program_content);
        let pairs = token.into_inner();

        for token in pairs {
            match token.as_rule() {
                Rule::namespace_statement => {
                    let _ = self.parse_namespace_statement(token);
                }
                Rule::class_statement => {
                    let _ = self.parse_class_statement(token);
                }
                Rule::enum_statement => {
                    let _ = self.parse_enum_statement(token);
                }
                Rule::function_declaration => {
                    let _ = self.parse_function_declaration_statement(token);
                }
                Rule::using_statement => continue,
                Rule::EOI => break,
                _ => {}
            }
        }

        Ok(())
    }

    fn parse_namespace_statement(&mut self, token: Pair<'a>) -> ParserResult<()> {
        check_rule!(token, Rule::namespace_statement);
        let mut pair = token.into_inner();
        let namespace_name_token = pair.next().unwrap();
        check_rule!(namespace_name_token, Rule::var_name);

        if let Some(statement_body_token) = pair.next() {
            self.parse_program_content(statement_body_token)?;
        }
        Ok(())
    }

    fn parse_function_declaration_statement(&mut self, token: Pair<'a>) -> ParserResult<()> {
        check_rule!(token, Rule::function_declaration);
        let mut pair = token.into_inner();
        let function_header_token = pair.next().unwrap();
        let _fn_header = self.parse_function_header(function_header_token)?;

        Ok(())
    }

    fn parse_function_header(&mut self, token: Pair<'a>) -> ParserResult<FunctionHeader> {
        check_rule!(token, Rule::function_header);

        let mut pair = token.into_inner();

        let function_header_prefix_token = pair.next().unwrap();
        let (_attributes, is_static) =
            self.parse_function_header_prefix(function_header_prefix_token)?;
        let _return_type_token = pair.next().unwrap();
        check_rule!(_return_type_token, Rule::type_literal);

        let name_token = pair.next().unwrap();
        check_rule!(name_token, Rule::function_name);
        let function_name = name_token.as_str().to_string();

        let params = if let Some(parameters_token) = pair.next() {
            check_rule!(parameters_token, Rule::parameter_list);
            self.parse_parameter_list(parameters_token)?
        } else {
            Vec::new()
        };

        Ok(FunctionHeader::new(function_name, params, is_static))
    }

    fn parse_function_statement(
        &mut self,
        token: Pair<'a>,
    ) -> ParserResult<(FunctionHeader, ScriptBlock)> {
        check_rule!(token, Rule::function_statement);

        let mut pair = token.into_inner();

        let function_header_token = pair.next().unwrap();
        let fn_header = self.parse_function_header(function_header_token)?;

        let function_body_token = pair.next().unwrap();
        let mut script_block = self.parse_statements_block(function_body_token)?;
        script_block = script_block.with_params(fn_header.params().clone());
        Ok((fn_header, script_block))
    }

    pub(crate) fn eval_function_statement(&mut self, token: Pair<'a>) -> ParserResult<()> {
        check_rule!(token, Rule::function_statement);

        let mut pair = token.into_inner();

        let function_header_token = pair.next().unwrap();
        let fn_header = self.parse_function_header(function_header_token)?;

        let function_body_token = pair.next().unwrap();
        let mut script_block = self.parse_statements_block(function_body_token)?;
        script_block = script_block.with_params(fn_header.params().clone());
        self.variables
            .add_global_function(fn_header.name().to_string(), script_block);
        Ok(())
    }

    pub(crate) fn eval_if_statement(&mut self, token: Pair<'a>) -> ParserResult<()> {
        check_rule!(token, Rule::if_statement);

        //collect tokens from each case
        self.if_statement_collect_tokens(token.clone());

        let mut pair = token.into_inner();
        let condition_token = pair.next().unwrap();
        let true_token = pair.next().unwrap();
        let condition_val = self.eval_expression(condition_token.clone())?;
        if condition_val.cast_to_bool() {
            let _ = self.eval_statement_block(true_token);
        } else if let Some(mut token) = pair.next() {
            if token.as_rule() == Rule::elseif_clauses {
                for else_if in token.into_inner() {
                    let mut pairs = else_if.into_inner();
                    let condition_token = pairs.next().unwrap();
                    let statement_token = pairs.next().unwrap();
                    let condition_val = self.eval_expression(condition_token)?;
                    if condition_val.cast_to_bool() {
                        let _ = self.eval_statement_block(statement_token);
                        return Ok(());
                    }
                }
                let Some(token2) = pair.next() else {
                    return Ok(());
                };
                token = token2;
            }
            if token.as_rule() == Rule::else_condition {
                let statement_token = token.into_inner().next().unwrap();
                let _ = self.eval_statement_block(statement_token);
            }
        }

        Ok(())
    }

    pub(crate) fn if_statement_collect_tokens(&mut self, token: Pair<'a>) {
        //we want collect tokens from each case, but we need to preserve all variables
        //to consider: maybe instead of collecting tokens, we should return whole
        // deobfuscated if statement
        let results = self.results.clone();
        let current_variables = self.variables.clone();
        if let Err(err) = self.impl_if_statement_collect_tokens(token.clone()) {
            log::debug!("Error during if_statement_collect_tokens: {:?}", err);
        }
        self.variables = current_variables;
        self.results = results;
    }

    pub(crate) fn impl_if_statement_collect_tokens(&mut self, token: Pair<'a>) -> ParserResult<()> {
        check_rule!(token, Rule::if_statement);

        let mut pair = token.into_inner();
        let condition_token = pair.next().unwrap();
        let true_token = pair.next().unwrap();
        let _condition_val = self.eval_expression(condition_token.clone())?;
        let _ = self.eval_statement_block(true_token);
        if let Some(mut token) = pair.next() {
            if token.as_rule() == Rule::elseif_clauses {
                for else_if in token.into_inner() {
                    let mut pairs = else_if.into_inner();
                    let condition_token = pairs.next().unwrap();
                    let statement_token = pairs.next().unwrap();
                    let _condition_val = self.eval_expression(condition_token)?;

                    let _ = self.eval_statement_block(statement_token);
                }
                let Some(token2) = pair.next() else {
                    return Ok(());
                };
                token = token2;
            }
            if token.as_rule() == Rule::else_condition {
                let statement_token = token.into_inner().next().unwrap();
                let _ = self.eval_statement_block(statement_token);
            }
        }
        Ok(())
    }

    fn eval_flow_control_statement(&mut self, token: Pair<'a>) -> (ParserResult<Val>, FlowControl) {
        check_rule!(token, Rule::flow_control_statement);
        let token = token.into_inner().next().unwrap();

        match token.as_rule() {
            Rule::break_statement => (Ok(Val::Null), FlowControl::Break), //TODO
            Rule::continue_statement => (Ok(Val::Null), FlowControl::Continue), //TODO
            Rule::flow_control_expression_statement => {
                let token = token.into_inner().next().unwrap();
                //todo: throw, return or exit
                let val = if let Some(expression_token) = token.into_inner().next() {
                    self.eval_expression(expression_token)
                } else {
                    Ok(Val::Null)
                };
                (val, FlowControl::Break)
            }
            _ => unexpected_token!(token),
        }
    }

    fn parse_function_header_prefix(
        &mut self,
        token: Pair<'a>,
    ) -> ParserResult<(Vec<Attribute>, bool)> {
        check_rule!(token, Rule::function_header_prefix);
        let mut pair = token.into_inner();
        let mut attributes = Vec::new();
        let mut is_static = false;

        //attributes
        let mut token = pair.next();

        if let Some(first_token) = &token
            && let Rule::attribute_list = first_token.as_rule()
        {
            //we don't care about function attributes for now
            attributes = self.parse_attribute_list(token.unwrap())?;
            token = pair.next();
        }

        if let Some(second_token) = &token
            && let Rule::access_modifier = second_token.as_rule()
        {
            //we don't care about function attributes for now
            //access = Some(self.parse_access_modifier(second_token)?);
            token = pair.next();
        }

        if let Some(third_token) = token
            && let Rule::function_attributes = third_token.as_rule()
        {
            //we don't care about function attributes for now
            for attr in third_token.into_inner() {
                if attr.as_str() == "static" {
                    is_static = true;
                }
            }
        }

        Ok((attributes, is_static))
        // let _access_modifier: Option<String> = if let Rule::access_modifier =
        // token.as_rule() {     //we don't care about class attributes
        // for now     let access_modifier = "unknown".to_string();
        // //self.parse_access_modifier(token)?;     token =
        // pair.next().unwrap();     Some(access_modifier)
        // } else {
        //     None
        // };

        // let _function_attributes: Option<String> =
        //     if let Rule::function_attributes = token.as_rule() {
        //         //we don't care about class attributes for now
        //         let function_attributes = "unknown".to_string();
        // //self.parse_access_modifier(token)?;         token =
        // pair.next().unwrap();         Some(function_attributes)
        //     } else {
        //         None
        //     };

        // Ok(attribute_list)
    }

    fn parse_enum_statement(&mut self, token: Pair<'a>) -> ParserResult<()> {
        check_rule!(token, Rule::enum_statement);
        let mut pair = token.into_inner();

        let fn_header_token = pair.next().unwrap();
        let _attrs = self.parse_function_header_prefix(fn_header_token)?;

        let class_name_token = pair.next().unwrap();
        check_rule!(class_name_token, Rule::simple_name);
        let enum_name = class_name_token.as_str().to_string();

        let mut token = pair.next().unwrap();
        if let Rule::inheritance = token.as_rule() {
            //skip
            token = pair.next().unwrap();
        }

        if let Rule::statement_terminator = token.as_rule() {
            //empty enum
            //todo
            return Ok(());
        }
        check_rule!(token, Rule::class_body);
        let enum_body_pairs = token.into_inner();

        let mut properties = ClassProperties::new();

        for assignment_token in enum_body_pairs {
            let (var_name, variable) = self.parse_enum_assign_exp(assignment_token)?;
            properties.add_property(var_name.name, Some(ValType::Int), Some(variable));
        }
        let class_type = ClassType::new(
            enum_name.clone(),
            Some(properties),
            HashMap::new(),
            HashMap::new(),
        );
        self.types_map
            .insert(enum_name, Box::new(class_type.clone()));

        Ok(())
    }

    fn parse_field_attribute(&mut self, token: Pair<'a>) -> ParserResult<()> {
        check_rule!(token, Rule::field_attribute);
        let mut pairs = token.into_inner();
        if let Some(token) = pairs.next()
            && let Rule::attribute_list = token.as_rule()
        {
            let _attributes = self.parse_attribute_list(token)?;
        }

        Ok(())
    }

    fn parse_field_declaration(
        &mut self,
        token: Pair<'a>,
    ) -> ParserResult<(VarName, Option<ValType>)> {
        check_rule!(token, Rule::field_declaration);
        let mut pair = token.into_inner();

        let field_attribute_token = pair.next().unwrap();
        self.parse_field_attribute(field_attribute_token)?;

        let var_type_token = pair.next().unwrap();
        let ttype = match self.eval_type_literal(var_type_token) {
            Ok(ttype) => Some(ttype),
            Err(e) => {
                log::debug!("Error parsing type literal: {:?}", e);
                None
            }
        };

        let variable_token = pair.next().unwrap();
        check_rule!(variable_token, Rule::variable);
        let var_name = Self::parse_variable(variable_token)?;

        Ok((var_name, ttype))
    }

    fn eval_field_initialization(
        &mut self,
        token: Pair<'a>,
    ) -> ParserResult<(VarName, Option<ValType>, Val)> {
        let mut pair = token.into_inner();

        let field_attribute_token = pair.next().unwrap();
        check_rule!(field_attribute_token, Rule::field_attribute);
        self.parse_field_attribute(field_attribute_token)?;

        let var_type_token = pair.next().unwrap();
        check_rule!(var_type_token, Rule::type_literal);
        let val_type = match self.eval_type_literal(var_type_token) {
            Ok(ttype) => Some(ttype),
            Err(e) => {
                log::debug!("Error parsing type literal: {:?}", e);
                None
            }
        };

        let variable_token = pair.next().unwrap();
        check_rule!(variable_token, Rule::variable);
        let var_name = Self::parse_variable(variable_token)?;

        let expression_token = pair.next().unwrap();
        check_rule!(expression_token, Rule::expression);

        let right_operand = self.eval_expression(expression_token)?;
        if let Some(ttype) = &val_type
            && let Err(e) = right_operand.cast_to_type(ttype)
        {
            log::debug!("Error casting right operand to type {:?}: {:?}", ttype, e);
        }

        Ok((var_name, val_type, right_operand))
    }

    fn parse_enum_assign_exp(&mut self, token: Pair<'a>) -> ParserResult<(VarName, Val)> {
        let mut pair = token.into_inner();

        let variable_token = pair.next().unwrap();
        check_rule!(variable_token, Rule::variable);
        let var_name = Self::parse_variable(variable_token)?;

        let expression_token = pair.next().unwrap();
        check_rule!(expression_token, Rule::expression);

        let right_operand = self.eval_expression(expression_token)?;

        Ok((var_name, Val::Int(right_operand.cast_to_int()?)))
    }

    fn parse_class_statement(&mut self, token: Pair<'a>) -> ParserResult<()> {
        check_rule!(token, Rule::class_statement);
        let mut pair = token.into_inner();

        let token = pair.next().unwrap();
        let _attrs = self.parse_function_header_prefix(token)?;

        let class_name_token = pair.next().unwrap();
        check_rule!(class_name_token, Rule::simple_name);
        let class_name = class_name_token.as_str().to_string();

        let mut token = pair.next().unwrap();
        if let Rule::inheritance = token.as_rule() {
            //skip
            token = pair.next().unwrap();
        }
        if let Rule::statement_terminator = token.as_rule() {
            //empty class
            //todo
            return Ok(());
        }

        check_rule!(token, Rule::class_body);
        let class_body_pairs = token.into_inner();
        let class_body_functions_token = class_body_pairs.clone();

        let properties = ClassProperties::new();
        let mut methods: HashMap<String, ScriptBlock> = HashMap::new();
        let mut static_methods: HashMap<String, ScriptBlock> = HashMap::new();
        for member_token in class_body_functions_token {
            match member_token.as_rule() {
                Rule::function_statement => {
                    let (fn_header, script_block) = self.parse_function_statement(member_token)?;

                    let method_name = MethodName::new(fn_header.name(), fn_header.params());

                    if fn_header.is_static() {
                        static_methods.insert(method_name.full_name().to_string(), script_block);
                    } else {
                        methods.insert(method_name.full_name().to_string(), script_block);
                    }
                }
                _ => continue,
            }
        }
        let mut class_type = ClassType::new(class_name.clone(), None, static_methods, methods);
        self.types_map
            .insert(class_name.clone(), Box::new(class_type.clone()));

        for member_token in class_body_pairs {
            match member_token.as_rule() {
                Rule::field_declaration => {
                    let (var_name, ttype) = self.parse_field_declaration(member_token)?;
                    class_type.add_property(var_name.name, ttype, None);
                }
                Rule::field_initialization => {
                    let (var_name, ttype, default_val) =
                        self.eval_field_initialization(member_token)?;
                    class_type.add_property(
                        var_name.name.clone(),
                        ttype.clone(),
                        Some(default_val),
                    );
                    self.set_this(class_type.clone());
                }
                Rule::function_statement => continue,
                Rule::statement_terminator => continue,
                _ => unexpected_token!(member_token),
            }
        }

        let class_type = class_type.with_properties(properties.clone());
        self.types_map
            .insert(class_name.clone(), Box::new(class_type.clone()));
        Ok(())
    }

    fn eval_statement(&mut self, token: Pair<'a>) -> (ParserResult<Val>, Option<FlowControl>) {
        if let Rule::code_line = token.as_rule() {
            (self.eval_code_line(token), None)
        } else if let Rule::flow_control_statement = token.as_rule() {
            let (res, flow_control) = self.eval_flow_control_statement(token);
            (res, Some(flow_control))
        } else {
            let res = match token.as_rule() {
                Rule::if_statement => self.eval_if_statement(token),
                Rule::labeled_statement => self.parse_labeled_statement(token),
                Rule::function_statement => self.eval_function_statement(token),
                Rule::function_declaration => self.parse_function_declaration_statement(token),
                Rule::class_statement => self.parse_class_statement(token),
                Rule::enum_statement => self.parse_enum_statement(token),
                Rule::try_statement => self.parse_try_statement(token),
                Rule::statement_terminator => Ok(()),
                Rule::EOI => Ok(()),
                _ => {
                    not_implemented!(token)
                }
            };
            (res.map(|_| Val::Null), None)
        }
    }

    fn eval_code_line(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::code_line);
        let inner_token = token.into_inner().next().unwrap();
        match inner_token.as_rule() {
            Rule::assignment_exp => self.eval_assignment_exp(inner_token),
            Rule::field_initialization => {
                let (var_name, _, value) = self.eval_field_initialization(inner_token)?;
                self.set_variable(&var_name, value)
            }
            Rule::field_declaration => {
                let (var_name, _ttype) = self.parse_field_declaration(inner_token)?;
                self.set_variable(&var_name, Val::Null)
            }
            Rule::expression_access => self.eval_expression_access(inner_token),
            Rule::expression => self.eval_expression(inner_token),
            _ => unexpected_token!(inner_token),
        }
    }

    fn parse_try_statement(&mut self, token: Pair<'a>) -> ParserResult<()> {
        check_rule!(token, Rule::try_statement);
        let mut pair = token.into_inner();

        let try_block_token = pair.next().unwrap();
        let _try_block = self.eval_statement_block(try_block_token);

        let Some(mut token) = pair.next() else {
            return Ok(());
        };

        if token.as_rule() == Rule::catch_clauses {
            for catch_token in token.into_inner() {
                let mut pairs = catch_token.into_inner();
                let _catch_type_list_token = pairs.next().unwrap();
                let catch_block_token = pairs.next().unwrap();
                let _catch_block = self.eval_statement_block(catch_block_token);
            }
            let Some(token2) = pair.next() else {
                return Ok(());
            };
            token = token2;
        }
        if token.as_rule() == Rule::finally_clause {
            let finally_block_token = token.into_inner().next().unwrap();
            let _finally_block = self.eval_statement_block(finally_block_token);
        }
        Ok(())
    }

    fn parse_loop_statement(&mut self, token: Pair<'a>) -> ParserResult<()> {
        //check_rule!(token, Rule::for_statement);
        let mut pairs = token.into_inner();
        let _header_token = pairs.next().unwrap();
        let statements_token = pairs.next().unwrap();

        let _ = self.eval_statement_block(statements_token);
        Ok(())
    }

    fn parse_foreach_statement(&mut self, token: Pair<'a>) -> ParserResult<()> {
        //check_rule!(token, Rule::for_statement);
        let mut pairs = token.into_inner();
        let header_token = pairs.next().unwrap();
        check_rule!(header_token, Rule::foreach_header);
        let statements_token = pairs.next().unwrap();

        let (var_name, collection) = self.parse_foreach_header(header_token)?;
        for item in collection {
            self.set_variable(&var_name, item)?;

            let (_, flow_control) = self.eval_statement_block(statements_token.clone());
            if let Some(fc) = flow_control {
                match fc {
                    FlowControl::Break => break,
                    FlowControl::Continue => continue,
                }
            }
        }
        Ok(())
    }

    fn parse_foreach_header(&mut self, token: Pair<'a>) -> ParserResult<(VarName, Vec<Val>)> {
        check_rule!(token, Rule::foreach_header);
        let mut pairs = token.into_inner();
        let _type_token = pairs.next().unwrap();
        let var_token = pairs.next().unwrap();
        check_rule!(var_token, Rule::variable);
        let var_name = Self::parse_variable(var_token)?;

        let collection_token = pairs.next().unwrap();
        check_rule!(collection_token, Rule::expression);
        let collection_val = self.eval_expression(collection_token)?;
        let collection = collection_val.cast_to_array();
        Ok((var_name, collection))
    }

    fn parse_do_while_statement(&mut self, token: Pair<'a>) -> ParserResult<()> {
        check_rule!(token, Rule::do_statement);
        let mut pairs = token.into_inner();
        let statements_token = pairs.next().unwrap();

        let _ = self.eval_statement_block(statements_token);
        Ok(())
    }

    fn parse_switch_statement(&mut self, token: Pair<'a>) -> ParserResult<()> {
        check_rule!(token, Rule::switch_statement);
        let mut pairs = token.into_inner();
        let _switch_condition = pairs.next().unwrap();
        let switch_body = pairs.next().unwrap();
        check_rule!(switch_body, Rule::switch_body);

        let switch_clauses = switch_body.into_inner();
        for clause in switch_clauses {
            let mut pairs = clause.into_inner();
            let _condition_token = pairs.next().unwrap();
            for statement_token in pairs {
                let _ = self.eval_statement(statement_token.clone());
            }
        }
        Ok(())
    }

    fn parse_labeled_statement(&mut self, token: Pair<'a>) -> ParserResult<()> {
        let mut pairs = token.into_inner();
        let mut token = pairs.next().unwrap();
        let _label = if let Rule::label = token.as_rule() {
            let label = token.as_str();
            token = pairs.next().unwrap();
            Some(label)
        } else {
            None
        };

        match token.as_rule() {
            Rule::switch_statement => self.parse_switch_statement(token),
            Rule::foreach_statement => self.parse_foreach_statement(token),
            Rule::for_statement => self.parse_loop_statement(token),
            Rule::while_statement => self.parse_loop_statement(token),
            Rule::do_statement => self.parse_do_while_statement(token),
            _ => unexpected_token!(token),
        }
    }

    fn safe_eval_sub_expr(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::sub_expression);
        let Some(inner_token) = token.into_inner().next() else {
            return Ok(Val::Null);
        };
        let mut inner_val = self.eval_expression(inner_token)?;
        if let Val::ScriptText(script) = &mut inner_val {
            *script = format!("{{{}}}", script);
            //self.tokens.push(Token::SubExpression(script.clone()));
        }
        Ok(inner_val)
    }

    fn eval_statement_block_string(&mut self, input: &str) -> (Val, Option<FlowControl>) {
        let Ok(mut pairs) = CSharpSession::parse(Rule::statement_block, input) else {
            return (Val::Null, None);
        };
        let statement_block_token = pairs.next().expect("");
        check_rule!(statement_block_token, Rule::statement_block);

        self.eval_statement_block(statement_block_token)
    }

    fn eval_statement_block(&mut self, token: Pair<'a>) -> (Val, Option<FlowControl>) {
        check_rule!(token, Rule::statement_block);
        let mut last_statement = Val::Null;
        let statements = token.into_inner().next().unwrap();
        check_rule!(statements, Rule::statements);

        for token in statements.into_inner() {
            let (val, flow_control) = self.eval_statement(token.clone());
            last_statement = match val {
                Ok(s) => s,
                Err(err) => {
                    self.errors.push(err);
                    Val::ScriptText(token.as_str().to_string())
                }
            };
            if let Some(fc) = flow_control {
                return (last_statement, Some(fc));
            }
        }
        (last_statement, None)
    }

    fn parse_dq(&mut self, token: Pair<'a>) -> ParserResult<String> {
        let mut res_str = String::new();
        let pairs = token.into_inner();
        for token in pairs {
            let s = match token.as_rule() {
                Rule::sub_expression => self.safe_eval_sub_expr(token)?.cast_to_string(),
                Rule::backtick_escape => token
                    .as_str()
                    .strip_prefix("`")
                    .unwrap_or_default()
                    .to_string(),
                _ => token.as_str().to_string(),
            };
            res_str.push_str(s.as_str());
        }
        Ok(res_str)
    }

    fn eval_string_literal(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::string_literal);
        let mut pair = token.into_inner();
        let token = pair.next().unwrap();
        let cloned_token = token.clone();

        let mut is_expandable = false;
        let res = match token.as_rule() {
            Rule::simple_string_content | Rule::verbatim_content => token.as_str().to_string(),
            Rule::interpolated_string_literal | Rule::in_verbatim_string_literal => {
                is_expandable = true;
                self.parse_dq(token)?
            }
            Rule::raw_string_literal => token.into_inner().next().unwrap().as_str().to_string(),
            _ => unexpected_token!(token),
        };
        let ps_token = if is_expandable {
            Token::string_expandable(cloned_token.as_str().to_string(), res.clone())
        } else {
            Token::String(res.clone())
        };
        self.tokens.push(ps_token);

        Ok(Val::String(res.into()))
    }

    fn get_variable(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::variable);
        let var_name = Self::parse_variable(token)?;
        let Some(var) = self.variables.get(&var_name, &self.types_map) else {
            return Err(ParserError::VariableError(VariableError::NotDefined(
                var_name.name,
            )));
        };
        Ok(var)
    }

    fn eval_expression_access<'b>(&'b mut self, token: Pair<'b>) -> ParserResult<Val> {
        check_rule!(token, Rule::expression_access);
        fn get_member_name(token: Pair<'_>) -> &'_ str {
            token.into_inner().next().unwrap().as_str()
        }
        let mut pairs = token.into_inner();
        let var_token = pairs.next().unwrap();
        let var_name = Self::parse_variable(var_token)?;

        let mut access_vec = vec![];
        for access_token in pairs {
            let access = match access_token.as_rule() {
                Rule::member_access => Access::Member(get_member_name(access_token)),
                Rule::method_invocation => {
                    let mut pairs = access_token.into_inner();
                    let member_access = pairs.next().unwrap();
                    check_rule!(member_access, Rule::member_access);
                    let member_name = get_member_name(member_access);

                    let argument_list_token = pairs.next().unwrap();
                    check_rule!(argument_list_token, Rule::argument_list);

                    Access::Method(member_name, self.eval_argument_list(argument_list_token)?)
                }
                Rule::argument_list => Access::Function(self.eval_argument_list(access_token)?),
                Rule::element_access => {
                    let mut pairs = access_token.into_inner();
                    let index_token = pairs.next().unwrap();
                    check_rule!(index_token, Rule::expression);
                    Access::Element(self.eval_expression(index_token)?)
                }
                _ => unexpected_token!(access_token),
            };
            access_vec.push(access);
        }

        let mut val_to_return = Val::Null;
        let mut variables_cloned = self.variables.clone();
        {
            let Some(mut variable) = variables_cloned.get_mut(&var_name) else {
                return Err(ParserError::VariableError(VariableError::NotDefined(
                    var_name.name,
                )));
            };

            for access in access_vec {
                variable = match access {
                    Access::Member(name) => variable.member(name)?,
                    Access::Element(index) => variable.get_index_ref(index)?,
                    Access::Function(_index) => continue, //todo!()
                    Access::Method(name, args) => {
                        let mangled_name = MethodName::from_args(name, &args);

                        let mut call = {
                            let owned = variable.clone();
                            owned.method(mangled_name)?
                        };

                        if let Err(err) = call(variable, args, self) {
                            self.errors.push(err.into());
                        }
                        continue;
                    }
                };
            }
            val_to_return = variable.clone();
        }
        self.variables = variables_cloned;

        Ok(val_to_return)
    }

    fn eval_variable_access<'b>(&'b mut self, token: Pair<'b>) -> ParserResult<&'b mut Val> {
        check_rule!(token, Rule::variable_access);
        fn get_member_name(token: Pair<'_>) -> &'_ str {
            token.into_inner().next().unwrap().as_str()
        }
        let mut pairs = token.into_inner();
        let var_token = pairs.next().unwrap();
        let var_name = Self::parse_variable(var_token)?;

        let mut access_vec = vec![];
        for access_token in pairs {
            let access = match access_token.as_rule() {
                Rule::member_access => Access::Member(get_member_name(access_token)),
                Rule::element_access => {
                    let mut pairs = access_token.into_inner();
                    let index_token = pairs.next().unwrap();
                    check_rule!(index_token, Rule::expression);
                    Access::Element(self.eval_expression(index_token)?)
                }
                _ => unexpected_token!(access_token),
            };
            access_vec.push(access);
        }
        let Some(mut variable) = self.variables.get_mut(&var_name) else {
            return Err(ParserError::VariableError(VariableError::NotDefined(
                var_name.name,
            )));
        };

        for access in access_vec {
            variable = match access {
                Access::Member(name) => variable.member(name)?,
                Access::Element(index) => variable.get_index_ref(index)?,
                _ => todo!(),
            };
        }

        Ok(variable)
    }

    fn parse_variable(token: Pair<'a>) -> ParserResult<VarName> {
        check_rule!(token, Rule::variable);
        let mut pair = token.into_inner();
        let token = pair.next().unwrap();
        Ok(VarName::new(None, token.as_str().to_string()))
    }

    fn eval_expression_with_unary_operator(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::expression_with_unary_operator);
        let mut pair = token.into_inner();
        let token = pair.next().unwrap();

        let res = match token.as_rule() {
            Rule::pre_inc_expression => {
                let variable_token = token.into_inner().next().unwrap();
                let var_name = Self::parse_variable(variable_token)?;
                let mut var = self
                    .variables
                    .get(&var_name, &self.types_map)
                    .unwrap_or_default();
                var.inc()?;

                self.variables.set(&var_name, var.clone())?;
                var
            }
            Rule::pre_dec_expression => {
                let variable_token = token.into_inner().next().unwrap();
                let var_name = Self::parse_variable(variable_token)?;
                let mut var = self
                    .variables
                    .get(&var_name, &self.types_map)
                    .unwrap_or_default();
                var.dec()?;

                self.variables.set(&var_name, var.clone())?;
                var
            }
            Rule::cast_expression => self.eval_cast_expression(token)?,
            Rule::negate_op => {
                let unary_token = pair.next().unwrap();
                let unary = self.eval_unary_exp(unary_token)?;
                Val::Bool(!unary.cast_to_bool())
            }
            Rule::bitwise_negate_op => {
                let unary_token = pair.next().unwrap();
                let unary = self.eval_unary_exp(unary_token)?;
                Val::Int(!unary.cast_to_int()?)
            }
            _ => unexpected_token!(token),
        };

        Ok(res)
    }

    fn eval_simple_arg(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::simple_arg);
        let mut pairs = token.into_inner();
        let token = pairs.next().unwrap();
        self.eval_expression(token)
    }

    fn eval_named_arg(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::named_arg);
        let mut pairs = token.into_inner();
        let key_token = pairs.next().unwrap();
        check_rule!(key_token, Rule::var_name);

        let expression_token = pairs.next().unwrap();
        self.eval_expression(expression_token)
    }

    fn eval_argument_list(&mut self, token: Pair<'a>) -> ParserResult<Vec<Val>> {
        check_rule!(token, Rule::argument_list);
        self.skip_error += 1;

        let pairs = token.into_inner();
        let mut args = vec![];
        for token in pairs {
            let arg = match token.as_rule() {
                Rule::named_arg => self.eval_named_arg(token)?,
                Rule::simple_arg => self.eval_simple_arg(token)?,
                _ => unexpected_token!(token),
            };

            args.push(arg);
        }
        self.skip_error -= 1;

        Ok(args)
    }

    fn eval_member_access(&mut self, token: Pair<'a>) -> ParserResult<String> {
        //check_rule!(token, Rule::member_access);
        let member_name_token = token.into_inner().next().unwrap();
        let member_name = member_name_token.as_str().to_string();

        Ok(member_name)
    }

    fn parse_method_invocation(
        &mut self,
        token: Pair<'a>,
        object: &Val,
    ) -> ParserResult<(String, Vec<Val>)> {
        check_rule!(token, Rule::method_invocation);
        let token_string = token.as_str().to_string();

        let mut pairs = token.into_inner();

        let access = pairs.next().unwrap();
        let method_name = self.eval_member_access(access)?;
        let args = if let Some(token) = pairs.next() {
            check_rule!(token, Rule::argument_list);
            match self.eval_argument_list(token) {
                Ok(args) => args,
                Err(e) => {
                    log::debug!("eval_argument_list error: {:?}", e);

                    //nevertheless push the function token
                    self.tokens.push(Token::method(
                        token_string.clone(),
                        object.clone().into(),
                        method_name.clone(),
                        Vec::new(),
                    ));
                    Err(e)?
                }
            }
        } else {
            Vec::new()
        };

        self.tokens.push(Token::method(
            token_string,
            object.clone().into(),
            method_name.clone(),
            args.clone().iter().map(|arg| arg.clone().into()).collect(),
        ));
        Ok((method_name, args))
    }
    fn eval_element_access(&mut self, token: Pair<'a>, object: &Val) -> ParserResult<Val> {
        let mut pairs = token.into_inner();
        let index_token = pairs.next().unwrap();
        check_rule!(index_token, Rule::expression);
        let index = self.eval_expression(index_token)?;
        Ok(object.get_index(index)?)
    }

    fn value_access(&mut self, token: Pair<'a>, object: &mut Val) -> ParserResult<Val> {
        fn get_member_name(token: Pair<'_>) -> &'_ str {
            token.into_inner().next().unwrap().as_str()
        }
        Ok(match token.as_rule() {
            Rule::argument_list => {
                let raw_args_token = token.as_str();
                let fn_name = object.cast_to_string();
                let args = self.eval_argument_list(token)?;
                self.tokens.push(Token::function(
                    format!("{}{}", fn_name.clone(), raw_args_token),
                    fn_name.clone(),
                    args.iter()
                        .map(|arg| arg.clone().cast_to_string())
                        .collect(),
                ));
                if let Some(call) = self.variables.get_function(&fn_name) {
                    call(args, self)?
                } else {
                    return Err(RuntimeError::MemberNotFound(fn_name).into());
                }
            }
            Rule::member_access => match object {
                Val::RuntimeType(rt) => rt.readonly_static_member(get_member_name(token))?,
                _ => object.readonly_member(get_member_name(token))?,
            },
            Rule::method_invocation => {
                let (function_name, args) = self.parse_method_invocation(token, object)?;
                let mangled_name = MethodName::from_args(function_name.as_str(), &args);

                match object {
                    Val::RuntimeType(rt) => {
                        let mut call = rt.static_method(mangled_name)?;
                        call(args, self)?
                    }
                    _ => {
                        // Clone the value to get the method closure without borrowing `object`
                        let mut call = {
                            let owned = object.clone();
                            owned.method(mangled_name)?
                        };
                        call(object, args, self)?
                    }
                }
            }
            Rule::element_access => self.eval_element_access(token, object)?,
            _ => unexpected_token!(token),
        })
    }

    fn eval_constructor(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::constructor);
        let mut pairs = token.into_inner();
        let type_token = pairs.next().unwrap();
        check_rule!(type_token, Rule::type_literal);
        let ttype = self.eval_type_literal(type_token)?;

        let args = if let Some(token) = pairs.next() {
            check_rule!(token, Rule::argument_list);
            self.eval_argument_list(token)?
        } else {
            Vec::new()
        };
        let new_object = ttype.init(args, self);
        let _args = if let Some(token) = pairs.next() {
            check_rule!(token, Rule::initializer_list);
            self.eval_initializer_list(token)?
        } else {
            Vec::new()
        };

        //let new_object = ttype.init(args);
        new_object
    }

    fn eval_initializer_list(&mut self, _token: Pair<'a>) -> ParserResult<Vec<Val>> {
        check_rule!(_token, Rule::initializer_list);
        //todo
        Ok(Vec::new())
    }

    fn eval_list_initialization(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::list_initialization);
        let mut pairs = token.into_inner();
        let mut token = pairs.next().unwrap();
        let ttype = if let Rule::type_literal = token.as_rule() {
            let ttype = self.eval_type_literal(token)?;
            token = pairs.next().unwrap();
            Some(ttype)
        } else {
            None
        };

        let args = match token.as_rule() {
            Rule::initializer_list => self.eval_initializer_list(token)?,
            Rule::argument_list => self.eval_argument_list(token)?,
            _ => unexpected_token!(token),
        };
        if let Some(ttype) = ttype {
            ttype.init(args, self)
        } else {
            ValType::Array(None).init(args, self)
        }
    }

    fn eval_new_expression(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::new_expression);
        let mut pairs = token.into_inner();
        let token = pairs.next().unwrap();
        match token.as_rule() {
            Rule::list_initialization => self.eval_list_initialization(token),
            Rule::constructor => self.eval_constructor(token),
            _ => unexpected_token!(token),
        }
    }

    fn eval_value_access(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::value_access);
        let mut pairs = token.into_inner();
        let token = pairs.next().unwrap();

        let mut object = self.eval_value(token)?;

        for token in pairs {
            object = self.value_access(token, &mut object)?;
        }
        Ok(object.clone())
    }

    fn parse_access(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::value_access);
        let value_access_token_string = token.as_str().to_string();
        let mut pairs = token.into_inner();
        let value_token = pairs.next().unwrap();
        check_rule!(value_token, Rule::value);

        let mut object = self
            .eval_value(value_token.clone())
            .map(|v| v.cast_to_script())
            .unwrap_or(value_token.as_str().to_string());

        for token in pairs {
            match token.as_rule() {
                Rule::argument_list => {
                    let args = self.eval_argument_list(token)?;
                    self.tokens.push(Token::function(
                        value_access_token_string.clone(),
                        object.clone(),
                        args.clone()
                            .into_iter()
                            .map(|arg| arg.cast_to_string())
                            .collect(),
                    ));

                    object = format!(
                        "{}({})",
                        value_token.as_str(),
                        args.iter()
                            .map(|arg| arg.cast_to_script())
                            .collect::<Vec<String>>()
                            .join(", ")
                    )
                }
                Rule::member_access => {
                    object.push_str(token.as_str());
                }
                Rule::method_invocation => {
                    let (method_name, args) = self
                        .parse_method_invocation(token.clone(), &Val::ScriptText(object.clone()))?;

                    object = format!(
                        "{}.{}({})",
                        object,
                        method_name,
                        args.iter()
                            .map(|arg| arg.cast_to_script())
                            .collect::<Vec<String>>()
                            .join(", ")
                    )
                }
                Rule::element_access => {
                    let mut pairs = token.into_inner();
                    let index_token = pairs.next().unwrap();
                    check_rule!(index_token, Rule::expression);
                    let index = self.eval_expression(index_token)?;
                    object = format!("{}[{}]", object, index);
                }
                _ => unexpected_token!(token),
            }
        }
        Ok(Val::String(object.into()))
    }

    fn eval_primary_expression(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::primary_expression);
        let mut pair = token.into_inner();
        let token = pair.next().unwrap();

        let res = match token.as_rule() {
            Rule::new_expression => self.eval_new_expression(token.clone())?,
            Rule::value_access => match self.eval_value_access(token.clone()) {
                Ok(res) => res,
                Err(err) => {
                    log::debug!("eval_access error: {:?}", err);
                    self.errors.push(err);
                    self.parse_access(token)?
                }
            },
            Rule::value => self.eval_value(token)?,
            Rule::post_inc_expression => {
                let variable_token = token.into_inner().next().unwrap();
                let var_name = Self::parse_variable(variable_token)?;
                let mut var = self
                    .variables
                    .get(&var_name, &self.types_map)
                    .unwrap_or_default();
                let var_to_return = var.clone();

                var.inc()?;
                self.variables.set(&var_name, var.clone())?;

                //if var_to_return.ttype() ==
                var_to_return
            }
            Rule::post_dec_expression => {
                let variable_token = token.into_inner().next().unwrap();
                let var_name = Self::parse_variable(variable_token)?;
                let mut var = self
                    .variables
                    .get(&var_name, &self.types_map)
                    .unwrap_or_default();
                let var_to_return = var.clone();

                var.dec()?;
                self.variables.set(&var_name, var.clone())?;

                var_to_return
            }
            _ => unexpected_token!(token),
        };

        Ok(res)
    }

    fn eval_dimensions(&mut self, token: Pair<'a>) -> ParserResult<Vec<usize>> {
        check_rule!(token, Rule::dimensions);
        let mut dimensions = Vec::new();
        for dim_token in token.into_inner() {
            check_rule!(dim_token, Rule::dimension);
            let size_token = dim_token.into_inner().next().unwrap();
            let size = self.eval_decimal_integer(size_token)?.cast_to_int()? as usize;
            dimensions.push(size);
        }
        Ok(dimensions)
    }

    fn eval_type_literal(&mut self, token: Pair<'a>) -> ParserResult<ValType> {
        check_rule!(token, Rule::type_literal);
        let mut pairs = token.into_inner();
        let token = pairs.next().unwrap();

        match token.as_rule() {
            Rule::dimension => {
                let dimensions = self.eval_dimensions(token)?;
                let mut val_type = ValType::cast("object", &self.types_map)?;
                for _dim in &dimensions {
                    val_type = ValType::Array(Some(Box::new(val_type)));
                }
                Ok(val_type)
            }
            Rule::type_name => {
                let type_name = token.as_str();
                let mut opt_token = pairs.next();
                let (_generic, dimensions) = if let Some(next_token) = &opt_token {
                    let generic = if next_token.as_rule() == Rule::generic_type {
                        //todo: eval generic args
                        opt_token = pairs.next();
                        Some(1)
                    } else {
                        None
                    };

                    let dimensions = if let Some(next_token) = opt_token {
                        check_rule!(token, Rule::dimensions);
                        self.eval_dimensions(next_token)?
                    } else {
                        Vec::new()
                    };
                    (generic, dimensions)
                } else {
                    (None, Vec::new())
                };
                let mut val_type = ValType::cast(type_name, &self.types_map)?;
                for _dim in &dimensions {
                    val_type = ValType::Array(Some(Box::new(val_type)));
                }
                Ok(val_type)
            }
            _ => unexpected_token!(token),
        }
    }

    fn eval_cast_literal(&mut self, token: Pair<'a>) -> ParserResult<ValType> {
        check_rule!(token, Rule::cast_literal);

        let token = token.into_inner().next().unwrap();
        check_rule!(token, Rule::type_literal);
        self.eval_type_literal(token)
    }

    fn parse_statements_block(&mut self, token: Pair<'a>) -> ParserResult<ScriptBlock> {
        check_rule!(token, Rule::statement_block);

        let body = token.as_str().to_string();

        //todo is it necessary?
        // Ok(if let Ok(deobfuscated_body) =
        // self.deobfuscate_script(&script_body) {
        //     ScriptBlock::new(params, deobfuscated_body.clone(),
        // format!("{};{}", params_str, deobfuscated_body)) } else {
        //     ScriptBlock::new(params, script_body, raw_text)
        // })
        self.statements_block_collect_tokens(token);

        Ok(ScriptBlock::new(body))
    }

    pub(crate) fn statements_block_collect_tokens(&mut self, token: Pair<'a>) {
        //we want collect tokens from each case, but we need to preserve all variables
        //to consider: maybe instead of collecting tokens, we should return whole
        // deobfuscated if statement
        let statements_token = token.into_inner().next().unwrap();

        let results = self.results.clone();
        let current_variables = self.variables.clone();
        let errors = self.errors.clone();
        if let Err(err) = self.parse_statements_token(statements_token) {
            log::debug!("Error during statements_block collect_tokens: {:?}", err);
        }
        self.variables = current_variables;
        self.results = results;
        self.errors = errors;
    }

    fn eval_hash_key(&mut self, token: Pair<'a>) -> ParserResult<String> {
        check_rule!(token, Rule::key_expression);
        let mut pairs = token.into_inner();
        let key_token = pairs.next().unwrap();

        Ok(match key_token.as_rule() {
            Rule::simple_name => key_token.as_str().to_string(),
            Rule::unary_exp => self.eval_unary_exp(key_token)?.cast_to_string(),
            _ => unexpected_token!(key_token),
        })
    }

    fn eval_hash_entry(&mut self, token: Pair<'a>) -> ParserResult<(String, Val)> {
        check_rule!(token, Rule::hash_entry);

        let mut pairs = token.into_inner();
        let token_key = pairs.next().unwrap();
        let token_value = pairs.next().unwrap();
        let value = self.eval_expression(token_value)?;

        Ok((self.eval_hash_key(token_key)?, value))
    }

    fn eval_hash_literal(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::hash_literal_expression);
        let pairs = token.into_inner();
        let mut hash = HashMap::new();
        for token in pairs {
            let (key, value) = self.eval_hash_entry(token)?;
            hash.insert(key, value);
        }
        Ok(Val::HashTable(hash))
    }

    fn get_predefined_variable(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::predefined_variable);
        let token = token.into_inner().next().unwrap();
        match token.as_rule() {
            Rule::true_literal => Ok(Val::Bool(true)),
            Rule::false_literal => Ok(Val::Bool(false)),
            Rule::null_literal => Ok(Val::Null),
            _ => unexpected_token!(token),
        }
    }

    fn eval_value(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::value);
        let mut pair = token.into_inner();
        let token = pair.next().unwrap();

        let res = match token.as_rule() {
            Rule::parenthesized_expression => self.eval_parenthesized_expression(token)?,
            Rule::sub_expression => self.safe_eval_sub_expr(token)?,
            Rule::hash_literal_expression => self.eval_hash_literal(token)?,
            Rule::string_literal => self.eval_string_literal(token)?,
            Rule::number_literal => self.eval_number_literal(token)?,
            Rule::char_literal => {
                Val::Char(token.as_str().chars().nth(1).unwrap_or_default() as u32)
            }
            Rule::predefined_variable => self.get_predefined_variable(token)?,
            Rule::variable => self.get_variable(token)?,
            _ => unexpected_token!(token),
        };
        log::debug!("eval_value - res: {:?}", res);
        Ok(res)
    }

    fn eval_number_literal(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::number_literal);
        let mut negate = false;
        let mut pairs = token.into_inner();
        let mut token = pairs.next().unwrap();

        //first handle prefix sign: + or -
        if token.as_rule() == Rule::minus {
            negate = true;
            token = pairs.next().unwrap();
        } else if token.as_rule() == Rule::plus {
            token = pairs.next().unwrap();
        }

        let mut val = self.eval_number(token)?;

        if negate {
            val.neg()?;
        }

        if let Some(unit) = pairs.next() {
            let unit = unit.as_str().to_ascii_lowercase();
            let unit_int = match unit.as_str() {
                "k" => 1024,
                "m" => 1024 * 1024,
                "g" => 1024 * 1024 * 1024,
                "t" => 1024 * 1024 * 1024 * 1024,
                "p" => 1024 * 1024 * 1024 * 1024 * 1024,
                _ => 1,
            };
            val.mul(Val::Int(unit_int))?;
        }
        Ok(val)
    }

    fn eval_decimal_integer(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::decimal_integer);
        let int_val = token.into_inner().next().unwrap();
        Ok(Val::Int(int_val.as_str().parse::<i64>().unwrap()))
    }

    fn eval_number(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::number);
        let mut pairs = token.into_inner();
        let token = pairs.next().unwrap();
        let v = match token.as_rule() {
            Rule::decimal_integer => Val::Int(token.as_str().parse::<i64>().unwrap_or_default()),
            Rule::hex_integer => {
                let int_val = token.into_inner().next().unwrap();
                Val::Int(i64::from_str_radix(int_val.as_str(), 16).unwrap())
            }
            Rule::float => Val::Float(token.as_str().trim().parse::<f64>()?),
            _ => unexpected_token!(token),
        };
        Ok(v)
    }

    fn eval_unary_exp(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::unary_exp);
        let token = token.into_inner().next().unwrap();
        match token.as_rule() {
            Rule::expression_with_unary_operator => self.eval_expression_with_unary_operator(token),
            Rule::primary_expression => self.eval_primary_expression(token),
            _ => unexpected_token!(token),
        }
    }

    fn eval_range_exp(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        fn range(mut left: i64, right: i64) -> Vec<Val> {
            let mut v = Vec::new();
            if left <= right {
                loop {
                    v.push(left);
                    if left == right {
                        break;
                    }
                    left += 1;
                }
            } else {
                loop {
                    v.push(left);
                    if left == right {
                        break;
                    }
                    left -= 1;
                }
            }
            v.into_iter().map(Val::Int).collect()
        }
        check_rule!(token, Rule::range_exp);
        let mut pairs = token.into_inner();
        let token = pairs.next().unwrap();
        let left = self.eval_unary_exp(token)?;
        if let Some(token) = pairs.next() {
            check_rule!(token, Rule::unary_exp);
            let left = left.cast_to_int()?;
            let right = self.eval_unary_exp(token)?.cast_to_int()?;
            Ok(Val::Array(range(left, right)))
        } else {
            Ok(left)
        }
    }

    fn eval_format_impl(&mut self, format: Val, mut pairs: Pairs<'a>) -> ParserResult<Val> {
        fn format_with_vec(fmt: &str, args: Vec<Val>) -> ParserResult<String> {
            fn strange_special_case(fmt: &str, n: i64) -> String {
                fn split_digits(n: i64) -> Vec<u8> {
                    n.abs() // ignore sign for digit splitting
                        .to_string()
                        .chars()
                        .filter_map(|c| c.to_digit(10).map(|opt| opt as u8))
                        .collect()
                }

                //"{0:31sdfg,0100a0b00}" -f 578 evals to 310100a5b78
                let mut digits = split_digits(n);
                digits.reverse();
                let mut fmt_vec = fmt.as_bytes().to_vec();
                fmt_vec.reverse();

                let mut i = 0;
                for digit in digits {
                    while i < fmt_vec.len() {
                        if fmt_vec[i] != b'0' {
                            i += 1
                        } else {
                            fmt_vec[i] = digit + b'0';
                            break;
                        }
                    }
                }
                fmt_vec.reverse();
                String::from_utf8(fmt_vec).unwrap_or_default()
            }

            let mut output = String::new();
            let mut i = 0;

            while i < fmt.len() {
                if fmt[i..].starts_with('{') {
                    if let Some(end) = fmt[i..].find('}') {
                        let token = &fmt[i + 1..i + end];
                        let formatted = if token.contains(':') {
                            let mut parts = token.split(':');
                            let index: usize = if let Some(p) = parts.next() {
                                p.parse().unwrap_or(0)
                            } else {
                                0
                            };

                            let spec = parts.next();
                            match args.get(index) {
                                Some(val) => match spec {
                                    Some(s) if s.starts_with('N') => {
                                        let precision = s[1..].parse::<usize>().unwrap_or(2);
                                        if let Ok(f) = val.cast_to_float() {
                                            format!("{:.1$}", f, precision)
                                        } else {
                                            val.cast_to_string().to_string()
                                        }
                                    }
                                    Some(s) => strange_special_case(s, val.cast_to_int()?),
                                    None => val.cast_to_string().to_string(),
                                },
                                None => format!("{{{}}}", token), /* leave as-is if index out of
                                                                   * bounds */
                            }
                        } else if token.contains(',') {
                            let mut parts = token.split(',');
                            let index: usize = parts.next().unwrap().parse().unwrap_or(0);
                            let spec = parts.next();
                            match args.get(index) {
                                Some(val) => match spec {
                                    Some(s) => {
                                        let spaces = s.parse::<usize>().unwrap_or(0);
                                        let spaces_str = " ".repeat(spaces);
                                        format!("{spaces_str}{}", val.cast_to_string())
                                    }
                                    _ => val.cast_to_string().to_string(),
                                },
                                None => format!("{{{}}}", token), /* leave as-is if index out of
                                                                   * bounds */
                            }
                        } else {
                            let index: usize =
                                Val::String(token.to_string().into()).cast_to_int()? as usize;
                            match args.get(index) {
                                Some(val) => val.cast_to_string().to_string(),
                                None => format!("{{{}}}", token), /* leave as-is if index out of
                                                                   * bounds */
                            }
                        };

                        output.push_str(&formatted);
                        i += end + 1;
                    } else {
                        output.push('{');
                        i += 1;
                    }
                } else {
                    output.push(fmt[i..].chars().next().unwrap());
                    i += 1;
                }
            }

            Ok(output)
        }

        Ok(if let Some(token) = pairs.next() {
            let first_fmt = format.cast_to_string();

            let second_fmt = self.eval_range_exp(token)?;
            let res = self.eval_format_impl(second_fmt, pairs)?;
            Val::String(format_with_vec(first_fmt.as_str(), res.cast_to_array())?.into())
        } else {
            format
        })
    }

    fn eval_format_exp(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::format_exp);
        let mut pairs = token.into_inner();
        let format = self.eval_range_exp(pairs.next().unwrap())?;
        self.eval_format_impl(format, pairs)
    }

    fn eval_mult(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::multiplicative_exp);
        let mut pairs = token.into_inner();
        let mut res = self.eval_format_exp(pairs.next().unwrap())?;
        while let Some(op) = pairs.next() {
            let Some(fun) = ArithmeticPred::get(op.as_str()) else {
                log::error!("No arithmetic function for operator: {}", op.as_str());
                return Err(ParserError::NotImplemented(format!(
                    "No arithmetic function for operator: {}",
                    op.as_str()
                )));
            };

            let postfix = pairs.next().unwrap();
            let right_op = self.eval_format_exp(postfix)?;
            res = fun(res, right_op)?;
        }

        Ok(res)
    }

    fn eval_additive(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::additive_exp);

        let mut pairs = token.into_inner();
        let mut res = self.eval_mult(pairs.next().unwrap())?;
        while let Some(op) = pairs.next() {
            //check_rule!(op, Rule::additive_op); plus or minus
            let Some(fun) = ArithmeticPred::get(op.as_str()) else {
                log::error!("No arithmetic function for operator: {}", op.as_str());
                return Err(ParserError::NotImplemented(format!(
                    "No arithmetic function for operator: {}",
                    op.as_str()
                )));
            };

            let mult = pairs.next().unwrap();
            let right_op = self.eval_mult(mult)?;
            res = fun(res, right_op)?;
        }

        Ok(res)
    }

    fn eval_comparison_exp(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::comparison_exp);
        let mut pairs = token.into_inner();
        let token = pairs.next().unwrap();

        // we need to handle strange case. -split and -join can be invoke without
        // previous expression, eg. "-join 'some'"
        let mut res = if token.as_rule() == Rule::additive_exp {
            self.eval_additive(token)?
        } else {
            Val::Null
        };

        while let Some(op) = pairs.next() {
            let Some(fun) = StringPred::get(op.as_str()) else {
                log::error!("No string predicate for operator: {}", op.as_str());
                return Err(ParserError::NotImplemented(format!(
                    "No string predicate for operator: {}",
                    op.as_str()
                )));
            };

            let token = pairs.next().unwrap();
            let right_op = self.eval_additive(token)?;

            log::trace!("res: {:?}, right_op: {:?}", &res, &right_op);
            res = fun(res, right_op)?;
            log::trace!("res: {:?}", &res);
        }

        Ok(res)
    }

    fn parse_parameter_list(&mut self, token: Pair<'a>) -> ParserResult<Vec<Param>> {
        check_rule!(token, Rule::parameter_list);
        let mut params = vec![];
        let param_list_pairs = token.into_inner();
        for fn_parameter_token in param_list_pairs {
            check_rule!(fn_parameter_token, Rule::fn_parameter);
            params.push(self.parse_fn_parameter(fn_parameter_token)?);
        }
        Ok(params)
    }

    fn parse_attribute_list(&mut self, token: Pair<'a>) -> ParserResult<Vec<Attribute>> {
        check_rule!(token, Rule::attribute_list);
        let attribute_list_pairs = token.into_inner();
        let mut attributes = vec![];
        for attribute_token in attribute_list_pairs {
            check_rule!(attribute_token, Rule::attribute);
            let attribute = self.parse_attribute(attribute_token)?;
            attributes.push(attribute);
        }
        Ok(attributes)
    }

    fn eval_member_chain(&mut self, token: Pair<'a>) -> ParserResult<String> {
        check_rule!(token, Rule::member_chain);
        Ok(token.as_str().to_string())
    }

    fn parse_attribute(&mut self, token: Pair<'a>) -> ParserResult<Attribute> {
        check_rule!(token, Rule::attribute);
        let mut pairs = token.into_inner();
        let attribute_name = self.eval_member_chain(pairs.next().unwrap())?;
        let mut attr_args = vec![];
        let Some(attribute_arguments_token) = pairs.next() else {
            return Ok(Attribute::new(attribute_name, attr_args));
        };

        for attribute_argument in attribute_arguments_token.into_inner() {
            let attribute_arg = self.parse_attribute_argument(attribute_argument)?;
            attr_args.push(attribute_arg);
        }
        self.tokens.push(Token::attribute(
            attribute_name.clone(),
            attr_args
                .iter()
                .map(|arg| {
                    (
                        arg.key().clone(),
                        arg.default_value().clone().unwrap_or_default().into(),
                    )
                })
                .collect::<Vec<(String, PsValue)>>(),
        ));
        Ok(Attribute::new(attribute_name, attr_args))
    }

    fn parse_attribute_argument(&mut self, token: Pair<'a>) -> ParserResult<AttributeArg> {
        check_rule!(token, Rule::attribute_argument);
        let mut pairs = token.into_inner();
        let key_name = self.eval_expression(pairs.next().unwrap())?;
        let default_value = if let Some(attribute_argument) = pairs.next() {
            check_rule!(attribute_argument, Rule::expression);
            Some(self.eval_expression(attribute_argument)?)
        } else {
            None
        };
        Ok(AttributeArg::new(key_name.cast_to_string(), default_value))
    }

    fn parse_fn_parameter(&mut self, token: Pair<'a>) -> ParserResult<Param> {
        check_rule!(token, Rule::fn_parameter);
        let mut pairs = token.into_inner();
        let mut token = pairs.next().unwrap();

        let _attributes = if token.as_rule() == Rule::attribute_list {
            let attrs = self.parse_attribute_list(token)?;
            //log::trace!("Parsed fn_parameter attributes: {:?}", &attrs);
            token = pairs.next().unwrap();
            attrs
        } else {
            vec![]
        };

        let type_literal = self.eval_type_literal(token)?;

        let var_name_token = pairs.next().unwrap();
        check_rule!(var_name_token, Rule::variable);
        let var_name = Self::parse_variable(var_name_token)?;

        let default_value = if let Some(default_value_token) = pairs.next() {
            check_rule!(default_value_token, Rule::fn_parameter_default);
            let default_value_expr = default_value_token.into_inner().next().unwrap();
            let default_value = self.eval_primary_expression(default_value_expr)?;
            Some(default_value)
        } else {
            None
        };
        Ok(Param::new(type_literal, var_name.name, default_value))
    }

    fn eval_bitwise_exp(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::bitwise_exp);

        let mut pairs = token.into_inner();
        let mut res = self.eval_as_exp(pairs.next().unwrap())?;
        while let Some(op) = pairs.next() {
            check_rule!(op, Rule::bitwise_operator);
            let Some(fun) = BitwisePred::get(op.as_str()) else {
                log::error!("No bitwise predicate for operator: {}", op.as_str());
                return Err(ParserError::NotImplemented(format!(
                    "No bitwise predicate for operator: {}",
                    op.as_str()
                )));
            };

            let mult = pairs.next().unwrap();
            let right_op = self.eval_as_exp(mult)?;
            res = fun(res, right_op)?;
        }

        Ok(res)
    }

    fn eval_as_exp(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::as_expression);

        let mut pairs = token.into_inner();
        let mut res = self.eval_comparison_exp(pairs.next().unwrap())?;
        for token in pairs {
            let runtime_object = match token.as_rule() {
                Rule::type_literal => self.eval_type_literal(token)?,
                Rule::comparison_exp => self.eval_comparison_exp(token)?.ttype(),
                _ => unexpected_token!(token),
            };

            res = res.cast_to_type(&runtime_object).unwrap_or_default();
        }

        Ok(res)
    }

    fn add_deobfuscated_statement(&mut self, msg: String) {
        if let Some(last) = self.results.last_mut() {
            last.deobfuscated.push(msg);
        }
    }

    fn add_output_statement(&mut self, msg: StreamMessage) {
        if let Some(last) = self.results.last_mut() {
            last.output.push(msg);
        }
    }

    fn eval_expression(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::expression);
        let token_string = token.as_str().trim().to_string();

        let mut pairs = token.into_inner();
        let mut res = self.eval_bitwise_exp(pairs.next().unwrap())?;
        while let Some(op) = pairs.next() {
            check_rule!(op, Rule::logical_operator);
            let Some(fun) = LogicalPred::get(op.as_str()) else {
                log::error!("No logical predicate for operator: {}", op.as_str());
                return Err(ParserError::NotImplemented(format!(
                    "No logical predicate for operator: {}",
                    op.as_str()
                )));
            };

            let mult = pairs.next().unwrap();
            let right_op = self.eval_bitwise_exp(mult)?;
            res = Val::Bool(fun(res, right_op));
        }
        self.tokens
            .push(Token::expression(token_string, res.clone().into()));

        if let Val::String(value::PsString(s)) = &res {
            self.tokens.push(Token::String(s.clone()));
        }

        Ok(res)
    }

    fn eval_cast_expression(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::cast_expression);

        let mut pairs = token.into_inner();
        let type_token = pairs.next().unwrap();
        check_rule!(type_token, Rule::cast_literal);
        let val_type = self.eval_cast_literal(type_token)?;
        let token = pairs.next().unwrap();
        let res = match token.as_rule() {
            Rule::parenthesized_expression => self.eval_parenthesized_expression(token)?,
            Rule::unary_exp => self.eval_unary_exp(token)?,
            _ => unexpected_token!(token),
        };
        Ok(res.cast_to_type(&val_type)?)
    }

    fn eval_parenthesized_expression(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::parenthesized_expression);
        let inner = token.into_inner().next().unwrap();
        self.eval_expression(inner)
    }

    fn eval_assignment_exp(&mut self, token: Pair<'a>) -> ParserResult<Val> {
        check_rule!(token, Rule::assignment_exp);

        let mut pairs = token.into_inner();
        let assign_var_token = pairs.next().unwrap();
        check_rule!(assign_var_token, Rule::variable_access);

        let assignment_op = pairs.next().unwrap();
        check_rule!(assignment_op, Rule::modify_and_assign_op);

        //get operand
        let op = assignment_op.into_inner().next().unwrap();
        let pred = ArithmeticPred::get(op.as_str());

        let right_token = pairs.next().unwrap();
        check_rule!(right_token, Rule::expression);

        let right_op = self.eval_expression(right_token.clone())?;

        // if variable is being initialized, we don't need to eval it
        let variable = self.eval_variable_access(assign_var_token)?;
        let accessed_elem = variable;

        let Some(pred) = pred else {
            log::error!("No arithmetic function for operator: {}", op.as_str());
            return Err(ParserError::NotImplemented(format!(
                "No arithmetic function for operator: {}",
                op.as_str()
            )));
        };

        *accessed_elem = pred(accessed_elem.clone(), right_op)?;
        // println!("Assigning variable: {} = {:?}", var_name, variable);
        // Ok((var_name, variable))
        Ok(accessed_elem.clone())
    }

    fn set_variable(&mut self, var_name: &VarName, value: Val) -> ParserResult<Val> {
        self.variables.set(var_name, value.clone())?;
        //we want save each assignment statement
        self.add_deobfuscated_statement(format!("{} = {}", var_name, value.cast_to_script()));

        Ok(Val::NonDisplayed(Box::new(value)))
    }

    fn push_scope_session(&mut self) {
        self.variables.push_scope_session();
    }

    fn pop_scope_session(&mut self) {
        self.variables.pop_scope_session();
    }
}

#[cfg(test)]
mod tests {
    use pest::Parser;

    use super::*;

    #[test]
    fn comment_and_semicolon() {
        let input = r#"
// This is a single line comment
var a = 1;
int b = 2; 
Console.WriteLine(a);

Console.WriteLine("Hello");  // Another comment

/*
    This is a
    multi-line block comment
*/
"#;

        let _ = CSharpSession::parse(Rule::statements, input).unwrap();
    }

    #[test]
    fn while_loop() {
        let input = r#"
while (true) {
    if (5 > 6) {
        break;
    }
    // other code
}
"#;

        let _ = CSharpSession::parse(Rule::statements, input).unwrap();
    }

    #[test]
    fn foreach_loop() {
        let input = r#"
foreach (int n in $numbers) {
    Console.WriteLine(n);
}
"#;

        let _ = CSharpSession::parse(Rule::statements, input).unwrap();
    }

    #[test]
    fn for_loop() {
        let input = r#"
// Comma separated assignment expressions enclosed in parentheses.
for (int i = 0; i < 5; i++) 
{
  Console.WriteLine(i);
}
"#;

        let _ = CSharpSession::parse(Rule::statements, input).unwrap();
    }

    #[test]
    fn switch() {
        let input = r#"
switch(expression) 
{
  case x:
    // code block
    break;
  case y:
    // code block
    break;
  default:
    // code block
    break;
}
"#;

        let _ = CSharpSession::parse(Rule::statements, input).unwrap();
    }

    #[test]
    fn functions() {
        let input = r#"
class Program
{
  static int MyMethod(string[] fname, int age) 
  {
    return age;
  }
  static void Main(string[] args)
  {
    MyMethod(child3: "John", child1: "Liam", child2: "Liam");
  }
}
"#;

        let _ = CSharpSession::parse(Rule::program, input).unwrap();
    }

    #[test]
    fn if_expression() {
        let input = r#"
int time = 22;
if (time < 10) 
{
  Console.WriteLine("Good morning.");
} 
else if (time < 20) 
{
  Console.WriteLine("Good day.");
} 
else 
{
  Console.WriteLine("Good evening.");
}
"#;

        let _ = CSharpSession::parse(Rule::statements, input).unwrap();
    }

    #[test]
    fn range() {
        let input = r#"
string s = "abcdef";
string part = s[1..4];   // "bcd"
"#;

        let _ = CSharpSession::parse(Rule::statements, input).unwrap();
    }

    #[test]
    fn literals() {
        let input = r#"
int myNum = 5;               // Integer (whole number)
double myDoubleNum = 5.99D;  // Floating point number
char myLetter = 'D';         // Character
bool myBool = true;          // Boolean
string myText = "Hello";     // String
"#;

        let _ = CSharpSession::parse(Rule::statements, input).unwrap();
    }

    #[test]
    fn arrays() {
        let input = r#"
string[] cars = {"Volvo", "BMW", "Ford", "Mazda"};
Console.WriteLine(cars[0]);
"#;

        let _ = CSharpSession::parse(Rule::statements, input).unwrap();
    }
}
