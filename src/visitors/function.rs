use std::collections::{HashMap, HashSet};
use syn::{
    visit::{self, Visit}, 
    ItemFn, Visibility, ExprUnsafe, ImplItemFn,
    // Removed unused import: spanned::Spanned
};
use quote::ToTokens;

use crate::models::{FunctionInfo, TypeDefinition, VisibilityKind};

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
}

impl FunctionVisitor {
    pub fn new(file_path: String, source_code: String) -> Self {
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
        if impl_fn.sig.inputs.iter().any(|arg| matches!(arg, syn::FnArg::Receiver(_))) {
            if let Some(impl_type) = &self.current_impl_type {
                param_types.insert(impl_type.clone());
            }
        }
        
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
    
    /// Update current function's unsafe state
    pub fn update_unsafe_state(&mut self) {
        if let Some(ref func_path) = self.current_function {
            if self.has_unsafe {
                if let Some(info) = self.functions.get_mut(func_path) {
                    info.has_internal_unsafe = true;
                    self.unsafe_functions.insert(func_path.clone());
                }
            }
        }
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
        
        // Continue visiting inside unsafe block
        visit::visit_expr_unsafe(self, i);
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
}