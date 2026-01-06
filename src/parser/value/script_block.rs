use super::{
    MethodCallType, RuntimeObjectTrait, Val, ValType,
    params::{Param, Params},
    runtime_object::StaticFnCallType,
};
use crate::{
    CSharpSession,
    parser::{ParserError, ParserResult, VarName},
};

#[derive(Debug, Clone, Default)]
pub(crate) struct ScriptBlock {
    pub params: Params,
    pub body: String,
    pub raw_text: String,
    pub deobfuscated: Vec<String>,
}

impl RuntimeObjectTrait for ScriptBlock {
    fn clone_rt(&self) -> Box<dyn RuntimeObjectTrait> {
        Box::new(self.clone())
    }

    fn type_definition(&self) -> Box<dyn super::RuntimeTypeTrait> {
        Box::new(ValType::ScriptBlock)
    }
}

impl ScriptBlock {
    pub fn new(script: String) -> Self {
        Self {
            params: Params::new(Vec::new()),
            body: script.clone(),
            raw_text: script,
            deobfuscated: Vec::new(),
        }
    }
}

impl std::fmt::Display for ScriptBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.raw_text)
    }
}

impl ScriptBlock {
    pub fn with_params(self, params: Vec<Param>) -> ScriptBlock {
        ScriptBlock {
            params: Params::new(params),
            body: self.body,
            raw_text: self.raw_text,
            deobfuscated: self.deobfuscated,
        }
    }

    pub fn run_mut(&mut self, args: Vec<Val>, ps: &mut CSharpSession) -> ParserResult<Val> {
        if self.body.is_empty() {
            return Ok(Val::Null);
        }

        for (i, param) in self.params.0.iter().enumerate() {
            let val = args
                .get(i)
                .cloned()
                .unwrap_or(param.default_value().unwrap_or(Val::Null));
            ps.variables
                .set_local(param.name(), val)
                .map_err(ParserError::from)?;
        }

        let (script_last_output, _) = ps.eval_statement_block_string(self.body.as_str());
        //output.into_iter().for_each(|f| ps.add_output_statement(f));
        Ok(script_last_output)
    }

    pub fn run(&self, args: Vec<Val>, ps: &mut CSharpSession) -> ParserResult<Val> {
        let mut self_clone = self.clone();
        self_clone.run_mut(args, ps)
    }

    pub(crate) fn get_static_method(&self) -> Option<StaticFnCallType> {
        let fn_body = self.clone();
        Some(Box::new(move |params, session| {
            let mut owned = fn_body.clone();
            owned
                .run_static_method(params, session)
                .map_err(|e| super::MethodError::RuntimeError(e.to_string()))
        }))
    }

    pub fn run_static_method(
        &mut self,
        args: Vec<Val>,
        session: &mut CSharpSession,
    ) -> ParserResult<Val> {
        session.push_scope_session();
        let script_last_output = self.run_static_method_impl(args, session);
        session.pop_scope_session();
        script_last_output
    }

    fn run_static_method_impl(
        &mut self,
        args: Vec<Val>,
        ps: &mut CSharpSession,
    ) -> ParserResult<Val> {
        for (i, param) in self.params.0.iter().enumerate() {
            let val = args
                .get(i)
                .cloned()
                .unwrap_or(param.default_value().unwrap_or(Val::Null));
            ps.variables
                .set_local(param.name(), val)
                .map_err(ParserError::from)?;
        }
        let (script_last_output, _) = ps.eval_statement_block_string(self.body.as_str());

        Ok(script_last_output)
    }

    pub(crate) fn get_method(&self) -> Option<MethodCallType> {
        let fn_body = self.clone();
        Some(Box::new(move |object: &mut Val, args, session| {
            let mut owned = fn_body.clone();
            owned
                .run_method(object, args, session)
                .map_err(|e| super::MethodError::RuntimeError(e.to_string()))
        }))
    }

