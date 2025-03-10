use std::collections::{HashMap, HashSet};
use syn::{
    visit::{self, Visit}, 
    ItemFn, Visibility, ExprUnsafe, ImplItemFn, Expr, ExprCall, ExprMethodCall, ExprPath,
    ExprUnary, UnOp, Pat, PathSegment,
    // Removed unused import: spanned::Spanned
};
use quote::ToTokens;

use crate::models::{FunctionInfo, TypeDefinition, VisibilityKind, UnsafeOperation, UnsafeOperationType};

/// Visitor for collecting function information and detecting unsafe blocks
pub struct FunctionVisitor {
    pub current_module_path: Vec<String>,
    pub functions: HashMap<String, FunctionInfo>,
    pub unsafe_functions: HashSet<String>,
    pub current_function: Option<String>,
    pub has_unsafe: bool,
    pub file_path: String,
    pub source_code: String,
    pub type_definitions: HashMap<String, TypeDefinition>, // Collected type definitions
    pub current_impl_type: Option<String>, // Current impl block's type name
    pub impl_blocks: HashMap<String, Vec<String>>, // Collection of complete impl blocks for each type
    pub in_unsafe_block: bool, // 是否在unsafe块内
    pub current_unsafe_operations: Vec<UnsafeOperation>, // 当前函数中的unsafe操作
    pub known_unsafe_functions: HashSet<String>, // 已知的unsafe函数列表
}

impl FunctionVisitor {
    pub fn new(file_path: String, source_code: String) -> Self {
        // 初始化已知的unsafe函数列表
        let mut known_unsafe_functions = HashSet::new();
        // 标准库中常见的unsafe函数
        known_unsafe_functions.insert("std::mem::transmute".to_string());
        known_unsafe_functions.insert("std::mem::transmute_copy".to_string());
        known_unsafe_functions.insert("ptr::read".to_string());
        known_unsafe_functions.insert("ptr::read_volatile".to_string());
        known_unsafe_functions.insert("ptr::write".to_string());
        known_unsafe_functions.insert("ptr::write_volatile".to_string());
        known_unsafe_functions.insert("ptr::copy".to_string());
        known_unsafe_functions.insert("ptr::copy_nonoverlapping".to_string());
        known_unsafe_functions.insert("std::ptr::drop_in_place".to_string());
        known_unsafe_functions.insert("from_raw_parts_mut".to_string());
        known_unsafe_functions.insert("slice::from_raw_parts_mut".to_string());
        known_unsafe_functions.insert("slice::from_raw_parts".to_string());
        known_unsafe_functions.insert("core::slice::from_raw_parts_mut".to_string());
        known_unsafe_functions.insert("core::slice::from_raw_parts".to_string());
        known_unsafe_functions.insert("from_utf8_unchecked".to_string());
        known_unsafe_functions.insert("from_utf8_unchecked_mut".to_string());
        
        FunctionVisitor {
            current_module_path: Vec::new(),
            functions: HashMap::new(),
            unsafe_functions: HashSet::new(),
            current_function: None,
            has_unsafe: false,
            file_path,
            source_code,
            type_definitions: HashMap::new(),
            current_impl_type: None,
            impl_blocks: HashMap::new(),
            in_unsafe_block: false,
            current_unsafe_operations: Vec::new(),
            known_unsafe_functions,
        }
    }
    
    /// Get current module path
    pub fn get_current_module_path(&self) -> String {
        self.current_module_path.join("::")
    }
    
    /// Convert from syn::Visibility to VisibilityKind
    pub fn convert_visibility(&self, vis: &Visibility) -> VisibilityKind {
        match vis {
            Visibility::Public(_) => VisibilityKind::Public,
            Visibility::Restricted(restricted) if restricted.path.is_ident("crate") => VisibilityKind::Crate,
            Visibility::Restricted(_) => VisibilityKind::Restricted,
            _ => VisibilityKind::Module,
        }
    }
    
