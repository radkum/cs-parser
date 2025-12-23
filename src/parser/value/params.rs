use super::val_type::RuntimeTypeTrait;
use crate::parser::{Val, ValType};

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    name: String,
    ttype: ValType,
    default_value: Option<Val>,
}

impl Param {
    pub fn new(ttype: ValType, name: String, default_value: Option<Val>) -> Self {
        Self {
            name,
            ttype,
            default_value,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ttype(&self) -> ValType {
        self.ttype.clone()
    }

    pub fn default_value(&self) -> Option<Val> {
        self.default_value.clone()
    }
}

impl std::fmt::Display for Param {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let ttype = format!("[{}] ", self.ttype.name());

        let default = if let Some(default) = &self.default_value {
            format!(" = {}", default)
        } else {
            "".to_string()
        };

        write!(f, "{ttype}${}{default}", self.name)
    }
}

pub struct FunctionHeader {
    name: String,
    params: Vec<Param>,
    is_static: bool,
}

impl FunctionHeader {
    pub fn new(name: String, params: Vec<Param>, is_static: bool) -> Self {
        Self {
            name,
            params,
            is_static,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn params(&self) -> &Vec<Param> {
        &self.params
    }

    pub fn is_static(&self) -> bool {
        self.is_static
    }
}

impl std::fmt::Display for FunctionHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let params_str = self
            .params
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<String>>()
            .join(", ");
        write!(f, "void {}({});", self.name, params_str)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Params(pub Vec<Param>);

impl Params {
    pub fn new(params: Vec<Param>) -> Self {
        Self(params)
    }
}

impl std::fmt::Display for Params {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let params_str = self
            .0
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<String>>()
            .join(", ");
        write!(f, "param (\n{}\n)", params_str)
    }
}