    pub fn run_method(
        &mut self,
        this: &mut Val,
        args: Vec<Val>,
        session: &mut CSharpSession,
    ) -> ParserResult<Val> {
        if self.body.is_empty() {
            return Ok(Val::Null);
        }
        session.push_scope_session();
        let script_last_output = self.run_method_impl(this, args, session);
        session.pop_scope_session();
        script_last_output
    }

    fn run_method_impl(
        &mut self,
        this: &mut Val,
        args: Vec<Val>,
        session: &mut CSharpSession,
    ) -> ParserResult<Val> {
        session
            .variables
            .set_local("this", this.clone())
            .map_err(ParserError::from)?;
        for (i, param) in self.params.0.iter().enumerate() {
            let val = args
                .get(i)
                .cloned()
                .unwrap_or(param.default_value().unwrap_or(Val::Null));
            session
                .variables
                .set_local(param.name(), val)
                .map_err(ParserError::from)?;
        }

        let (script_last_output, _) = session.eval_statement_block_string(self.body.as_str());
        if let Some(val) =
            session
                .variables
                .get(&VarName::new("this".to_string()), &session.types_map, None)
        {
            *this = val.clone();
        }
        Ok(script_last_output)
    }
}

#[cfg(test)]
mod tests {
    use crate::{CSharpSession, NEWLINE};

    #[test]
    fn simple() {
        let mut p = CSharpSession::new();
        let input = r#"$scriptblock = {3};$scriptblock"#;
        let s = p.parse_input(input).unwrap();
        assert_eq!(s.result().to_string(), "3".to_string());
    }

    #[test]
    fn test_script_block() {
        let mut p = CSharpSession::new();
        let input = r#"$elo = 3;$sb = { param($x, $y = 4); $x+$y+$elo};&$sb 1 2"#;
        let program_res = p.parse_input(input).unwrap();
        assert_eq!(program_res.result().to_string(), "6".to_string());
        assert_eq!(
            program_res.deobfuscated(),
            vec!["$elo = 3", "$sb = {param($x, $y = 4); $x+$y+$elo}", "6",].join(NEWLINE)
        );
        assert_eq!(program_res.output(), "6".to_string());
        assert_eq!(program_res.errors().len(), 0);
    }

    #[test]
    fn test_script_block_default_args() {
        let mut p = CSharpSession::new();
        let input = r#"$elo = 3;$sb = { param($x, $y = 4); $x+$y+$elo};.$sb 1"#;
        let s = p.parse_input(input).unwrap();
        assert_eq!(s.result().to_string(), "8".to_string());
    }

    #[test]
    fn test_non_existing_script_block() {
        let mut p = CSharpSession::new();
        let input = r#"$elo = 3;$sb = { param($x, $y = 4); $x+$y+$elo};.$sb2 1"#;
        let program_res = p.parse_input(input).unwrap();
        assert!(program_res.result().to_string().is_empty(),);
        assert_eq!(
            program_res.deobfuscated(),
            vec![
                "$elo = 3",
                "$sb = {param($x, $y = 4); $x+$y+$elo}",
                ".$sb2 1",
            ]
            .join(NEWLINE)
        );
        assert!(program_res.output().is_empty(),);
        assert_eq!(program_res.errors().len(), 1);
        assert_eq!(
            program_res.errors()[0].to_string(),
            "VariableError: Variable \"sb2\" is not defined"
        );
    }

    #[test]
    fn test_script_block_value_assignment() {
        let mut p = CSharpSession::new();
        let input = r#"$scriptBlock = {param($x, $y) return $x + $y};& $scriptBlock 10 20"#;
        let s = p.parse_input(input).unwrap();
        assert_eq!(s.result().to_string(), "30".to_string());
    }

    #[test]
    fn test_script_block_without_assignment() {
        let mut p = CSharpSession::new();
        let input = r#"& {param($x, $y) return $x + $y} 10 20 40"#;
        let s = p.parse_input(input).unwrap();
        assert_eq!(s.deobfuscated(), "30".to_string());
        assert_eq!(s.result().to_string(), "30".to_string());
    }
}
