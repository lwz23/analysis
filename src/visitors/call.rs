use std::collections::HashMap;
use syn::{
    visit::{self, Visit},
    UseTree,
};
use quote::ToTokens;

use crate::models::FunctionCall;

/// Visitor for collecting function call relationships
pub struct CallVisitor {
    pub current_module_path: Vec<String>,
    pub current_function: Option<String>,
    pub calls: Vec<FunctionCall>,
    // Mapping of imported modules and aliases
    pub imports: HashMap<String, String>,
}

impl CallVisitor {
    pub fn new() -> Self {
        CallVisitor {
            current_module_path: Vec::new(),
            current_function: None,
            calls: Vec::new(),
            imports: HashMap::new(),
        }
    }
    
    /// Get current module path
    pub fn get_current_module_path(&self) -> String {
        self.current_module_path.join("::")
    }
    
    /// Handle function call expression
    pub fn handle_call(&mut self, func_path: &syn::Path) {
        if let Some(ref caller) = self.current_function {
            let callee = self.resolve_path(func_path);
            self.calls.push(FunctionCall {
                caller: caller.clone(),
                callee,
            });
        }
    }
    
    /// Resolve path, handling imports and aliases
    pub fn resolve_path(&self, path: &syn::Path) -> String {
        let path_str = path.to_token_stream().to_string().replace(' ', "");
        
        // Check if it's an imported module or alias
        if path.segments.len() > 0 {
            let first_segment = &path.segments[0].ident.to_string();
            if let Some(import) = self.imports.get(first_segment) {
                // Replace first part of path with full imported path
                return path_str.replacen(first_segment, import, 1);
            }
        }
        
        // If it's a relative path (not starting with crate:: or ::), add current module path
        if !path_str.starts_with("crate::") && !path_str.starts_with("::") {
            let module_path = self.get_current_module_path();
            if !module_path.is_empty() {
                return format!("{}::{}", module_path, path_str);
            }
        }
        
        path_str
    }
    
    /// Process import statement
    pub fn process_use(&mut self, use_tree: &UseTree, prefix: &str) {
        match use_tree {
            UseTree::Path(use_path) => {
                let next_prefix = if prefix.is_empty() {
                    use_path.ident.to_string()
                } else {
                    format!("{}::{}", prefix, use_path.ident)
                };
                self.process_use(&*use_path.tree, &next_prefix);
            },
            UseTree::Name(use_name) => {
                let full_path = if prefix.is_empty() {
                    use_name.ident.to_string()
                } else {
                    format!("{}::{}", prefix, use_name.ident)
                };
                // Add to import mapping
                self.imports.insert(use_name.ident.to_string(), full_path);
            },
            UseTree::Rename(use_rename) => {
                let full_path = if prefix.is_empty() {
                    use_rename.ident.to_string()
                } else {
                    format!("{}::{}", prefix, use_rename.ident)
                };
                // Add alias to import mapping
                self.imports.insert(use_rename.rename.to_string(), full_path);
            },
            UseTree::Glob(_) => {
                // Handling wildcard imports is complex, simplified here
            },
            UseTree::Group(use_group) => {
                for tree in &use_group.items {
                    self.process_use(tree, prefix);
                }
            },
        }
    }
}

impl<'ast> Visit<'ast> for CallVisitor {
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
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        let name = i.sig.ident.to_string();
        let module_path = self.get_current_module_path();
        let full_path = if module_path.is_empty() {
            name
        } else {
            format!("{}::{}", module_path, name)
        };
        
        self.current_function = Some(full_path);
        
        // Visit function body
        visit::visit_block(self, &i.block);
        
        self.current_function = None;
    }
    
    /// Visit function in impl block
    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        let name = i.sig.ident.to_string();
        let module_path = self.get_current_module_path();
        let full_path = if module_path.is_empty() {
            name
        } else {
            format!("{}::{}", module_path, name)
        };
        
        self.current_function = Some(full_path);
        
        // Visit function body
        visit::visit_block(self, &i.block);
        
        self.current_function = None;
    }
    
    /// Visit function call expression
    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        // Check if called expression is a path (function name)
        if let syn::Expr::Path(expr_path) = &*i.func {
            self.handle_call(&expr_path.path);
        }
        
        // Continue visiting arguments
        for arg in &i.args {
            visit::visit_expr(self, arg);
        }
    }
    
    /// Visit method call expression
    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        // Method calls are more complex, need type information for accurate resolution
        // Simplified handling, just record method name
        if let Some(ref caller) = self.current_function {
            let method_name = i.method.to_string();
            // Simplification: assume method is defined in current module
            let module_path = self.get_current_module_path();
            let callee = if module_path.is_empty() {
                method_name
            } else {
                format!("{}::{}", module_path, method_name)
            };
            
            self.calls.push(FunctionCall {
                caller: caller.clone(),
                callee,
            });
        }
        
        // Continue visiting receiver and arguments
        visit::visit_expr(self, &i.receiver);
        for arg in &i.args {
            visit::visit_expr(self, arg);
        }
    }
    
    /// Visit import statement
    fn visit_item_use(&mut self, i: &'ast syn::ItemUse) {
        self.process_use(&i.tree, "");
        visit::visit_item_use(self, i);
    }
}