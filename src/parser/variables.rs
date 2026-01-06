mod function;
mod variable;

use std::collections::HashMap;

pub(super) use function::FunctionMap;
use phf::phf_map;
use thiserror_no_std::Error;
pub(super) use variable::{Scope, VarName};

use crate::parser::{ClassType, RuntimeTypeTrait, Val, value::ScriptBlock};
#[derive(Error, Debug, PartialEq, Clone)]
pub enum VariableError {
    #[error("Variable \"{0}\" is not defined")]
    NotDefined(String),
    #[error("Cannot overwrite variable \"{0}\" because it is read-only or constant.")]
    ReadOnly(String),
}

pub type VariableResult<T> = core::result::Result<T, VariableError>;
pub type VariableMap = HashMap<String, Val>;

#[derive(Clone, Default)]
pub struct Variables {
    static_scope: VariableMap,
    variables_stack: Vec<VariableMap>,
    global_functions: FunctionMap,
    state: Stack,
    values_persist: bool,
    this: Option<ClassType>,
    //special variables
    // status: bool, // $?
    // first_token: Option<String>,
    // last_token: Option<String>,
    // current_pipeline: Option<String>,
}

#[derive(Default, Clone)]
struct Stack(u32);

impl Variables {
    const PREDEFINED_VARIABLES: phf::Map<&'static str, Val> = phf_map! {
        "true" => Val::Bool(true),
        "false" => Val::Bool(false),
        "null" => Val::Null,
    };

    pub(crate) fn set_this(&mut self, val: ClassType) {
        self.this = Some(val);
    }

    pub fn load_from_file(
        &mut self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut config_parser = configparser::ini::Ini::new();
        let map = config_parser.load(path)?;
        self.load(map)
    }

    pub fn load_from_string(&mut self, ini_string: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut config_parser = configparser::ini::Ini::new();
        let map = config_parser.read(ini_string.into())?;
        self.load(map)
    }

    pub fn init(&mut self) {
        self.variables_stack.clear();
        self.variables_stack.push(VariableMap::new());
        self.state = Stack::default();
    }

    fn load(
        &mut self,
        conf_map: HashMap<String, HashMap<String, Option<String>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for (section_name, properties) in conf_map {
            for (key, value) in properties {
                let Some(value) = value else {
                    continue;
                };

                let var_name = match section_name.as_str() {
                    "static" => VarName::new(key),
                    _ => {
                        continue;
                    }
                };

                // Try to parse the value as different types
                let parsed_value = if let Ok(bool_val) = value.parse::<bool>() {
                    Val::Bool(bool_val)
                } else if let Ok(int_val) = value.parse::<i64>() {
                    Val::Int(int_val)
                } else if let Ok(float_val) = value.parse::<f64>() {
                    Val::Float(float_val)
                } else if value.is_empty() {
                    Val::Null
                } else {
                    Val::String(value.clone().into())
                };

                // Insert the variable (overwrite if it exists and is not read-only)
                if let Err(err) = self.set(&var_name, parsed_value.clone()) {
                    log::error!("Failed to set variable {:?}: {}", var_name, err);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn get_global(&self) -> VariableMap {
        self.static_scope.clone()
    }

    pub(crate) fn add_global_function(&mut self, name: String, func: ScriptBlock) {
        self.global_functions.insert(name, func);
    }

    /// Creates a new empty Variables container.
    ///
    /// # Arguments
    ///
    /// * initializes the container with CSharp built-in variables like `$true`,
    ///   `$false`, `$null`, and `$?`. If `false`,
    ///
    /// # Returns
    ///
    /// A new `Variables` instance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cs_parser::Variables;
    ///
    /// // Create with built-in variables
    /// let vars_with_builtins = Variables::new();
    ///
    /// // Create empty
    /// let empty_vars = Variables::new();
    /// ```
    pub fn new() -> Variables {
        Default::default()
    }

    // not exported in this version
    #[allow(dead_code)]
    pub(crate) fn values_persist(mut self) -> Self {
        self.values_persist = true;
        self
    }

    /// Loads variables from an INI configuration file.
    ///
    /// This method parses an INI file and loads its key-value pairs as
    /// CSharp variables. Variables are organized by INI sections, with
    /// the `[global]` section creating global variables and other sections
    /// creating scoped variables.
    ///
    /// # Arguments
    ///
    /// * `path` - A reference to the path of the INI file to load.
    ///
    /// # Returns
    ///
    /// * `Result<Variables, VariableError>` - A Variables instance with the
    ///   loaded data, or an error if the file cannot be read or parsed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cs_parser::{Variables, CSharpSession};
    /// use std::path::Path;
    ///
    /// // Load from INI file
    /// let variables = Variables::from_ini_string("[global]\nname = John Doe\n[local]\nlocal_var = \"local_value\"").unwrap();
    /// let mut session = CSharpSession::new().with_variables(variables);
    ///
    /// // Access loaded variables
    /// let name = session.safe_eval("$global:name").unwrap();
    /// let local_var = session.safe_eval("$local:local_var").unwrap();
    /// ```
    ///
    /// # INI Format
    ///
    /// ```ini
    /// # Global variables (accessible as $global:key)
    /// [global]
    /// name = John Doe
    /// version = 1.0
    ///
    /// # Local scope variables (accessible as $local:key)
    /// [local]
    /// temp_dir = /tmp
    /// debug = true
    /// ```
    pub fn from_ini_string(ini_string: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut variables = Self::new();
        variables.load_from_string(ini_string)?;
        Ok(variables)
    }

    /// Create a new Variables instance with variables loaded from an INI file
    pub fn from_ini_file(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut variables = Self::new();
        variables.load_from_file(path)?;
        Ok(variables)
    }

    // fn const_map_from_scope(&self, scope: &Scope) -> &VariableMap {
    //     match scope {
    //         Scope::Static => &self.static_scope,
    //         Scope::Local => match self.state {
    //             Stack(depth) => &self.variables_stack[depth as usize],
    //         },
    //     }
    // }

    pub(crate) fn current_scope(&mut self) -> &mut VariableMap {
        let depth = self.state.0;
        &mut self.variables_stack[depth as usize]
    }

    pub(crate) fn get_current_scope(&mut self) -> VariableMap {
        self.current_scope().clone()
    }

    pub(crate) fn get_static_scope(&mut self) -> VariableMap {
        self.static_scope.clone()
    }

    // fn map_from_scope(&mut self, scope: &Scope) -> &mut VariableMap {
    //     match scope {
    //         Scope::Static => &mut self.static_scope,
    //         Scope::Local => self.current_scope(),
    //     }
    // }

    /// Sets the value of a variable in the specified scope.
    ///
    /// # Arguments
    ///
    /// * `var_name` - The variable name and scope information.
    /// * `val` - The value to assign to the variable.
    ///
    /// # Returns
    ///
    /// * `Result<(), VariableError>` - Success or an error if the variable is
    ///   read-only.
    pub(crate) fn set(&mut self, var_name: &VarName, val: Val) -> VariableResult<()> {
        let var = self.find_mut_variable_in_scopes(var_name)?;

        if let Some(variable) = var {
            *variable = val;
        } else {
            let map = self.current_scope();
            map.insert(var_name.name.clone(), val);
        }

        Ok(())
    }

    pub(crate) fn update(&mut self, var_name: &VarName, val: Val) -> VariableResult<()> {
        let var = self.find_mut_variable_in_scopes(var_name)?;

        if let Some(variable) = var {
            *variable = val;
        } else {
            log::warn!("Variable {:?} not defined, cannot update", var_name);
        }

        Ok(())
    }

    pub(crate) fn set_local(&mut self, name: &str, val: Val) -> VariableResult<()> {
        let var_name = VarName::new(name.to_string());
        self.set(&var_name, val)
    }

    fn find_mut_variable_in_scopes(
        &mut self,
        var_name: &VarName,
    ) -> VariableResult<Option<&mut Val>> {
        let name = var_name.name.to_string();
        let name_str = name.as_str();

        // if let Some(scope) = &var_name.scope {
        //     let map = self.map_from_scope(scope);
        //     Ok(map.get_mut(name_str))
        // } else {
        if Self::PREDEFINED_VARIABLES.contains_key(name_str) {
            return Err(VariableError::ReadOnly(name.clone()));
        }

        // No scope specified, check local scopes first, then globals
        for local_scope in self.variables_stack.iter_mut().rev() {
            if local_scope.contains_key(name_str) {
                return Ok(local_scope.get_mut(name_str));
            }
        }

        // Finally, check static scope
        if self
            .static_scope
            .contains_key(var_name.to_string().as_str())
        {
            return Ok(self.static_scope.get_mut(var_name.to_string().as_str()));
        }

        Ok(None)
        //}
    }

    /// Retrieves the value of a variable from the appropriate scope.
    ///
    /// # Arguments
    ///
    /// * `var_name` - The variable name and scope information.
    ///
    /// # Returns
    ///
    /// * `VariableResult<Val>` - The variable's value, or an error if not
    ///   found.
    pub(crate) fn get(
        &self,
        var_name: &VarName,
        types_map: &HashMap<String, Box<dyn RuntimeTypeTrait>>,
        hierarchy: Option<&Vec<String>>,
    ) -> Option<Val> {
        let var = self.find_variable_in_scopes(var_name);

        if var.is_none() {
            let rt = types_map.get(var_name.name.as_str())?;
            Some(Val::RuntimeType(rt.clone_rt()))
        } else {
            var.cloned()
        }
    }

    pub(crate) fn get_mut(&mut self, var_name: &VarName) -> Option<&mut Val> {
        let Ok(var) = self.find_mut_variable_in_scopes(var_name) else {
            log::error!(
                "Failed to get mutable variable: {:?}. It's read-only",
                var_name
            );
            return None;
        };

        var
    }

    fn find_variable_in_scopes(&self, var_name: &VarName) -> Option<&Val> {
        let name = var_name.name.to_string();
        let name_str = name.as_str();

        // if let Some(scope) = &var_name.scope {
        //     let map = self.const_map_from_scope(scope);
        //     map.get(name_str)
        // } else {
        if Self::PREDEFINED_VARIABLES.contains_key(name_str) {
            return Self::PREDEFINED_VARIABLES.get(name_str);
        }

        // No scope specified, check local scopes first, then globals
        for local_scope in self.variables_stack.iter().rev() {
            if local_scope.contains_key(name_str) {
                return local_scope.get(name_str);
            }
        }

        // check class scope
        if let Some(class) = &self.this {
            return class.property(name_str);
        }

        if self
            .static_scope
            .contains_key(var_name.to_string().as_str())
        {
            return self.static_scope.get(var_name.to_string().as_str());
        }

        None
        //}
    }

    pub(crate) fn push_scope_session(&mut self) {
        let current_map = self.current_scope();
        let new_map = current_map.clone();

        self.variables_stack.push(new_map);
        self.state = Stack(self.variables_stack.len() as u32 - 1);
    }

    pub(crate) fn pop_scope_session(&mut self) {
        match self.variables_stack.len() {
            0 => {} /* unreachable */
            _ => {
                self.variables_stack.pop();
                self.state = Stack(self.variables_stack.len() as u32 - 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Variables;
    use crate::{CSharpSession, PsValue};

    #[test]
    fn test_builtin_variables() {
        let mut p = CSharpSession::new();
        assert_eq!(
            p.safe_eval_statements(r#" true;"#)
                .unwrap()
                .to_string()
                .as_str(),
            "True"
        );
        assert_eq!(
            p.safe_eval_statements(r#" false;"#)
                .unwrap()
                .to_string()
                .as_str(),
            "False"
        );
        assert_eq!(
            p.safe_eval_statements(r#" null;"#)
                .unwrap()
                .to_string()
                .as_str(),
            ""
        );
    }

    #[test]
    fn test_variables() {
        let mut p = CSharpSession::new();

        let program_res = p
            .parse_statements_string_as_program(
                r#" int var_int = 5; string var_string = "assdfa"; "#,
            )
            .unwrap();
        let script_variables = program_res.static_variables();
        assert_eq!(script_variables.get("var_int"), Some(&PsValue::Int(5)));
        assert_eq!(
            script_variables.get("var_string"),
            Some(&PsValue::String("assdfa".into()))
        );
    }

    #[test]
    fn test_from_ini() {
        let input = r#"[global]
name = radek
age = 30
is_admin = true
height = 5.9
empty_value =

[script]
local_var = "local_value"
        "#;
        let mut variables = Variables::new().values_persist();
        variables.load_from_string(input).unwrap();
        let mut p = CSharpSession::new().with_variables(variables);

        assert_eq!(
            p.safe_eval_statements(r#" name; "#).unwrap(),
            PsValue::String("radek".into())
        );
        assert_eq!(
            p.safe_eval_statements(r#" age; "#).unwrap(),
            PsValue::Int(30)
        );
        assert_eq!(
            p.safe_eval_statements(r#" false; "#)
                .unwrap()
                .to_string()
                .as_str(),
            "False"
        );
        assert_eq!(
            p.safe_eval_statements(r#" null; "#)
                .unwrap()
                .to_string()
                .as_str(),
            ""
        );
        assert_eq!(
            p.safe_eval_statements(r#" local_var; "#)
                .unwrap()
                .to_string()
                .as_str(),
            "\"local_value\""
        );
    }

    #[test]
    fn test_from_ini_string() {
        let input = r#"[global]
name = radek
age = 30
is_admin = true
height = 5.9
empty_value =

[script]
local_var = "local_value"
        "#;

        let variables = Variables::from_ini_string(input).unwrap().values_persist();
        let mut p = CSharpSession::new().with_variables(variables);
        assert_eq!(
            p.safe_eval_statements(r#" name; "#).unwrap(),
            PsValue::String("radek".into())
        );
        assert_eq!(
            p.safe_eval_statements(r#" age; "#).unwrap(),
            PsValue::Int(30)
        );
        assert_eq!(
            p.safe_eval_statements(r#" false; "#)
                .unwrap()
                .to_string()
                .as_str(),
            "False"
        );
        assert_eq!(
            p.safe_eval_statements(r#" null; "#)
                .unwrap()
                .to_string()
                .as_str(),
            ""
        );
        assert_eq!(
            p.safe_eval_statements(r#" local_var; "#)
                .unwrap()
                .to_string()
                .as_str(),
            "\"local_value\""
        );
    }
}
