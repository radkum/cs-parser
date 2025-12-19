mod arithmetic;
mod bitwise;
mod comparison;
mod contain;
mod join;
mod logical;
mod replace;
mod split;
mod type_check;

pub(crate) use arithmetic::ArithmeticPred;
pub(crate) use bitwise::{BitwiseError, BitwisePred};
pub(crate) use comparison::ComparisonPred;
pub(crate) use contain::ContainPred;
pub(crate) use join::JoinPred;
pub(crate) use logical::LogicalPred;
pub(crate) use replace::ReplacePred;
pub(crate) use split::SplitPred;
use thiserror_no_std::Error;
pub(crate) use type_check::TypeCheckPred;

use super::{Val, ValResult, ValType, value::ValError};
const AS_PREDICATE: &str = "-as";

#[derive(Error, Debug, PartialEq, Clone)]
pub enum OpError {
    #[error("The -ireplace operator allows only two elements to follow it, not {0}")]
    ReplaceInvalidArgsNumber(usize),
    #[error("-is and -isnot needs to have type on ther right side, not {0}")]
    NotType(String),
    #[error("ValError: {0}")]
    ValError(ValError),
    #[error("Unknown type: {0}")]
    UnknownType(String),
}

impl From<ValError> for OpError {
    fn from(value: ValError) -> Self {
        Self::ValError(value)
    }
}

type OpResult<T> = core::result::Result<T, OpError>;

pub(crate) type StringPredType = Box<dyn Fn(Val, Val) -> OpResult<Val>>;

pub(crate) struct StringPred;
impl StringPred {
    pub(crate) fn get(name: &str) -> Option<StringPredType> {

        //-as is very simple, thats why there is no single module for that
        if name == AS_PREDICATE {
            return Some(Box::new(move |v1, v2| Ok(v1.cast(&v2)?)));
        }

        if let Some(compare) = ComparisonPred::get(name) {
            return Some(Box::new(move |v1, v2| Ok(Val::Bool(compare(v1, v2)))));
        }

        if let Some(replace) = ReplacePred::get(name) {
            return Some(Box::new(move |v1, v2| {
                let (from, to) = if let Val::Array(arr) = v2 {
                    if arr.len() == 1 {
                        (arr[0].clone(), Val::Null)
                    } else if arr.len() == 2 {
                        (arr[0].clone(), arr[1].clone())
                    } else {
                        Err(OpError::ReplaceInvalidArgsNumber(arr.len()))?
                    }
                } else {
                    (v2, Val::Null)
                };
                Ok(Val::String(replace(v1, from, to).into()))
            }));
        }

        if let Some(type_check) = TypeCheckPred::get(name) {
            return Some(Box::new(move |v1, v2| {
                if let Val::RuntimeType(rt) = &v2 {
                    return Ok(Val::Bool(type_check(v1, rt.type_definition())));
                }

                Err(OpError::NotType(v2.cast_to_string()))
            }));
        }

        if let Some(join) = JoinPred::get(name) {
            return Some(Box::new(move |v1, v2| Ok(Val::String(join(v1, v2).into()))));
        }

        if let Some(split) = SplitPred::get(name) {
            return Some(Box::new(move |v1, v2| Ok(split(v1, v2))));
        }

        if let Some(contain) = ContainPred::get(name) {
            return Some(Box::new(move |v1, v2| Ok(Val::Bool(contain(v1, v2)))));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use crate::{CSharpSession, Variables};
    #[test]
    fn test_range_with_float() {
        let mut p = CSharpSession::new();
        assert_eq!(p.safe_eval_statements(r#" [string](1..1.3) "#).unwrap().to_string().as_str(), "1");
        assert_eq!(p.safe_eval_statements(r#" [string](1...3) "#).unwrap().to_string().as_str(), "1 0");
        assert_eq!(p.safe_eval_statements(r#" [string]1...3 "#).unwrap().to_string().as_str(), "1\n0");
    }

    #[test]
    fn test_unary() {
        let mut p = CSharpSession::new();
        assert_eq!(p.safe_eval_statements(r#" +5 "#).unwrap().to_string().as_str(), "5");
        assert_eq!(p.safe_eval_statements(r#" -5 "#).unwrap().to_string().as_str(), "-5");
    }

    #[test]
    fn test_format_operator() {
        let mut p = CSharpSession::new();
        assert_eq!(
            p.safe_eval_statements(r#" "Hello, {0}!" -f "world" "#)
                .unwrap()
                .to_string().as_str(),
            "Hello, world!"
        );
        assert_eq!(
            p.safe_eval_statements(r#" "Hello, {0}!" -f "every{0}" -f "body"  "#)
.unwrap()
            .to_string()
            .as_str(),
            "Hello, everybody!"
        );
        assert_eq!(
            p.safe_eval_statements(r#" "{0} + {1} = {2}" -f 5, 7, (5 + 7) "#)
                .unwrap()
                .to_string().as_str(),
            "5 + 7 = 12"
        );
        assert_eq!(
            p.safe_eval_statements(r#" "{0:N2}" -f 1234.56789 "#).unwrap()
            .to_string()
            .as_str(),
            "1234.57"
        );
        assert_eq!(
            p.safe_eval_statements(r#" "|{0,10}|" -f "Hi" "#).unwrap()
            .to_string()
            .as_str(),
            "|          Hi|"
        );
        assert_eq!(
            p.safe_eval_statements(
                r#" $level = "INFO";$message = "Disk space low";"{0}: {1}" -f $level, $message "#
            )
            .unwrap()
            .to_string()
            .as_str(),
            "INFO: Disk space low"
        );
        assert_eq!(
            p.safe_eval_statements(r#" "{0:310100a0b00}" -f 578 "#)
.unwrap()
            .to_string()
            .as_str(),
            "310100a5b78"
        );
    }

    #[test]
    fn test_strings() {
        let mut p = CSharpSession::new().with_variables(Variables::new().values_persist());
        assert_eq!(
            p.safe_eval_statements(r#" 'It''s fine' "#).unwrap()
            .to_string()
            .as_str(),
            "It''s fine"
        );
        assert_eq!(
            p.safe_eval_statements(r#" "Price is $" "#).unwrap()
            .to_string()
            .as_str(),
            "Price is $"
        );
        assert_eq!(
            p.safe_eval_statements(r#" "Result: $(1+2)" "#).unwrap()
            .to_string()
            .as_str(),
            "Result: 3"
        );
        assert_eq!(
            p.safe_eval_statements(r#" $name = "Radek";"Hello $name" "#)
.unwrap()
            .to_string()
            .as_str(),
            "Hello Radek"
        );
        assert_eq!(
            p.safe_eval_statements(r#" "This is a quote: `"" "#).unwrap()
            .to_string()
            .as_str(),
            "This is a quote: \""
        );
        assert_eq!(
            p.safe_eval_statements(r#" "A backtick `` and escaped quote `"" "#)
.unwrap()
            .to_string()
            .as_str(),
            "A backtick ` and escaped quote \""
        );
        assert_eq!(
            p.safe_eval_statements(
                r#" @"
Hello $name $
Multiline with $(1+2)
"@ "#
            )
            .unwrap()
            .to_string()
            .as_str(),
            "Hello Radek $\nMultiline with 3"
        );

        assert_eq!(
            p.safe_eval_statements(
                r#" @'
This is a
multi-line
here string
'@ "#
            )
            .unwrap()
            .to_string()
            .as_str(),
            "This is a\nmulti-line\nhere string"
        );
    }
}