    /// Add function to result set
    pub fn add_function(&mut self, name: String, vis: &Visibility, fn_item: &ItemFn) {
        let module_path = self.get_current_module_path();
        let full_path = if module_path.is_empty() {
            name.clone()
        } else {
            format!("{}::{}", module_path, name)
        };
        
        self.current_function = Some(full_path.clone());
        self.has_unsafe = false;
        
        // Extract function source code
        let source_code = fn_item.to_token_stream().to_string();
        
        // Check if function signature is declared unsafe
        let is_unsafe_fn = fn_item.sig.unsafety.is_some();
        
        // Analyze custom types used in function parameters and return
        let (param_types, return_types) = self.analyze_function_signature(&fn_item.sig);
        
        // 检测是否包含&self参数
        let has_self_param = fn_item.sig.inputs.iter().any(|arg| {
            match arg {
                syn::FnArg::Receiver(_) => true,
                syn::FnArg::Typed(pat_type) => {
                    if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                        return pat_ident.ident == "self";
                    }
                    false
                }
            }
        });
        
        // 确定函数所属的类型
        let owner_type = if has_self_param {
            // 尝试从module_path中推断类型
            let parts: Vec<&str> = module_path.split("::").collect();
            if !parts.is_empty() {
                Some(parts.last().unwrap().to_string())
            } else {
                None
            }
        } else {
            None
        };
        
        let info = FunctionInfo {
            name,
            module_path,
            visibility: self.convert_visibility(vis),
            has_internal_unsafe: false, // Updated later
            is_unsafe_fn,
            file_path: self.file_path.clone(),
            source_code,
            param_custom_types: param_types,
            return_custom_types: return_types,
            has_self_param,
            owner_type,
            unsafe_operations: Vec::new(),
        };
        
