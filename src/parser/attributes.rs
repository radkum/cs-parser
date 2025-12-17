use super::value::Val;
pub(super) struct Attribute {
    name: String,
    args: Vec<AttributeArg>,
}

impl Attribute {
    pub fn new(name: String, args: Vec<AttributeArg>) -> Self {
        Self { name, args }
    }
}

pub(super) struct AttributeArg {
    key: String,
    value: Option<Val>,
}

impl AttributeArg {
    pub fn new(key: String, value: Option<Val>) -> Self {
        Self { key, value }
    }

    pub fn key(&self) -> &String {
        &self.key
    }

    pub fn default_value(&self) -> &Option<Val> {
        &self.value
    }
}
