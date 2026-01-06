use smart_default::SmartDefault;

#[derive(Debug, Eq, Hash, PartialEq, Clone, Default)]
pub(crate) struct VarName {
    pub scope: Scope,
    pub name: String,
}

impl VarName {
    pub(crate) fn new(name: String) -> Self {
        Self {
            scope: Scope::Local,
            name,
        }
    }

    pub(crate) fn new_static(name: String, hierarchy: &Vec<String>) -> Self {
        Self {
            scope: Scope::Static(hierarchy.clone()),
            name,
        }
    }
}

impl std::fmt::Display for VarName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.scope {
            Scope::Static(ref hierarchy) => write!(f, "{}.{}", hierarchy.join("."), self.name),
            Scope::Local => write!(f, "{}", self.name),
        }
    }
}

#[derive(Debug, Eq, Hash, PartialEq, Clone, SmartDefault)]
pub(crate) enum Scope {
    #[default]
    Static(Vec<String>),
    Local,
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Scope::Static(hierarchy) => write!(f, "static {}", hierarchy.join("::")),
            Scope::Local => write!(f, "local"),
        }
    }
}

impl From<&str> for Scope {
    fn from(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "static" => Scope::Static(vec![]),
            "local" => Scope::Local,
            _ => Scope::Local,
        }
    }
}
