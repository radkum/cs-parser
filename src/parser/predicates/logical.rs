use std::{collections::HashMap, sync::LazyLock};

use super::Val;

pub(crate) type LogicalPredType = fn(Val, Val) -> bool;

pub(crate) struct LogicalPred;

impl LogicalPred {
    const LOGICAL_PRED_MAP: LazyLock<HashMap<&'static str, LogicalPredType>> =
        LazyLock::new(|| HashMap::from([("&&", and as _), ("||", or as _)]));

    pub(crate) fn get(name: &str) -> Option<LogicalPredType> {
        Self::LOGICAL_PRED_MAP.get(name).copied()
    }
}

pub fn and(a: Val, b: Val) -> bool {
    a.cast_to_bool() && b.cast_to_bool()
}

pub fn or(a: Val, b: Val) -> bool {
    a.cast_to_bool() || b.cast_to_bool()
}

pub fn xor(a: Val, b: Val) -> bool {
    a.cast_to_bool() != b.cast_to_bool()
}

// #[cfg(test)]
// mod tests {
//     use crate::CSharpSession;

//     #[test]
//     fn test_and() {
//         let mut p = CSharpSession::new();
//         assert_eq!(
//             p.safe_eval_statements(r#" $true -AND $true "#).unwrap(),
//             "True".to_string()
//         );
//         assert_eq!(
//             p.safe_eval_statements(r#" $true -And $false "#).unwrap(),
//             "False".to_string()
//         );
//         assert_eq!(
//             p.safe_eval_statements(r#" $false -and $true "#).unwrap(),
//             "False".to_string()
//         );
//         assert_eq!(
//             p.safe_eval_statements(r#" $false -and $false "#).unwrap(),
//             "False".to_string()
//         );

//         assert_eq!(
//             p.safe_eval_statements(r#" $true -and $false -and $true
// "#).unwrap(),             "False".to_string()
//         );
//     }

//     #[test]
//     fn test_or() {
//         let mut p = CSharpSession::new();
//         assert_eq!(
//             p.safe_eval_statements(r#" $true -oR $true "#).unwrap(),
//             "True".to_string()
//         );
//         assert_eq!(
//             p.safe_eval_statements(r#" $true -or $false "#).unwrap(),
//             "True".to_string()
//         );
//         assert_eq!(
//             p.safe_eval_statements(r#" $false -or $true "#).unwrap(),
//             "True".to_string()
//         );
//         assert_eq!(
//             p.safe_eval_statements(r#" $false -or $false "#).unwrap(),
//             "False".to_string()
//         );

//         assert_eq!(
//             p.safe_eval_statements(r#" $true -Or $false -or $true
// "#).unwrap(),             "True".to_string()
//         );
//     }

//     #[test]
//     fn test_xor() {
//         let mut p = CSharpSession::new();
//         assert_eq!(
//             p.safe_eval_statements(r#" $true -Xor $true "#).unwrap(),
//             "False".to_string()
//         );
//         assert_eq!(
//             p.safe_eval_statements(r#" $true -xOr $false "#).unwrap(),
//             "True".to_string()
//         );
//         assert_eq!(
//             p.safe_eval_statements(r#" $false -XOR $true "#).unwrap(),
//             "True".to_string()
//         );
//         assert_eq!(
//             p.safe_eval_statements(r#" $false -xor $false "#).unwrap(),
//             "False".to_string()
//         );
//     }

//     #[test]
//     fn test_not() {
//         let mut p = CSharpSession::new();
//         assert_eq!(p.safe_eval_statements(r#" -Not 4 "#).unwrap(),
// "False".to_string());         assert_eq!(p.safe_eval_statements(r#" -nOt ""
// "#).unwrap(), "True".to_string());         assert_eq!(p.
// safe_eval_statements(r#" -not "asd" "#).unwrap(), "False".to_string());
//         assert_eq!(
//             p.safe_eval_statements(r#" -nOt "96.5" "#).unwrap(),
//             "False".to_string()
//         );
//         assert_eq!(
//             p.safe_eval_statements(r#" -Not "+96.5" "#).unwrap(),
//             "False".to_string()
//         );
//         assert_eq!(
//             p.safe_eval_statements(r#" -NOT "96.5as" "#).unwrap(),
//             "False".to_string()
//         );
//     }
// }