        self.functions.insert(full_path, info);
    }
    
    /// Extract source code from impl block function
    pub fn add_impl_function(&mut self, name: String, vis: &Visibility, impl_fn: &ImplItemFn) {
        let module_path = self.get_current_module_path();
        let full_path = if module_path.is_empty() {
            name.clone()
        } else {
            format!("{}::{}", module_path, name)
        };
        
        self.current_function = Some(full_path.clone());
        self.has_unsafe = false;
        
        // Extract function source code
        let source_code = impl_fn.to_token_stream().to_string();
        
        // Check if function signature is declared unsafe
        let is_unsafe_fn = impl_fn.sig.unsafety.is_some();
        
        // Analyze custom types used in function parameters and return
        let (mut param_types, return_types) = self.analyze_function_signature(&impl_fn.sig);
        
        // If method has self parameter and we know current impl type, add it to parameter types
        let has_self_param = impl_fn.sig.inputs.iter().any(|arg| matches!(arg, syn::FnArg::Receiver(_)));
        
        // 确定函数所属的类型
        let owner_type = if has_self_param {
            if let Some(impl_type) = &self.current_impl_type {
                // 如果在impl块中，我们知道类型
                param_types.insert(impl_type.clone());
                Some(impl_type.clone())
            } else {
                // 尝试从module_path中推断类型
                let parts: Vec<&str> = module_path.split("::").collect();
                if !parts.is_empty() {
                    Some(parts.last().unwrap().to_string())
                } else {
                    None
                }
            }
        } else {
            None
        };
        
        let info = FunctionInfo {
            name,
            module_path,
            visibility: self.convert_visibility(vis),
            has_internal_unsafe: false, // Updated later
            is_unsafe_fn,
            file_path: self.file_path.clone(),
            source_code,
            param_custom_types: param_types,
            return_custom_types: return_types,
            has_self_param,
            owner_type,
            unsafe_operations: Vec::new(),
        };
        
        self.functions.insert(full_path, info);
    }
    
    /// Add type definition to result set
    pub fn add_type_definition<T: ToTokens>(&mut self, name: String, vis: &Visibility, type_item: &T) {
        let module_path = self.get_current_module_path();
        let full_path = if module_path.is_empty() {
            name.clone()
        } else {
            format!("{}::{}", module_path, name)
        };
        
        // Extract source code of type definition
        let source_code = type_item.to_token_stream().to_string();
        
        let definition = TypeDefinition {
            name,
            module_path,
            visibility: self.convert_visibility(vis),
            source_code,
            file_path: self.file_path.clone(),
            constructors: Vec::new(), // Initialize as empty list
        };
        
        self.type_definitions.insert(full_path, definition);
    }

    /// Check if a method is a constructor for the given type
    pub fn is_constructor(&self, method: &syn::ImplItemFn, type_name: &str) -> bool {
        if let syn::ReturnType::Type(_, ty) = &method.sig.output {
            match &**ty {
                syn::Type::Path(type_path) => {
                    if let Some(segment) = type_path.path.segments.last() {
                        let return_type = segment.ident.to_string();
                        return return_type == "Self" || return_type == type_name;
                    }
                },
                // Handle reference types, like &mut Self
                syn::Type::Reference(type_ref) => {
                    if let syn::Type::Path(type_path) = &*type_ref.elem {
                        if let Some(segment) = type_path.path.segments.last() {
                            let return_type = segment.ident.to_string();
                            return return_type == "Self" || return_type == type_name;
                        }
                    }
                },
                _ => {}
            }
        }
        false
    }
    
    /// Analyze custom types used in function signature, return parameters and return types separately
    pub fn analyze_function_signature(&self, sig: &syn::Signature) -> (HashSet<String>, HashSet<String>) {
        let mut param_types = HashSet::new();
        let mut return_types = HashSet::new();
        
        // Analyze function parameters
        for param in &sig.inputs {
            if let syn::FnArg::Typed(pat_type) = param {
                self.extract_custom_types(&pat_type.ty, &mut param_types);
            }
        }
        
        // Analyze return type
        if let syn::ReturnType::Type(_, ty) = &sig.output {
            self.extract_custom_types(ty, &mut return_types);
        }
        
        (param_types, return_types)
    }
    
    /// Extract custom types from type
    pub fn extract_custom_types(&self, ty: &syn::Type, result: &mut HashSet<String>) {
        match ty {
            syn::Type::Path(type_path) if !self.is_primitive_type(&type_path.path) => {
                // Extract type name from path
                if let Some(segment) = type_path.path.segments.last() {
                    let type_name = segment.ident.to_string();
                    result.insert(type_name);
                    
                    // Recursively process generic parameters
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        for arg in &args.args {
                            if let syn::GenericArgument::Type(inner_ty) = arg {
                                self.extract_custom_types(inner_ty, result);
                            }
                        }
                    }
                }
            },
            syn::Type::Reference(type_ref) => {
                // Handle reference types
                self.extract_custom_types(&type_ref.elem, result);
            },
            syn::Type::Array(type_array) => {
                // Handle array types
                self.extract_custom_types(&type_array.elem, result);
            },
            syn::Type::Slice(type_slice) => {
                // Handle slice types
                self.extract_custom_types(&type_slice.elem, result);
            },
            syn::Type::Tuple(type_tuple) => {
                // Handle tuple types
                for elem in &type_tuple.elems {
                    self.extract_custom_types(elem, result);
                }
            },
            _ => {}
        }
    }
    
    /// Check if it's a Rust primitive type or standard library type
    pub fn is_primitive_type(&self, path: &syn::Path) -> bool {
        if path.segments.len() != 1 {
            return false;
        }
        
        let type_name = path.segments[0].ident.to_string();
        matches!(type_name.as_str(), 
            // Primitive types
            "bool" | "char" | "str" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" |
            "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "f32" | "f64" |
            // Common standard library types
            "String" | "Vec" | "Option" | "Result" | "Box" | "Rc" | "Arc" | "Cell" | "RefCell" |
            "HashMap" | "HashSet" | "BTreeMap" | "BTreeSet" | "VecDeque" | "LinkedList" |
            "Mutex" | "RwLock" | "Condvar" | "Once" | "Thread" | "Duration" | "Instant" |
            "SystemTime" | "Path" | "PathBuf"
        )
    }
    
    /// 检测是否是已知的unsafe函数
    pub fn is_known_unsafe_function(&self, path: &str) -> bool {
        self.known_unsafe_functions.contains(path)
    }
    
    /// 检查函数名是否包含unsafe关键词
    pub fn has_unsafe_keywords(&self, name: &str) -> bool {
        name.contains("unsafe") || 
        name.contains("unchecked") || 
        name.contains("as_ptr") || 
        name.contains("as_mut_ptr") || 
        name == "assume_init" || 
        name == "set_len" ||
        name.contains("transmute") ||
        name.contains("from_raw_parts")
    }
    
    /// 记录unsafe操作
    pub fn record_unsafe_operation(&mut self, op_type: UnsafeOperationType, description: String, code_snippet: String) {
        // 只有在有当前函数的情况下才记录
        if let Some(current_fn) = &self.current_function {
            // 检查是否已存在相同的操作（相同代码片段）
            for op in &self.current_unsafe_operations {
                if op.code_snippet == code_snippet {
                    return; // 跳过重复的操作
                }
            }
            
            // 简化描述，去除冗余信息
            let simplified_description = match &op_type {
                UnsafeOperationType::RawPointerDereference => "裸指针操作".to_string(),
                UnsafeOperationType::UnsafeFunctionCall => "调用unsafe函数".to_string(),
                UnsafeOperationType::UnsafeMethodCall => "调用unsafe方法".to_string(),
                UnsafeOperationType::InlineAssembly => "内联汇编".to_string(),
                UnsafeOperationType::UnionFieldAccess => "访问联合体字段".to_string(),
                UnsafeOperationType::MutStaticAccess => "访问可变静态变量".to_string(),
                UnsafeOperationType::Other(desc) => desc.clone(),
            };
            
            let operation = UnsafeOperation {
                operation_type: op_type,
                description: simplified_description,
                code_snippet,
                line_number: None,
            };
            
            // 先将操作添加到当前函数中
            if let Some(fn_info) = self.functions.get_mut(current_fn) {
                fn_info.unsafe_operations.push(operation.clone());
            }
            
            // 再添加到当前收集的操作列表中
            self.current_unsafe_operations.push(operation);
        }
    }
    
    /// 检测是否是裸指针类型
    pub fn is_raw_pointer_type(&self, ty: &syn::Type) -> bool {
        match ty {
            syn::Type::Ptr(_) => true,
            _ => false,
        }
    }
    
    /// 检测expr是否可能是裸指针
    pub fn might_be_raw_pointer(&self, expr: &Expr) -> bool {
        match expr {
            // 检查指针类型转换，如：expr as *const T 或 expr as *mut T
            Expr::Cast(expr_cast) => {
                match &*expr_cast.ty {
                    syn::Type::Ptr(_) => true,
                    _ => false,
                }
            },
            // 检查路径表达式，如：ptr
            Expr::Path(expr_path) => {
                let path_str = expr_path.to_token_stream().to_string();
                path_str.contains("ptr") || 
                path_str.contains("raw") || 
                path_str.contains("pointer")
            },
            // 检查方法调用结果，如：ptr.add(1)
            Expr::MethodCall(method_call) => {
                let method_name = method_call.method.to_string();
                method_name == "add" || 
                method_name == "offset" || 
                method_name == "as_ptr" || 
                method_name == "as_mut_ptr"
            },
            _ => false,
        }
    }
    
    /// 更新unsafe状态
    pub fn update_unsafe_state(&mut self) {
        if let Some(name) = &self.current_function {
            if let Some(func) = self.functions.get_mut(name) {
                func.has_internal_unsafe = self.has_unsafe;
                
                // 如果有unsafe块或调用，将函数添加到unsafe函数集合中
                if self.has_unsafe || func.is_unsafe_fn {
                    self.unsafe_functions.insert(name.clone());
                }
                
                // 更新函数的unsafe操作列表
                func.unsafe_operations = self.current_unsafe_operations.clone();
            }
        }
        
        // 重置当前unsafe操作列表
        self.current_unsafe_operations.clear();
    }

    // 判断函数调用是否是常见的unsafe操作
    pub fn is_common_unsafe_operation(&self, func_path: &str) -> Option<UnsafeOperationType> {
        // 常见的unsafe函数名列表
        let common_unsafe_funcs = [
            // 裸指针相关操作
            ("from_raw_parts", UnsafeOperationType::RawPointerDereference),
            ("from_raw_parts_mut", UnsafeOperationType::RawPointerDereference),
            ("copy_nonoverlapping", UnsafeOperationType::RawPointerDereference),
            ("copy", UnsafeOperationType::RawPointerDereference),
            ("write", UnsafeOperationType::RawPointerDereference),
            ("read", UnsafeOperationType::RawPointerDereference),
            ("offset", UnsafeOperationType::RawPointerDereference),
            ("add", UnsafeOperationType::RawPointerDereference),
            ("drop_in_place", UnsafeOperationType::RawPointerDereference),
            ("slice_from_raw_parts", UnsafeOperationType::RawPointerDereference),
            ("slice_from_raw_parts_mut", UnsafeOperationType::RawPointerDereference),
            
            // 内存相关
            ("transmute", UnsafeOperationType::Other("内存转换".to_string())),
            ("forget", UnsafeOperationType::Other("内存忽略".to_string())),
            ("zeroed", UnsafeOperationType::Other("零初始化".to_string())),
            ("uninitialized", UnsafeOperationType::Other("未初始化内存".to_string())),
            
            // 其他常见unsafe操作
            ("set_len", UnsafeOperationType::UnsafeMethodCall),
            ("as_ptr", UnsafeOperationType::UnsafeMethodCall),
            ("as_mut_ptr", UnsafeOperationType::UnsafeMethodCall),
            ("from_utf8_unchecked", UnsafeOperationType::UnsafeMethodCall),
            ("from_utf8_unchecked_mut", UnsafeOperationType::UnsafeMethodCall),
        ];
        
        // 1. 先检查完整的函数调用路径
        for (keyword, op_type) in common_unsafe_funcs.iter() {
            if func_path.contains(keyword) {
                return Some(op_type.clone());
            }
        }
        
        // 2. 如果没有匹配到完整路径，检查路径的最后一部分
        if let Some(last_part) = func_path.split("::").last() {
            for (keyword, op_type) in common_unsafe_funcs.iter() {
                if last_part == *keyword {
                    return Some(op_type.clone());
                }
            }
        }
        
        None
    }

    /// 检查完整或部分函数路径是否是已知的unsafe函数
    pub fn is_known_unsafe_full_path(&self, segments: &[String]) -> bool {
        if segments.is_empty() {
            return false;
        }
        
        // 单独函数名检测 - 无论在什么路径下，这些函数都被视为unsafe
        let unsafe_function_names = [
            "from_raw_parts",
            "from_raw_parts_mut",
            "copy_nonoverlapping",
            "transmute",
            "forget",
            "offset",
            "from_utf8_unchecked",
            "from_utf8_unchecked_mut",
            "drop_in_place",
        ];
        
        // 检查最后的函数名是否是已知的unsafe函数
        let last_segment = &segments[segments.len() - 1];
        if unsafe_function_names.contains(&last_segment.as_str()) {
            // 如果函数名是已知unsafe函数，再确认所在的上下文是否匹配
            // 例如，如果函数名是"from_raw_parts"，我们需要检查它是否在slice模块中
            
            // from_raw_parts/from_raw_parts_mut: 检查是否来自slice模块
            if (last_segment == "from_raw_parts" || last_segment == "from_raw_parts_mut") && segments.len() > 1 {
                let prev_segment = &segments[segments.len() - 2];
                // 如果前一个部分不是slice，可能是其他模块的同名函数，不一定是unsafe
                if prev_segment != "slice" && prev_segment != "std" && prev_segment != "core" {
                    // 但作为保守策略，我们仍然认为它是unsafe的
                    return true;
                }
            }
            
            // copy/copy_nonoverlapping/read/write: 检查是否来自ptr模块
            if (last_segment == "copy" || last_segment == "copy_nonoverlapping" || last_segment == "read" || last_segment == "write") && segments.len() > 1 {
                let prev_segment = &segments[segments.len() - 2];
                // 如果前一个部分不是ptr，可能是其他模块的同名函数，不一定是unsafe
                if prev_segment != "ptr" && prev_segment != "std" && prev_segment != "core" {
                    // 允许其他unsafe调用也能被检测到
                    return true;
                }
            }
            
            // transmute/forget: 检查是否来自mem模块
            if (last_segment == "transmute" || last_segment == "forget") && segments.len() > 1 {
                let prev_segment = &segments[segments.len() - 2];
                // 如果前一个部分不是mem，可能是其他模块的同名函数，不一定是unsafe
                if prev_segment != "mem" && prev_segment != "std" && prev_segment != "core" {
                    // 允许其他unsafe调用也能被检测到
                    return true;
                }
            }
            
            // 保守策略：即使没有正确的上下文，仍然认为它可能是unsafe
            return true;
        }
        
        // 常见的unsafe函数完整路径
        let known_unsafe_paths = [
            vec!["core", "slice", "from_raw_parts"],
            vec!["core", "slice", "from_raw_parts_mut"],
            vec!["std", "slice", "from_raw_parts"],
            vec!["std", "slice", "from_raw_parts_mut"],
            vec!["slice", "from_raw_parts"],
            vec!["slice", "from_raw_parts_mut"],
            vec!["core", "ptr", "read"],
            vec!["core", "ptr", "write"],
            vec!["core", "ptr", "copy"],
            vec!["core", "ptr", "copy_nonoverlapping"],
            vec!["std", "ptr", "read"],
            vec!["std", "ptr", "write"],
            vec!["std", "ptr", "copy"],
            vec!["std", "ptr", "copy_nonoverlapping"],
            vec!["std", "mem", "transmute"],
            vec!["core", "mem", "transmute"],
            vec!["core", "mem", "forget"],
            vec!["std", "mem", "forget"],
        ];
        
        // 检查是否匹配任何已知的unsafe函数路径
        for path in &known_unsafe_paths {
            if path.len() <= segments.len() {
                let start_idx = segments.len() - path.len();
                let matching = segments[start_idx..].iter().zip(path.iter()).all(|(a, b)| a == b);
                if matching {
                    return true;
                }
            }
        }
        
        false
    }
}

