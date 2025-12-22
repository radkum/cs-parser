#[derive(Debug, Eq, Hash, PartialEq, Clone, Default)]
pub(crate) struct VarName {
    pub scope: Option<Scope>,
    pub name: String,
}

impl VarName {
    pub(crate) fn new(scope: Option<Scope>, name: String) -> Self {
        Self { scope, name }
    }

    pub(crate) fn new_with_scope(scope: Scope, name: String) -> Self {
        Self {
            scope: Some(scope),
            name,
        }
    }
}

impl std::fmt::Display for VarName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub(crate) enum Scope {
    Global,
    Local,
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Scope::Global => write!(f, "global"),
            Scope::Local => write!(f, "local"),
        }
    }
}

impl From<&str> for Scope {
    fn from(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "global" => Scope::Global,
            "local" => Scope::Local,
            _ => Scope::Global,
        }
    }
}
