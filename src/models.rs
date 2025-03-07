use std::collections::{HashMap, HashSet};

// Type definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDefinition {
    pub name: String,           // Type name
    pub module_path: String,    // Module path
    pub visibility: VisibilityKind, // Visibility
    pub source_code: String,    // Source code of type definition
    pub file_path: String,      // File path
    pub constructors: Vec<String>, // Constructors and related impl blocks
}

impl TypeDefinition {
    // Check if constructors already contain implementations from other_constructors
    pub fn contains_impl(&self, other_constructors: &[String]) -> bool {
        if other_constructors.is_empty() {
            return true;
        }
        
        for other in other_constructors {
            let mut found = false;
            for constructor in &self.constructors {
                if constructor == other {
                    found = true;
                    break;
                }
            }
            if !found {
                return false;
            }
        }
        true
    }
}

// Function basic information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionInfo {
    pub name: String,
    pub module_path: String,
    pub visibility: VisibilityKind,
    pub has_internal_unsafe: bool,
    pub is_unsafe_fn: bool,
    pub file_path: String,
    pub source_code: String,
    pub param_custom_types: HashSet<String>, // Custom types used in function parameters
    pub return_custom_types: HashSet<String>, // Custom types used in function return
}

// Function visibility
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VisibilityKind {
    Public,       // pub fn
    Crate,        // pub(crate) fn
    Module,       // fn (private)
    Restricted,   // pub(in path) fn or pub(super) fn
}

impl VisibilityKind {
    // Convert visibility to string representation
    pub fn to_string(&self) -> String {
        match self {
            VisibilityKind::Public => "pub ".to_string(),
            VisibilityKind::Crate => "pub(crate) ".to_string(),
            VisibilityKind::Module => "".to_string(), // No prefix for private functions
            VisibilityKind::Restricted => "pub(restricted) ".to_string(),
        }
    }
    
    // Check if it's public visibility
    pub fn is_public(&self) -> bool {
        matches!(self, VisibilityKind::Public)
    }
}

// Function call relationship
#[derive(Debug)]
pub struct FunctionCall {
    pub caller: String,    // Full path of the caller
    pub callee: String,    // Full path of the callee
}

// Information for a single function in a path
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathNodeInfo {
    pub full_path: String,       // Full path of the function
    pub visibility: VisibilityKind, // Function visibility
    pub source_code: String,     // Function source code
    pub param_custom_types: HashSet<String>, // Custom types used in function parameters
    pub return_custom_types: HashSet<String>, // Custom types used in function return
    pub has_self_param: bool,    // 是否包含&self参数
}

// Analysis result for a single file
#[derive(Debug, Clone)]
pub struct FileAnalysisResult {
    pub file_path: String,
    pub paths: Vec<Vec<PathNodeInfo>>, // Modified to store detailed function info
    pub type_definitions: HashMap<String, TypeDefinition>, // Related custom type definitions
}