impl<'ast> Visit<'ast> for FunctionVisitor {
    /// Visit module
    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        self.current_module_path.push(i.ident.to_string());
        
        // Visit module contents
        if let Some((_, items)) = &i.content {
            for item in items {
                visit::visit_item(self, item);
            }
        }
        
        self.current_module_path.pop();
    }
    
    /// Visit function definition
    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        let name = i.sig.ident.to_string();
        self.add_function(name, &i.vis, i);
        
        // Visit function body
        visit::visit_block(self, &i.block);
        
        // Update unsafe state
        self.update_unsafe_state();
        self.current_function = None;
    }
    
    /// Visit function in impl block
    fn visit_impl_item_fn(&mut self, i: &'ast ImplItemFn) {
        let name = i.sig.ident.to_string();
        self.add_impl_function(name, &i.vis, i);
        
        // Visit function body
        visit::visit_block(self, &i.block);
        
        // Update unsafe state
        self.update_unsafe_state();
        self.current_function = None;
    }
    
    /// Visit unsafe block
    fn visit_expr_unsafe(&mut self, i: &'ast ExprUnsafe) {
        self.has_unsafe = true;
        
        // 标记进入unsafe块
        let prev_in_unsafe = self.in_unsafe_block;
        self.in_unsafe_block = true;
        
        // Continue visiting inside unsafe block
        visit::visit_expr_unsafe(self, i);
        
        // 恢复之前的状态
        self.in_unsafe_block = prev_in_unsafe;
    }
    
    /// Visit struct definition
    fn visit_item_struct(&mut self, i: &'ast syn::ItemStruct) {
        let name = i.ident.to_string();
        self.add_type_definition(name, &i.vis, i);
        visit::visit_item_struct(self, i);
    }
    
    /// Visit enum definition
    fn visit_item_enum(&mut self, i: &'ast syn::ItemEnum) {
        let name = i.ident.to_string();
        self.add_type_definition(name, &i.vis, i);
        visit::visit_item_enum(self, i);
    }
    
    /// Visit type alias
    fn visit_item_type(&mut self, i: &'ast syn::ItemType) {
        let name = i.ident.to_string();
        self.add_type_definition(name, &i.vis, i);
        visit::visit_item_type(self, i);
    }

    /// Visit impl block
    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        // Extract type name for impl block
        let type_name = match &*i.self_ty {
            syn::Type::Path(type_path) => {
                if let Some(segment) = type_path.path.segments.last() {
                    segment.ident.to_string()
                } else {
                    // Skip if can't get type name
                    return;
                }
            },
            _ => return, // Not a path type, skip
        };
        
        // Set current impl type for use when processing methods
        self.current_impl_type = Some(type_name.clone());
        
        // Save complete impl block code
        let impl_code = i.to_token_stream().to_string();
        self.impl_blocks.entry(type_name.clone())
            .or_insert_with(Vec::new)
            .push(impl_code.clone());
        
        // Check if it's a Default trait implementation
        let is_default_impl = if let Some((_, trait_path, _)) = &i.trait_ {
            trait_path.segments.last()
                .map(|seg| seg.ident.to_string() == "Default")
                .unwrap_or(false)
        } else {
            false
        };
        
        // If it's a Default implementation, add the entire impl block as a constructor
        if is_default_impl {
            // Find matching type definition and add constructor
            for (path, def) in &mut self.type_definitions {
                if let Some(def_name) = path.split("::").last() {
                    if def_name == &type_name {
                        def.constructors.push(impl_code.clone());
                    }
                }
            }
        } else {
            // For non-Default implementations, only extract constructor methods
            for item in &i.items {
                if let syn::ImplItem::Fn(method) = item {
                    if self.is_constructor(method, &type_name) {
                        // Check if function is unsafe
                        let is_unsafe = method.sig.unsafety.is_some();
                        
                        // Only add safe constructors
                        if !is_unsafe {
                            // Only extract this constructor method
                            let method_code = format!("impl {} {{\n    {}\n}}", 
                                type_name, 
                                method.to_token_stream().to_string());
                            
                            // Find matching type definition and add constructor
                            for (path, def) in &mut self.type_definitions {
                                if let Some(def_name) = path.split("::").last() {
                                    if def_name == &type_name {
                                        def.constructors.push(method_code.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Continue visiting impl block contents
        visit::visit_item_impl(self, i);
        
        // Clear current impl type
        self.current_impl_type = None;
    }
    
    /// 检测裸指针解引用
    fn visit_expr_unary(&mut self, i: &'ast ExprUnary) {
        // 检查是否是解引用操作 (*expr)
        if matches!(i.op, UnOp::Deref(_)) {
            // 只有在unsafe块内的解引用才可能是危险的
            if self.in_unsafe_block {
                // 检查被解引用的表达式是否可能是裸指针
                if self.might_be_raw_pointer(&i.expr) {
                    // 排除明显的安全模式：&*expr（引用重借用）
                    let expr_str = i.to_token_stream().to_string();
                    if !expr_str.starts_with("& *") {
                        self.record_unsafe_operation(
                            UnsafeOperationType::RawPointerDereference,
                            "解引用裸指针".to_string(),
                            expr_str
                        );
                    }
                }
            }
        }
        
        // 继续访问子表达式
        visit::visit_expr_unary(self, i);
    }
    
    /// 检测函数调用，可能是unsafe函数调用
    fn visit_expr_call(&mut self, i: &'ast ExprCall) {
        if let Some(_current_function) = &self.current_function {
            let code_snippet = i.to_token_stream().to_string();
            
            // 检查是否调用unsafe函数
            if let Expr::Path(path) = &*i.func {
                let path_str = path.to_token_stream().to_string();
                
                // 检查是否是完整路径的unsafe函数 (例如 core::slice::from_raw_parts)
                let segments: Vec<String> = path.path.segments.iter()
                    .map(|seg| seg.ident.to_string())
                    .collect();
                
                if self.is_known_unsafe_full_path(&segments) {
                    self.record_unsafe_operation(
                        UnsafeOperationType::UnsafeFunctionCall,
                        format!("调用unsafe函数: {}", path_str),
                        code_snippet.clone()
                    );
                }
                // 检查是否是已知的unsafe函数
                else if self.is_known_unsafe_function(&path_str) {
                    self.record_unsafe_operation(
                        UnsafeOperationType::UnsafeFunctionCall,
                        format!("调用unsafe函数: {}", path_str),
                        code_snippet.clone()
                    );
                }
                
                // 检查是否是常见的unsafe操作
                if let Some(op_type) = self.is_common_unsafe_operation(&path_str) {
                    self.record_unsafe_operation(
                        op_type,
                        format!("调用unsafe操作: {}", path_str),
                        code_snippet
                    );
                }
            }
        }
        
        // 继续访问子表达式
        visit::visit_expr_call(self, i);
    }
    
    /// 检测方法调用，可能是unsafe方法调用
    fn visit_expr_method_call(&mut self, i: &'ast ExprMethodCall) {
        if let Some(current_function) = &self.current_function {
            let method_name = i.method.to_string();
            let code_snippet = i.to_token_stream().to_string();
            
            // 检查是否是已知的unsafe方法
            if self.has_unsafe_keywords(&method_name) {
                self.record_unsafe_operation(
                    UnsafeOperationType::UnsafeMethodCall,
                    format!("调用unsafe方法: {}", method_name),
                    code_snippet.clone()
                );
            }
            
            // 检查是否是常见的unsafe操作
            if let Some(op_type) = self.is_common_unsafe_operation(&method_name) {
                self.record_unsafe_operation(
                    op_type,
                    format!("调用unsafe操作: {}", method_name),
                    code_snippet
                );
            }
        }
        
        // 继续访问子表达式
        visit::visit_expr_method_call(self, i);
    }
}