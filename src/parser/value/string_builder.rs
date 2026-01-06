use super::{
    MethodError, MethodName, MethodResult, PsString, RuntimeObjectTrait, RuntimeTypeTrait, Val,
    runtime_object::{MethodCallType, RuntimeError, RuntimeResult},
    val_type::ObjectType,
};
use crate::{
    CSharpSession,
    parser::{ParserError, ParserResult},
};

#[derive(Debug, Clone)]
pub(crate) struct StringBuilderType {}

impl StringBuilderType {
    pub fn nname() -> String {
        "StringBuilder".to_string()
    }
}

impl RuntimeTypeTrait for StringBuilderType {
    fn init(&self, args: Vec<Val>, _: &mut CSharpSession) -> ParserResult<Val> {
        if args.len() == 1 {
            match &args[0] {
                Val::String(PsString(s)) => {
                    let x = Ok(Val::RuntimeObject(Box::new(StringBuilder {
                        buffer_string: Val::String(PsString(s.clone())),
                    })));
                    return x;
                }
                Val::Int(size) => {
                    let capacity = *size as usize;
                    let buffer = String::with_capacity(capacity * 2);
                    return Ok(Val::RuntimeObject(Box::new(StringBuilder {
                        buffer_string: Val::String(PsString(buffer)),
                    })));
                }
                _ => {}
            }
        }
        Err(ParserError::NotImplemented(format!(
            "StringBuilder::new for args {:?}",
            args
        )))
    }

    fn static_method(
        &self,
        method_name: MethodName,
    ) -> RuntimeResult<super::runtime_object::StaticFnCallType> {
        log::debug!(
            "get_static_method called with method_name: {}",
            method_name.name()
        );
        Err(MethodError::MethodNotFound(method_name.name().to_string()).into())
    }
    fn base_type(&self) -> Box<dyn RuntimeTypeTrait> {
        Box::new(ObjectType {})
    }
    fn name(&self) -> String {
        Self::nname()
    }
    fn clone_rt(&self) -> Box<dyn RuntimeTypeTrait> {
        Box::new(self.clone())
    }
}

#[derive(Debug, Clone)]
struct StringBuilder {
    buffer_string: Val,
}

impl std::fmt::Display for StringBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StringBuilder {{ {:?} }}", self.buffer_string)
    }
}

impl RuntimeObjectTrait for StringBuilder {
    fn method(&self, method_name: MethodName) -> RuntimeResult<MethodCallType> {
        match method_name.name() {
            "Append" => Ok(Box::new(append)),
            "ToString" => Ok(Box::new(to_string)),
            _ => Err(MethodError::MethodNotFound(method_name.name().to_string()).into()),
        }
    }
    fn member(&mut self, name: &str) -> RuntimeResult<&mut Val> {
        match name {
            "buffer_string" => Ok(&mut self.buffer_string),
            _ => Err(RuntimeError::MemberNotFound(name.to_string())),
        }
    }

    fn readonly_member(&mut self, name: &str) -> RuntimeResult<Val> {
        match name {
            "buffer_string" => Ok(self.buffer_string.clone()),
            _ => Err(RuntimeError::MemberNotFound(name.to_string())),
        }
    }
    fn clone_rt(&self) -> Box<dyn RuntimeObjectTrait> {
        Box::new(self.clone())
    }
    fn type_definition(&self) -> Box<dyn RuntimeTypeTrait> {
        Box::new(StringBuilderType {})
    }
}

fn to_string(this: &mut Val, _: Vec<Val>, _: &mut CSharpSession) -> MethodResult<Val> {
    if let Val::RuntimeObject(ro) = this {
        Ok(ro.readonly_member("buffer_string")?)
    } else {
        Err(MethodError::NotImplemented("GetString".into()))
    }
}

fn append(this: &mut Val, args: Vec<Val>, _: &mut CSharpSession) -> MethodResult<Val> {
    if args.len() != 1 {
        return Err(MethodError::IncorrectArgumentCount(
            1,
            args.len(),
            "Append".into(),
        ));
    }

    let ch = args[0].cast_to_char()?;
    if let Val::RuntimeObject(ro) = this {
        let mut buffer_string = ro.member("buffer_string")?;
        if let Val::String(PsString(s)) = &mut buffer_string {
            s.push(ch as u8 as char);
        }
        Ok(Val::Null)
    } else {
        Err(MethodError::NotImplemented("Append".into()))
    }
}

#[cfg(test)]
mod tests {
    use crate::{CSharpSession, PsValue};

    #[test]
    fn test_builtint_objects() {
        let mut p = CSharpSession::new();
        assert_eq!(
            p.parse_input(r#" [system.text.encoding]::unicode "#)
                .unwrap()
                .result(),
            PsValue::String("System.Text.UnicodeEncoding".into())
        );
        assert_eq!(
            p.parse_input(r#" [system.text.encoding]"#)
                .unwrap()
                .result(),
            PsValue::String(
                "IsPublic\tIsSerial\tName\tBaseType\n--------\t--------\t----\t--------\n    \
                 true\t    true\tEncoding\tSystem.Object"
                    .into()
            )
        );
        assert_eq!(
            p.parse_input(r#" [system.text.encoding].name"#)
                .unwrap()
                .result(),
            PsValue::String("Encoding".into())
        );
        assert_eq!(
            p.parse_input(r#" [system.text.encoding].basetype.name"#)
                .unwrap()
                .result(),
            PsValue::String("System.Object".into())
        );
        assert_eq!(
            p.parse_input(r#" [system.text.encoding]"adsf" "#)
                .unwrap()
                .result(),
            PsValue::Null
        );
        assert_eq!(
            p.parse_input(r#" [system.text.encoding]"adsf" "#)
                .unwrap()
                .errors()[0]
                .to_string(),
            "ValError: Failed to convert value String to type RuntimeObject".to_string()
        );
    }
}
