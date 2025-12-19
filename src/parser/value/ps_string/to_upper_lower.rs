use super::{MethodError, MethodResult, PsString, Val};

impl PsString {
    pub(super) fn to_upper(&self, args: Vec<Val>) -> MethodResult<Val> {
        if !args.is_empty() && args.len() != 1 {
            return Err(MethodError::new_incorrect_args("toUpper", args));
        }
        let PsString(input) = self;
        Ok(Val::String(input.to_uppercase().into()))
    }

    pub(super) fn to_upper_invariant(&self, args: Vec<Val>) -> MethodResult<Val> {
        if !args.is_empty() {
            return Err(MethodError::new_incorrect_args("toUpperInvariant", args));
        }
        let PsString(input) = self;
        Ok(Val::String(input.to_uppercase().into()))
    }

    pub(super) fn to_lower(&self, args: Vec<Val>) -> MethodResult<Val> {
        if !args.is_empty() && args.len() != 1 {
            return Err(MethodError::new_incorrect_args("toLower", args));
        }
        let PsString(input) = self;
        Ok(Val::String(input.to_lowercase().into()))
    }

    pub(super) fn to_lower_invariant(&self, args: Vec<Val>) -> MethodResult<Val> {
        if !args.is_empty() {
            return Err(MethodError::new_incorrect_args("toLowerInvariant", args));
        }
        let PsString(input) = self;
        Ok(Val::String(input.to_lowercase().into()))
    }
}

#[cfg(test)]
mod tests {
    use crate::{CSharpSession, PsValue};

    #[test]
    fn to_upper() {
        let mut p = CSharpSession::new();
        let input = r#"
$string = 'Hello, World*'
$string = $string.toupper()
$string"#;
        let program_res = p.parse_input(input).unwrap();
        assert_eq!(
            program_res.result(),
            PsValue::String("HELLO, WORLD*".to_string())
        );

        let input = r#"
$string = 'Hello, World*'
$string = $string.toupperinvariant()
$string"#;
        let program_res = p.parse_input(input).unwrap();
        assert_eq!(
            program_res.result(),
            PsValue::String("HELLO, WORLD*".to_string())
        );

        let input = r#"
$string = 'Hello, World*'
$string = $string.toupper("adf")
$string"#;
        let program_res = p.parse_input(input).unwrap();
        assert_eq!(
            program_res.result(),
            PsValue::String("HELLO, WORLD*".to_string())
        );

        let input = r#"
$string = 'Hello, World*'
$string = $string.toupperinvariant("adf")
$string"#;
        let program_res = p.parse_input(input).unwrap();
        assert_eq!(
            program_res.result(),
            PsValue::String("\"Hello, World*\".toupperinvariant(\"adf\")".to_string())
        );
        assert_eq!(
            program_res.errors()[0].to_string(),
            "MethodError: Incorrect arguments \"[\"String(PsString(\\\"adf\\\"))\"]\" for method \
             \"toUpperInvariant\""
                .to_string()
        );
    }

    #[test]
    fn to_lower() {
        let mut p = CSharpSession::new();
        let input = r#"
$string = 'HELLO, world*'
$string = $string.tolower()
$string"#;
        let program_res = p.parse_input(input).unwrap();
        assert_eq!(
            program_res.result(),
            PsValue::String("hello, world*".to_string())
        );

        let input = r#"
$string = 'HELLO, world*'
$string = $string.tolowerinvariant()
$string"#;
        let program_res = p.parse_input(input).unwrap();
        assert_eq!(
            program_res.result(),
            PsValue::String("hello, world*".to_string())
        );

        let input = r#"
$string = 'HELLO, world*'
$string = $string.tolower(', world')
$string"#;
        let program_res = p.parse_input(input).unwrap();
        assert_eq!(
            program_res.result(),
            PsValue::String("hello, world*".to_string())
        );

        let input = r#"
$string = 'HELLO, world*'
$string = $string.tolowerinvariant('hed')
$string"#;
        let program_res = p.parse_input(input).unwrap();
        assert_eq!(
            program_res.result(),
            PsValue::String("\"HELLO, world*\".tolowerinvariant(\"hed\")".to_string())
        );
        assert_eq!(
            program_res.errors()[0].to_string(),
            "MethodError: Incorrect arguments \"[\"String(PsString(\\\"hed\\\"))\"]\" for method \
             \"toLowerInvariant\""
                .to_string()
        );
    }
}
