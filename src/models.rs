use std::collections::{HashMap, HashSet};

// Unsafe operation type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsafeOperationType {
    RawPointerDereference,  // 裸指针解引用
    UnsafeFunctionCall,     // 调用unsafe函数
    UnsafeMethodCall,       // 调用unsafe方法
    InlineAssembly,         // 内联汇编
    UnionFieldAccess,       // 访问联合体字段
    MutStaticAccess,        // 访问可变静态变量
    DirectParamToUnsafe,    // 参数直接传递给unsafe操作
    Other(String),          // 其他类型的unsafe操作
}

impl UnsafeOperationType {
    pub fn to_string(&self) -> String {
        match self {
            UnsafeOperationType::RawPointerDereference => "裸指针解引用".to_string(),
            UnsafeOperationType::UnsafeFunctionCall => "调用unsafe函数".to_string(),
            UnsafeOperationType::UnsafeMethodCall => "调用unsafe方法".to_string(),
            UnsafeOperationType::InlineAssembly => "内联汇编".to_string(),
            UnsafeOperationType::UnionFieldAccess => "访问联合体字段".to_string(),
            UnsafeOperationType::MutStaticAccess => "访问可变静态变量".to_string(),
            UnsafeOperationType::DirectParamToUnsafe => "参数直接传递给unsafe操作".to_string(),
            UnsafeOperationType::Other(desc) => format!("其他unsafe操作: {}", desc),
        }
    }
}

// Detailed information about an unsafe operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsafeOperation {
    pub operation_type: UnsafeOperationType,  // 操作类型
    pub description: String,                  // 描述文本
    pub code_snippet: String,                 // 代码片段
    pub line_number: Option<usize>,           // 行号（可选）
}

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
    pub has_self_param: bool, // 是否包含&self参数
    pub owner_type: Option<String>, // 函数所属的类型
    pub unsafe_operations: Vec<UnsafeOperation>, // Unsafe operations in this function
    pub param_names: HashSet<String>, // 函数参数名称集合
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
    pub owner_type: Option<String>, // 函数所属的类型名称
    pub unsafe_operations: Vec<UnsafeOperation>, // Unsafe operations in this function
}

// Analysis result for a single file
#[derive(Debug, Clone)]
pub struct FileAnalysisResult {
    pub file_path: String,
    pub paths: Vec<Vec<PathNodeInfo>>, // Modified to store detailed function info
    pub type_definitions: HashMap<String, TypeDefinition>, // Related custom type definitions
    pub direct_param_paths: Vec<Vec<PathNodeInfo>>, // 模式1: 直接参数传递到unsafe操作的路径
}