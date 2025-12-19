use super::{
    MethodError, MethodResult, RuntimeObjectTrait, RuntimeTypeTrait, Val,
    runtime_object::{MethodCallType, RuntimeError, RuntimeResult},
    val_type::ObjectType,
};
use crate::parser::{MethodName, value::PsString};
use crate::parser::ParserResult;
use crate::CSharpSession;

#[derive(Debug, Clone)]
pub(crate) struct StringBuilderType {
}

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
                    let buffer = s.as_bytes().to_vec();
                    return Ok(Val::RuntimeObject(Box::new(StringBuilder { buffer })));
                },
                Val::Int(size) => {
                    let capacity = *size as usize;
                    let mut buffer = Vec::with_capacity(capacity * 2);
                    buffer.resize(capacity * 2, 0u8);
                    return Ok(Val::RuntimeObject(Box::new(StringBuilder { buffer })));
                },
                _ => {}
            }
        }
        Err(crate::parser::error::ParserError::NotImplemented(format!("StringBuilder::new for args {:?}", args).into()))
    }

    fn static_method(&self, method_name: MethodName) -> RuntimeResult<super::runtime_object::StaticFnCallType> {
        log::debug!("get_static_method called with method_name: {}", method_name.name());
        match method_name.name() {
            //_ => Ok(Box::new(to_string)),
            _ => Err(MethodError::MethodNotFound(method_name.name().to_string()).into()),
        }
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
    buffer: Vec<u8>,
}

impl std::fmt::Display for StringBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StringBuilder {{ {:?} }}", self.buffer)
    }
}

impl RuntimeObjectTrait for StringBuilder {
    fn method(&self, method_name: MethodName) -> RuntimeResult<MethodCallType> {
        match method_name.name() {
            "Append" => Ok(Box::new(append)),
            "toString" => Ok(Box::new(to_string)),
            _ => Err(MethodError::MethodNotFound(method_name.name().to_string()).into()),
        }
    }
    fn readonly_member(&mut self, name: &str) -> RuntimeResult<Val> {
        match name.to_ascii_lowercase().as_str() {
            "buffer_string" => Ok(Val::String(string_from_vec(self.buffer.clone()).into())),
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
        ro.readonly_member("buffer_string")?;
    }
    Err(MethodError::NotImplemented("GetString".into()).into())
}

fn append(this: &mut Val, _: Vec<Val>, _: &mut CSharpSession) -> MethodResult<Val> {
    todo!()
}

fn string_from_vec(mut buf: Vec<u8>) -> String {
    //if buf len is odd, then last char should be 0x65533
    let add_last = if !buf.len().is_multiple_of(2) {
        buf.pop();
        true
    } else {
        false
    };
    let u16_buffer = unsafe { buf.align_to_mut::<u16>().1 };

    let mut ends_with_null = false;
    if let Some(c) = u16_buffer.last()
        && *c == 0
    {
        ends_with_null = true;
    }

    let mut res_string = String::from_utf16_lossy(u16_buffer);
    if ends_with_null {
        res_string.pop();
    }

    if add_last {
        res_string.push('\u{FFFD}');
    }
    res_string
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
