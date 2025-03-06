use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use syn::{
    visit::{self, Visit}, ItemFn, Visibility, ExprUnsafe, ImplItemFn, UseTree, 
    spanned::Spanned,
};
use quote::ToTokens;
use walkdir::WalkDir;
use rayon::prelude::*;

// 表示自定义类型的定义
#[derive(Debug, Clone, PartialEq, Eq)]
struct TypeDefinition {
    name: String,           // 类型名称
    module_path: String,    // 模块路径
    visibility: VisibilityKind, // 可见性
    source_code: String,    // 类型定义的源代码
    file_path: String,      // 文件路径
    constructors: Vec<String>, // 构造函数和相关impl块
}

// 表示函数的基本信息
#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionInfo {
    name: String,
    module_path: String,
    visibility: VisibilityKind,
    has_internal_unsafe: bool,
    is_unsafe_fn: bool,
    file_path: String,
    source_code: String,
    param_custom_types: HashSet<String>, // 函数参数中使用的自定义类型
    return_custom_types: HashSet<String>, // 函数返回值中使用的自定义类型
}

// 表示函数的可见性
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum VisibilityKind {
    Public,       // pub fn
    Crate,        // pub(crate) fn
    Module,       // fn (私有)
    Restricted,   // pub(in path) fn 或 pub(super) fn
}

impl VisibilityKind {
    // 将可见性转换为字符串表示
    fn to_string(&self) -> String {
        match self {
            VisibilityKind::Public => "pub ".to_string(),
            VisibilityKind::Crate => "pub(crate) ".to_string(),
            VisibilityKind::Module => "".to_string(), // 私有函数不添加前缀
            VisibilityKind::Restricted => "pub(restricted) ".to_string(),
        }
    }
    
    // 检查是否为公开可见性
    fn is_public(&self) -> bool {
        matches!(self, VisibilityKind::Public)
    }
}

// 函数调用关系
#[derive(Debug)]
struct FunctionCall {
    caller: String,    // 调用者的完整路径
    callee: String,    // 被调用者的完整路径
}

// 路径中单个函数的信息
#[derive(Debug, Clone)]
struct PathNodeInfo {
    full_path: String,       // 函数的完整路径
    visibility: VisibilityKind, // 函数的可见性
    source_code: String,     // 函数源代码
    param_custom_types: HashSet<String>, // 函数参数中使用的自定义类型
    return_custom_types: HashSet<String>, // 函数返回值中使用的自定义类型
}

// 单个文件的分析结果
#[derive(Debug, Clone)]
struct FileAnalysisResult {
    file_path: String,
    paths: Vec<Vec<PathNodeInfo>>, // 修改为存储函数详细信息
    type_definitions: HashMap<String, TypeDefinition>, // 相关的自定义类型定义
}

// 函数调用图
struct CallGraph {
    // 从函数完整路径到函数信息的映射
    functions: HashMap<String, FunctionInfo>,
    // 从调用者到被调用者的映射
    calls: HashMap<String, HashSet<String>>,
    // 从被调用者到调用者的映射（反向图）
    reverse_calls: HashMap<String, HashSet<String>>,
    // 包含内部不安全代码的函数
    unsafe_functions: HashSet<String>,
    // 公开函数
    public_functions: HashSet<String>,
    // 公开且不安全的函数
    public_unsafe_functions: HashSet<String>,
    // 公开且非unsafe声明的函数
    public_non_unsafe_functions: HashSet<String>,
    // 最大搜索深度
    max_search_depth: usize,
    // 函数路径到其参数中使用的自定义类型的映射
    param_custom_types: HashMap<String, HashSet<String>>,
    // 函数路径到其返回值中使用的自定义类型的映射
    return_custom_types: HashMap<String, HashSet<String>>,
}

impl CallGraph {
    fn new(max_depth: usize) -> Self {
        CallGraph {
            functions: HashMap::new(),
            calls: HashMap::new(),
            reverse_calls: HashMap::new(),
            unsafe_functions: HashSet::new(),
            public_functions: HashSet::new(),
            public_unsafe_functions: HashSet::new(),
            public_non_unsafe_functions: HashSet::new(),
            max_search_depth: max_depth,
            param_custom_types: HashMap::new(),
            return_custom_types: HashMap::new(),
        }
    }

    // 添加函数到图中
    fn add_function(&mut self, path: String, info: FunctionInfo) {
        if info.visibility == VisibilityKind::Public {
            self.public_functions.insert(path.clone());
            
            // 如果函数是公开的且不是unsafe声明的，加入public_non_unsafe_functions集合
            if !info.is_unsafe_fn {
                self.public_non_unsafe_functions.insert(path.clone());
            }
            
            // 如果函数既是公开的又包含不安全代码，添加到新集合中
            if info.has_internal_unsafe {
                self.public_unsafe_functions.insert(path.clone());
            }
        }
        
        if info.has_internal_unsafe {
            self.unsafe_functions.insert(path.clone());
        }
        
        // 保存函数参数和返回值使用的自定义类型
        if !info.param_custom_types.is_empty() {
            self.param_custom_types.insert(path.clone(), info.param_custom_types.clone());
        }
        
        if !info.return_custom_types.is_empty() {
            self.return_custom_types.insert(path.clone(), info.return_custom_types.clone());
        }
        
        self.functions.insert(path, info);
    }

    // 添加函数调用关系
    fn add_call(&mut self, caller: String, callee: String) {
        self.calls.entry(caller.clone()).or_insert_with(HashSet::new).insert(callee.clone());
        self.reverse_calls.entry(callee).or_insert_with(HashSet::new).insert(caller);
    }

    // 检查路径是否有效，使用public_non_unsafe_functions替代public_functions作为第一个节点的检查
    fn is_valid_path(&self, path: &[String]) -> bool {
        // 检查路径长度必须大于1
        if path.len() <= 1 {
            return false;
        }
        
        // 检查第一个节点必须是public且非unsafe声明的函数
        if !self.public_non_unsafe_functions.contains(&path[0]) {
            return false;
        }
        
        // 检查最后一个节点必须是内部unsafe函数
        if !self.unsafe_functions.contains(&path[path.len() - 1]) {
            return false;
        }
        
        // 检查中间节点不能是unsafe函数或公开的不安全函数
        for i in 1..path.len() - 1 {
            if self.unsafe_functions.contains(&path[i]) || self.public_unsafe_functions.contains(&path[i]) {
                return false;
            }
        }
        
        true
    }
    
    // 检查路径是否是最小路径（除起始节点外，没有其他公开函数）
    fn is_minimal_path(&self, path: &[String]) -> bool {
        // 跳过第一个节点，检查后续节点是否有公开函数
        for i in 1..path.len() {
            if self.public_functions.contains(&path[i]) {
                return false;  // 发现中间有公开函数，非最小路径
            }
        }
        true
    }

    // 将路径转换为带有函数详细信息的格式
    fn convert_path_to_node_info(&self, path: Vec<String>) -> Vec<PathNodeInfo> {
        path.into_iter()
            .map(|full_path| {
                if let Some(info) = self.functions.get(&full_path) {
                    let param_types = self.param_custom_types.get(&full_path)
                        .cloned()
                        .unwrap_or_else(HashSet::new);
                    
                    let return_types = self.return_custom_types.get(&full_path)
                        .cloned()
                        .unwrap_or_else(HashSet::new);
                    
                    PathNodeInfo {
                        full_path,
                        visibility: info.visibility.clone(),
                        source_code: info.source_code.clone(),
                        param_custom_types: param_types,
                        return_custom_types: return_types,
                    }
                } else {
                    // 默认值，通常不会到达这里
                    PathNodeInfo {
                        full_path,
                        visibility: VisibilityKind::Module,
                        source_code: String::new(),
                        param_custom_types: HashSet::new(),
                        return_custom_types: HashSet::new(),
                    }
                }
            })
            .collect()
    }

    // 查找从公开函数到内部不安全函数的所有有效路径，返回带有函数详细信息的路径
    fn find_paths_to_unsafe(&self) -> Vec<Vec<PathNodeInfo>> {
        let mut all_paths = Vec::new();
        
        // 首先添加所有直接公开的不安全函数
        for pub_unsafe_fn in &self.public_unsafe_functions {
            // 只添加那些不是unsafe声明的公开函数
            if self.public_non_unsafe_functions.contains(pub_unsafe_fn) {
                let mut path = Vec::new();
                path.push(pub_unsafe_fn.clone());
                all_paths.push(path);
            }
        }
        
        // 获取非公开的不安全函数
        let non_public_unsafe = self.unsafe_functions.difference(&self.public_unsafe_functions)
                                                   .cloned()
                                                   .collect::<HashSet<String>>();
        
        // 预先计算每个非unsafe声明的公开函数可到达的不安全函数
        for pub_fn in &self.public_non_unsafe_functions {
            // 排除已经是公开不安全的函数
            if !self.public_unsafe_functions.contains(pub_fn) {
                // 预计算可到达的目标
                let reachable_targets = self.precompute_reachable_targets(pub_fn, &non_public_unsafe);
                
                if !reachable_targets.is_empty() {
                    let paths = self.find_valid_paths(pub_fn, &reachable_targets);
                    all_paths.extend(paths);
                }
            }
        }
        
        // 过滤有效路径、最小路径，并转换为带详细信息的格式
        all_paths.into_iter()
            .filter(|path| self.is_valid_path(path))
            .filter(|path| self.is_minimal_path(path)) // 添加最小路径过滤条件
            .filter(|path| path.len() > 1) // 只保留长度大于1的路径
            .map(|path| self.convert_path_to_node_info(path))
            .collect()
    }

    // 预计算可达的目标函数，减少搜索空间
    fn precompute_reachable_targets(&self, start: &String, targets: &HashSet<String>) -> HashSet<String> {
        let mut reachable = HashSet::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        
        queue.push_back(start.clone());
        visited.insert(start.clone());
        
        while let Some(current) = queue.pop_front() {
            if targets.contains(&current) {
                reachable.insert(current.clone());
            }
            
            if let Some(callees) = self.calls.get(&current) {
                for callee in callees {
                    if !visited.contains(callee) {
                        visited.insert(callee.clone());
                        queue.push_back(callee.clone());
                    }
                }
            }
        }
        
        reachable
    }

    // 查找从起始函数到目标函数集合中任意一个函数的所有有效路径
    fn find_valid_paths(&self, start: &String, targets: &HashSet<String>) -> Vec<Vec<String>> {
        let mut all_paths = Vec::new();
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        
        self.dfs_find_valid_paths(start, targets, &mut visited, &mut path, &mut all_paths, 0);
        
        all_paths
    }

    // 深度优先搜索查找有效路径，增加深度限制
    fn dfs_find_valid_paths(
        &self,
        current: &String,
        targets: &HashSet<String>,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
        all_paths: &mut Vec<Vec<String>>,
        depth: usize,
    ) {
        // 超过深度限制，终止搜索
        if depth > self.max_search_depth {
            return;
        }
        
        if visited.contains(current) {
            return; // 避免循环
        }
        
        // 检查当前节点是否是中间节点，如果是则检查是否为unsafe函数或公开的不安全函数
        if !path.is_empty() && !targets.contains(current) {
            // 跳过中间节点为unsafe函数或公开的不安全函数的路径
            if self.unsafe_functions.contains(current) || self.public_unsafe_functions.contains(current) {
                return;
            }
        }
        
        visited.insert(current.clone());
        path.push(current.clone());
        
        if targets.contains(current) {
            all_paths.push(path.clone()); // 找到一条有效路径
        } else if let Some(callees) = self.calls.get(current) {
            for callee in callees {
                self.dfs_find_valid_paths(callee, targets, visited, path, all_paths, depth + 1);
            }
        }
        
        // 回溯
        path.pop();
        visited.remove(current);
    }
}

// 用于收集函数信息和检测unsafe块的访问者
struct FunctionVisitor {
    current_module_path: Vec<String>,
    functions: HashMap<String, FunctionInfo>,
    unsafe_functions: HashSet<String>,
    current_function: Option<String>,
    has_unsafe: bool,
    file_path: String,
    source_code: String,
    type_definitions: HashMap<String, TypeDefinition>, // 收集到的类型定义
    current_impl_type: Option<String>, // 当前正在访问的impl块的类型名称
    impl_blocks: HashMap<String, Vec<String>>, // 每个类型的完整impl块集合
}

impl FunctionVisitor {
    fn new(file_path: String, source_code: String) -> Self {
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
    
    // 获取当前模块路径
    fn get_current_module_path(&self) -> String {
        self.current_module_path.join("::")
    }
    
    // 从syn::Visibility转换为VisibilityKind
    fn convert_visibility(&self, vis: &Visibility) -> VisibilityKind {
        match vis {
            Visibility::Public(_) => VisibilityKind::Public,
            Visibility::Restricted(restricted) if restricted.path.is_ident("crate") => VisibilityKind::Crate,
            Visibility::Restricted(_) => VisibilityKind::Restricted,
            _ => VisibilityKind::Module,
        }
    }
    
    // 添加函数到结果集
    fn add_function(&mut self, name: String, vis: &Visibility, fn_item: &ItemFn) {
        let module_path = self.get_current_module_path();
        let full_path = if module_path.is_empty() {
            name.clone()
        } else {
            format!("{}::{}", module_path, name)
        };
        
        self.current_function = Some(full_path.clone());
        self.has_unsafe = false;
        
        // 提取函数源代码
        let source_code = fn_item.to_token_stream().to_string();
        
        // 检查函数签名是否声明为unsafe
        let is_unsafe_fn = fn_item.sig.unsafety.is_some();
        
        // 分析函数参数和返回值中使用的自定义类型
        let (param_types, return_types) = self.analyze_function_signature(&fn_item.sig);
        
        let info = FunctionInfo {
            name,
            module_path,
            visibility: self.convert_visibility(vis),
            has_internal_unsafe: false, // 稍后更新
            is_unsafe_fn,
            file_path: self.file_path.clone(),
            source_code,
            param_custom_types: param_types,
            return_custom_types: return_types,
        };
        
        self.functions.insert(full_path, info);
    }
    
    // 从impl块函数提取源代码
    fn add_impl_function(&mut self, name: String, vis: &Visibility, impl_fn: &ImplItemFn) {
        let module_path = self.get_current_module_path();
        let full_path = if module_path.is_empty() {
            name.clone()
        } else {
            format!("{}::{}", module_path, name)
        };
        
        self.current_function = Some(full_path.clone());
        self.has_unsafe = false;
        
        // 提取函数源代码
        let source_code = impl_fn.to_token_stream().to_string();
        
        // 检查函数签名是否声明为unsafe
        let is_unsafe_fn = impl_fn.sig.unsafety.is_some();
        
        // 分析函数参数和返回值中使用的自定义类型
        let (mut param_types, return_types) = self.analyze_function_signature(&impl_fn.sig);
        
        // 如果方法有self参数，并且我们知道当前impl的类型，则添加该类型到参数类型中
        if impl_fn.sig.inputs.iter().any(|arg| matches!(arg, syn::FnArg::Receiver(_))) {
            if let Some(impl_type) = &self.current_impl_type {
                param_types.insert(impl_type.clone());
            }
        }
        
        let info = FunctionInfo {
            name,
            module_path,
            visibility: self.convert_visibility(vis),
            has_internal_unsafe: false, // 稍后更新
            is_unsafe_fn,
            file_path: self.file_path.clone(),
            source_code,
            param_custom_types: param_types,
            return_custom_types: return_types,
        };
        
        self.functions.insert(full_path, info);
    }
    
    // 添加类型定义到结果集
    fn add_type_definition<T: ToTokens>(&mut self, name: String, vis: &Visibility, type_item: &T) {
        let module_path = self.get_current_module_path();
        let full_path = if module_path.is_empty() {
            name.clone()
        } else {
            format!("{}::{}", module_path, name)
        };
        
        // 提取类型定义的源代码
        let source_code = type_item.to_token_stream().to_string();
        
        let definition = TypeDefinition {
            name,
            module_path,
            visibility: self.convert_visibility(vis),
            source_code,
            file_path: self.file_path.clone(),
            constructors: Vec::new(), // 初始化为空列表
        };
        
        self.type_definitions.insert(full_path, definition);
    }

    fn is_constructor(&self, method: &syn::ImplItemFn, type_name: &str) -> bool {
        if let syn::ReturnType::Type(_, ty) = &method.sig.output {
            match &**ty {
                syn::Type::Path(type_path) => {
                    if let Some(segment) = type_path.path.segments.last() {
                        let return_type = segment.ident.to_string();
                        return return_type == "Self" || return_type == type_name;
                    }
                },
                // 处理引用类型的情况，如 &mut Self
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
    
    // 更新当前函数的unsafe状态
    fn update_unsafe_state(&mut self) {
        if let Some(ref func_path) = self.current_function {
            if self.has_unsafe {
                if let Some(info) = self.functions.get_mut(func_path) {
                    info.has_internal_unsafe = true;
                    self.unsafe_functions.insert(func_path.clone());
                }
            }
        }
    }
    
    // 分析函数签名中使用的自定义类型，分别返回参数和返回值的自定义类型
    fn analyze_function_signature(&self, sig: &syn::Signature) -> (HashSet<String>, HashSet<String>) {
        let mut param_types = HashSet::new();
        let mut return_types = HashSet::new();
        
        // 分析函数参数
        for param in &sig.inputs {
            if let syn::FnArg::Typed(pat_type) = param {
                self.extract_custom_types(&pat_type.ty, &mut param_types);
            }
        }
        
        // 分析返回类型
        if let syn::ReturnType::Type(_, ty) = &sig.output {
            self.extract_custom_types(ty, &mut return_types);
        }
        
        (param_types, return_types)
    }
    
    // 从类型中提取自定义类型
    fn extract_custom_types(&self, ty: &syn::Type, result: &mut HashSet<String>) {
        match ty {
            syn::Type::Path(type_path) if !self.is_primitive_type(&type_path.path) => {
                // 提取路径中的类型名称
                if let Some(segment) = type_path.path.segments.last() {
                    let type_name = segment.ident.to_string();
                    result.insert(type_name);
                    
                    // 递归处理泛型参数
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
                // 处理引用类型
                self.extract_custom_types(&type_ref.elem, result);
            },
            syn::Type::Array(type_array) => {
                // 处理数组类型
                self.extract_custom_types(&type_array.elem, result);
            },
            syn::Type::Slice(type_slice) => {
                // 处理切片类型
                self.extract_custom_types(&type_slice.elem, result);
            },
            syn::Type::Tuple(type_tuple) => {
                // 处理元组类型
                for elem in &type_tuple.elems {
                    self.extract_custom_types(elem, result);
                }
            },
            _ => {}
        }
    }
    
    // 判断是否为Rust原始类型或标准库类型
    fn is_primitive_type(&self, path: &syn::Path) -> bool {
        if path.segments.len() != 1 {
            return false;
        }
        
        let type_name = path.segments[0].ident.to_string();
        matches!(type_name.as_str(), 
            // 原始类型
            "bool" | "char" | "str" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" |
            "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "f32" | "f64" |
            // 标准库常用类型
            "String" | "Vec" | "Option" | "Result" | "Box" | "Rc" | "Arc" | "Cell" | "RefCell" |
            "HashMap" | "HashSet" | "BTreeMap" | "BTreeSet" | "VecDeque" | "LinkedList" |
            "Mutex" | "RwLock" | "Condvar" | "Once" | "Thread" | "Duration" | "Instant" |
            "SystemTime" | "Path" | "PathBuf"
        )
    }
}

impl<'ast> Visit<'ast> for FunctionVisitor {
    // 访问模块
    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        self.current_module_path.push(i.ident.to_string());
        
        // 访问模块内容
        if let Some((_, items)) = &i.content {
            for item in items {
                visit::visit_item(self, item);
            }
        }
        
        self.current_module_path.pop();
    }
    
    // 访问函数定义
    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        let name = i.sig.ident.to_string();
        self.add_function(name, &i.vis, i);
        
        // 访问函数体
        visit::visit_block(self, &i.block);
        
        // 更新unsafe状态
        self.update_unsafe_state();
        self.current_function = None;
    }
    
    // 访问impl块中的函数
    fn visit_impl_item_fn(&mut self, i: &'ast ImplItemFn) {
        let name = i.sig.ident.to_string();
        self.add_impl_function(name, &i.vis, i);
        
        // 访问函数体
        visit::visit_block(self, &i.block);
        
        // 更新unsafe状态
        self.update_unsafe_state();
        self.current_function = None;
    }
    
    // 访问unsafe块
    fn visit_expr_unsafe(&mut self, i: &'ast ExprUnsafe) {
        self.has_unsafe = true;
        
        // 继续访问unsafe块内部
        visit::visit_expr_unsafe(self, i);
    }
    
    // 访问结构体定义
    fn visit_item_struct(&mut self, i: &'ast syn::ItemStruct) {
        let name = i.ident.to_string();
        self.add_type_definition(name, &i.vis, i);
        visit::visit_item_struct(self, i);
    }
    
    // 访问枚举定义
    fn visit_item_enum(&mut self, i: &'ast syn::ItemEnum) {
        let name = i.ident.to_string();
        self.add_type_definition(name, &i.vis, i);
        visit::visit_item_enum(self, i);
    }
    
    // 访问类型别名
    fn visit_item_type(&mut self, i: &'ast syn::ItemType) {
        let name = i.ident.to_string();
        self.add_type_definition(name, &i.vis, i);
        visit::visit_item_type(self, i);
    }

    // 访问impl块
    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        // 提取impl块对应的类型名称
        let type_name = match &*i.self_ty {
            syn::Type::Path(type_path) => {
                if let Some(segment) = type_path.path.segments.last() {
                    segment.ident.to_string()
                } else {
                    // 如果无法获取类型名称，则跳过
                    return;
                }
            },
            _ => return, // 不是路径类型，跳过
        };
        
        // 设置当前impl类型，以便在处理方法时使用
        self.current_impl_type = Some(type_name.clone());
        
        // 保存完整的impl块代码
        let impl_code = i.to_token_stream().to_string();
        self.impl_blocks.entry(type_name.clone())
            .or_insert_with(Vec::new)
            .push(impl_code.clone());
        
        // 检查是否是Default特性的实现
        let is_default_impl = if let Some((_, trait_path, _)) = &i.trait_ {
            trait_path.segments.last()
                .map(|seg| seg.ident.to_string() == "Default")
                .unwrap_or(false)
        } else {
            false
        };
        
        // 如果是Default实现，则将整个impl块添加为构造函数
        if is_default_impl {
            // 查找对应的类型定义并添加构造函数
            for (path, def) in &mut self.type_definitions {
                if let Some(def_name) = path.split("::").last() {
                    if def_name == &type_name {
                        def.constructors.push(impl_code.clone());
                    }
                }
            }
        } else {
            // 对于非Default实现，只提取构造函数方法
            for item in &i.items {
                if let syn::ImplItem::Fn(method) = item {
                    if self.is_constructor(method, &type_name) {
                        // 检查函数是否是unsafe的
                        let is_unsafe = method.sig.unsafety.is_some();
                        
                        // 只添加安全的构造函数
                        if !is_unsafe {
                            // 只提取这个构造函数方法
                            let method_code = format!("impl {} {{\n    {}\n}}", 
                                type_name, 
                                method.to_token_stream().to_string());
                            
                            // 查找对应的类型定义并添加构造函数
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
        
        // 继续访问impl块的内容
        visit::visit_item_impl(self, i);
        
        // 清除当前impl类型
        self.current_impl_type = None;
    }
}

// 用于收集函数调用关系的访问者
struct CallVisitor {
    current_module_path: Vec<String>,
    current_function: Option<String>,
    calls: Vec<FunctionCall>,
    // 导入的模块和别名映射
    imports: HashMap<String, String>,
}

impl CallVisitor {
    fn new() -> Self {
        CallVisitor {
            current_module_path: Vec::new(),
            current_function: None,
            calls: Vec::new(),
            imports: HashMap::new(),
        }
    }
    
    // 获取当前模块路径
    fn get_current_module_path(&self) -> String {
        self.current_module_path.join("::")
    }
    
    // 处理函数调用表达式
    fn handle_call(&mut self, func_path: &syn::Path) {
        if let Some(ref caller) = self.current_function {
            let callee = self.resolve_path(func_path);
            self.calls.push(FunctionCall {
                caller: caller.clone(),
                callee,
            });
        }
    }
    
    // 解析路径，处理导入和别名
    fn resolve_path(&self, path: &syn::Path) -> String {
        let path_str = path.to_token_stream().to_string().replace(' ', "");
        
        // 检查是否是导入的模块或别名
        if path.segments.len() > 0 {
            let first_segment = &path.segments[0].ident.to_string();
            if let Some(import) = self.imports.get(first_segment) {
                // 替换路径的第一部分为导入的完整路径
                return path_str.replacen(first_segment, import, 1);
            }
        }
        
        // 如果是相对路径（不以crate::或::开头），加上当前模块路径
        if !path_str.starts_with("crate::") && !path_str.starts_with("::") {
            let module_path = self.get_current_module_path();
            if !module_path.is_empty() {
                return format!("{}::{}", module_path, path_str);
            }
        }
        
        path_str
    }
    
    // 处理导入语句
    fn process_use(&mut self, use_tree: &UseTree, prefix: &str) {
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
                // 添加到导入映射
                self.imports.insert(use_name.ident.to_string(), full_path);
            },
            UseTree::Rename(use_rename) => {
                let full_path = if prefix.is_empty() {
                    use_rename.ident.to_string()
                } else {
                    format!("{}::{}", prefix, use_rename.ident)
                };
                // 添加别名到导入映射
                self.imports.insert(use_rename.rename.to_string(), full_path);
            },
            UseTree::Glob(_) => {
                // 处理通配符导入较复杂，这里简化处理
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
    // 访问模块
    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        self.current_module_path.push(i.ident.to_string());
        
        // 访问模块内容
        if let Some((_, items)) = &i.content {
            for item in items {
                visit::visit_item(self, item);
            }
        }
        
        self.current_module_path.pop();
    }
    
    // 访问函数定义
    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        let name = i.sig.ident.to_string();
        let module_path = self.get_current_module_path();
        let full_path = if module_path.is_empty() {
            name
        } else {
            format!("{}::{}", module_path, name)
        };
        
        self.current_function = Some(full_path);
        
        // 访问函数体
        visit::visit_block(self, &i.block);
        
        self.current_function = None;
    }
    
    // 访问impl块中的函数
    fn visit_impl_item_fn(&mut self, i: &'ast ImplItemFn) {
        let name = i.sig.ident.to_string();
        let module_path = self.get_current_module_path();
        let full_path = if module_path.is_empty() {
            name
        } else {
            format!("{}::{}", module_path, name)
        };
        
        self.current_function = Some(full_path);
        
        // 访问函数体
        visit::visit_block(self, &i.block);
        
        self.current_function = None;
    }
    
    // 访问函数调用表达式
    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        // 检查被调用的表达式是否是路径（函数名）
        if let syn::Expr::Path(expr_path) = &*i.func {
            self.handle_call(&expr_path.path);
        }
        
        // 继续访问参数
        for arg in &i.args {
            visit::visit_expr(self, arg);
        }
    }
    
    // 访问方法调用表达式
    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        // 方法调用更复杂，需要类型信息才能准确解析
        // 这里简化处理，只记录方法名
        if let Some(ref caller) = self.current_function {
            let method_name = i.method.to_string();
            // 简化：假设方法是在当前模块中定义的
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
        
        // 继续访问接收者和参数
        visit::visit_expr(self, &i.receiver);
        for arg in &i.args {
            visit::visit_expr(self, arg);
        }
    }
    
    // 访问导入语句
    fn visit_item_use(&mut self, i: &'ast syn::ItemUse) {
        self.process_use(&i.tree, "");
        visit::visit_item_use(self, i);
    }
}

// 静态分析器
struct StaticAnalyzer {
    results: Arc<Mutex<Vec<FileAnalysisResult>>>,
    max_search_depth: usize,
    file_size_limit: u64,
    timeout: Duration,
}

impl StaticAnalyzer {
    fn new(max_depth: usize, file_size_limit_mb: u64, timeout_seconds: u64) -> Self {
        StaticAnalyzer {
            results: Arc::new(Mutex::new(Vec::new())),
            max_search_depth: max_depth,
            file_size_limit: file_size_limit_mb * 1024 * 1024,
            timeout: Duration::from_secs(timeout_seconds),
        }
    }

    // 快速检查文件是否可能包含需要分析的代码
    fn should_analyze_file(&self, file_path: &Path) -> io::Result<bool> {
        // 检查文件大小
        let metadata = fs::metadata(file_path)?;
        if metadata.len() > self.file_size_limit {
            return Ok(false);
        }
        
        // 读取文件内容
        let content = fs::read_to_string(file_path)?;
        
        // 如果文件不包含 unsafe 或 pub fn，可以跳过
        if !content.contains("unsafe") || !content.contains("pub fn") {
            return Ok(false);
        }
        
        Ok(true)
    }

    // 分析单个文件
    fn analyze_file(&self, file_path: &Path) -> io::Result<Option<FileAnalysisResult>> {
        // 快速检查文件是否需要分析
        if !self.should_analyze_file(file_path)? {
            return Ok(None);
        }
        
        let start_time = Instant::now();
        
        // 读取文件内容，添加更多错误处理
        let source = match fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("读取文件错误 {}: {}", file_path.display(), e);
                return Ok(None); // 读取错误也返回None而不是错误
            }
        };
        
        // 解析源代码
        let syntax = match syn::parse_file(&source) {
            Ok(parsed) => parsed,
            Err(e) => {
                eprintln!("解析文件错误 {}: {}", file_path.display(), e);
                return Ok(None); // 将解析错误作为None返回，而不是错误
            }
        };
        
        // 收集函数信息
        let file_path_str = file_path.to_string_lossy().to_string();
        
        // 使用防御性编程捕获可能的异常
        let fn_visitor_result = std::panic::catch_unwind(|| {
            let mut fn_visitor = FunctionVisitor::new(file_path_str.clone(), source.clone());
            fn_visitor.visit_file(&syntax);
            fn_visitor
        });
        
        let fn_visitor = match fn_visitor_result {
            Ok(visitor) => visitor,
            Err(_) => {
                eprintln!("函数访问器处理文件时崩溃 {}", file_path.display());
                return Ok(None);
            }
        };
        
        // 同样防御性处理调用访问器
        let call_visitor_result = std::panic::catch_unwind(|| {
            let mut call_visitor = CallVisitor::new();
            call_visitor.visit_file(&syntax);
            call_visitor
        });
        
        let call_visitor = match call_visitor_result {
            Ok(visitor) => visitor,
            Err(_) => {
                eprintln!("调用访问器处理文件时崩溃 {}", file_path.display());
                return Ok(None);
            }
        };
        
        // 检查超时
        if start_time.elapsed() >= self.timeout {
            eprintln!("分析超时: {}", file_path.display());
            return Ok(None);
        }

        // 创建调用图并分析
        let mut call_graph = CallGraph::new(self.max_search_depth);
        
        // 添加函数和调用关系
        for (path, info) in fn_visitor.functions {
            call_graph.add_function(path, info);
        }
        
        for call in call_visitor.calls {
            call_graph.add_call(call.caller, call.callee);
        }
        
        // 查找路径，现在返回带有函数详细信息的路径
        let paths = call_graph.find_paths_to_unsafe();
        
        if paths.is_empty() {
            return Ok(None);
        }
        
        // 收集路径中使用的自定义类型
        let mut used_types = HashSet::new();
        for path in &paths {
            if !path.is_empty() {
                // 只收集起始函数参数中的自定义类型（已包含self类型）
                used_types.extend(&path[0].param_custom_types);
            }
        }
        
        // 查找相关的类型定义
        let mut path_type_defs = HashMap::new();
        for type_name in &used_types {
            for (path, def) in &fn_visitor.type_definitions {
                let def_name = path.split("::").last().unwrap_or(path);
                if def_name == *type_name {
                    // 创建定义的副本
                    let mut type_def = def.clone();
                    
                    // 添加该类型的安全构造函数（如果是self类型）
                    if let Some(impl_blocks) = fn_visitor.impl_blocks.get(def_name) {
                        // 如果在函数参数中使用了此类型，只添加安全的构造函数
                        for block_path in paths.iter() {
                            if !block_path.is_empty() && block_path[0].param_custom_types.contains(def_name) {
                                // 已经在FunctionVisitor中处理过构造函数，直接使用
                                // 从type_definitions中取回原始的构造函数列表
                                type_def.constructors = def.constructors.clone();
                                break; // 避免重复添加相同的构造函数
                            }
                        }
                        
                        // 然后添加调用链中的相关函数
                        let mut added_functions = HashSet::new(); // 跟踪已添加的函数
                        
                        for (path_index, block_path) in paths.iter().enumerate() {
                            for (step_index, node) in block_path.iter().enumerate() {
                                // 检查函数是否对该类型进行了操作
                                let operates_on_type = node.param_custom_types.contains(def_name) || 
                                                     node.return_custom_types.contains(def_name);
                                
                                // 避免重复添加相同的函数
                                if operates_on_type && !added_functions.contains(&node.full_path) {
                                    // 添加到已处理集合
                                    added_functions.insert(node.full_path.clone());
                                    
                                    // 提取函数名称
                                    let fn_name = node.full_path.split("::").last().unwrap_or(&node.full_path);
                                    
                                    // 检查函数是如何使用该类型的
                                    let relation_type = if node.param_custom_types.contains(def_name) && node.return_custom_types.contains(def_name) {
                                        "参数和返回值"
                                    } else if node.param_custom_types.contains(def_name) {
                                        "参数"
                                    } else {
                                        "返回值"
                                    };
                                    
                                    // 格式化为impl块样式，并添加步骤注释
                                    let method_code = format!(
                                        "impl {} {{\n    // 调用链 #{} - 步骤 #{} - 函数: {} - 使用类型作为: {}\n    {}\n}}", 
                                        def_name,
                                        path_index + 1,
                                        step_index + 1,
                                        fn_name,
                                        relation_type,
                                        StaticAnalyzer::enhanced_format_source_code(&node.source_code)
                                    );
                                    
                                    // 添加到结果中
                                    type_def.constructors.push(method_code);
                                }
                            }
                        }
                    }
                    
                    path_type_defs.insert(path.clone(), type_def);
                }
            }
        }
        
        Ok(Some(FileAnalysisResult {
            file_path: file_path_str,
            paths,
            type_definitions: path_type_defs,
        }))
    }

    // 并行分析目录
    fn analyze_directory_parallel(&self, dir_path: &Path) -> io::Result<()> {
        let start_time = Instant::now();
        
        // 收集所有Rust文件路径
        let rust_files = self.collect_rust_files(dir_path)?;
        let total_files = rust_files.len();
        
        println!("找到 {} 个Rust文件，开始并行分析...", total_files);
        
        // 创建进度计数器
        let processed_count = Arc::new(Mutex::new(0));
        let results = self.results.clone();
        let error_count = Arc::new(Mutex::new(0));
        
        // 并行处理文件，使用rayon的try_for_each来捕获可能的错误
        let process_result = rust_files.par_iter().try_for_each(|path| -> Result<(), io::Error> {
            // 使用catch_unwind捕获严重错误，防止单个文件导致整个处理停止
            let file_result = std::panic::catch_unwind(|| {
                self.analyze_file(path)
            });
            
            match file_result {
                Ok(Ok(Some(file_result))) => {
                    // 正常情况：文件分析成功并有结果
                    let mut results_guard = results.lock().unwrap();
                    results_guard.push(file_result);
                },
                Ok(Ok(None)) => {
                    // 正常情况：文件分析成功但无结果
                },
                Ok(Err(e)) => {
                    // 文件IO错误
                    eprintln!("分析文件IO错误 {}: {}", path.display(), e);
                    let mut count = error_count.lock().unwrap();
                    *count += 1;
                },
                Err(_) => {
                    // 解析错误或其他严重错误
                    eprintln!("解析文件时发生严重错误 {}", path.display());
                    let mut count = error_count.lock().unwrap();
                    *count += 1;
                }
            }
            
            // 更新进度
            let mut count = processed_count.lock().unwrap();
            *count += 1;
            if *count % 100 == 0 || *count == total_files {
                println!("已处理: {}/{} 文件 ({:.1}%) 用时: {:?}", 
                         *count, total_files, 
                         (*count as f64 / total_files as f64) * 100.0,
                         start_time.elapsed());
            }
            
            // 继续处理下一个文件
            Ok(())
        });
        
        // 处理整体错误
        if let Err(e) = process_result {
            eprintln!("并行处理文件时发生错误: {}", e);
        }
        
        let error_count = *error_count.lock().unwrap();
        println!("分析完成! 总共处理 {} 个文件，其中 {} 个文件出错，用时: {:?}", 
                 total_files, error_count, start_time.elapsed());
        
        Ok(())
    }
    
    // 收集目录中所有Rust文件
    fn collect_rust_files(&self, dir_path: &Path) -> io::Result<Vec<PathBuf>> {
        let mut rust_files = Vec::new();
        
        let walk_dir = WalkDir::new(dir_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok());
        
        for entry in walk_dir {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
                rust_files.push(path.to_owned());
            }
        }
        
        Ok(rust_files)
    }
    
    // 获取分析结果
    fn get_results(&self) -> Vec<FileAnalysisResult> {
        let guard = self.results.lock().unwrap();
        guard.clone()
    }
    
    fn beautify_source_code(source_code: &str) -> String {
        // Direct formatting approach with prettyplease
        if let Ok(parsed) = syn::parse_str::<syn::File>(&format!("{}", source_code)) {
            return prettyplease::unparse(&parsed);
        }
        
        // Try to wrap the function in a mod to handle complete function definitions
        let wrapped_code = format!("mod dummy {{ {} }}", source_code);
        if let Ok(parsed) = syn::parse_str::<syn::File>(&wrapped_code) {
            let formatted = prettyplease::unparse(&parsed);
            return Self::extract_from_mod(&formatted);
        }
        
        // Try wrapping as an impl block for method definitions
        let wrapped_code = format!("impl Dummy {{ {} }}", source_code);
        if let Ok(parsed) = syn::parse_str::<syn::File>(&wrapped_code) {
            let formatted = prettyplease::unparse(&parsed);
            return Self::extract_from_impl(&formatted);
        }
        
        // As a last resort, use an enhanced basic formatter
        Self::enhanced_format_source_code(source_code)
    }
    
    // Extract content from a dummy mod
    fn extract_from_mod(formatted: &str) -> String {
        let lines: Vec<&str> = formatted.lines().collect();
        let mut result = Vec::new();
        let mut in_mod = false;
        let mut brace_level: i32 = 0;
        
        for line in lines {
            if line.trim().starts_with("mod dummy {") {
                in_mod = true;
                brace_level = 1;
                continue;
            }
            
            if in_mod {
                let open_braces = line.matches('{').count() as i32;
                let close_braces = line.matches('}').count() as i32;
                
                brace_level += open_braces - close_braces;
                
                if brace_level <= 0 {
                    break; // We've reached the end of the mod
                }
                
                result.push(line);
            }
        }
        
        result.join("\n")
    }
    
    // Extract content from a dummy impl
    fn extract_from_impl(formatted: &str) -> String {
        let lines: Vec<&str> = formatted.lines().collect();
        let mut result = Vec::new();
        let mut in_impl = false;
        let mut brace_level: i32 = 0;
        
        for line in lines {
            if line.trim().starts_with("impl Dummy {") {
                in_impl = true;
                brace_level = 1;
                continue;
            }
            
            if in_impl {
                let open_braces = line.matches('{').count() as i32;
                let close_braces = line.matches('}').count() as i32;
                
                brace_level += open_braces - close_braces;
                
                if brace_level <= 0 {
                    break; // We've reached the end of the impl
                }
                
                result.push(line);
            }
        }
        
        result.join("\n")
    }

    // Enhanced basic formatter with more robust brace handling
    fn enhanced_format_source_code(source_code: &str) -> String {
        // First, check if the function is incomplete - if so, try to find the closing brace
        let complete_source = Self::ensure_complete_function(source_code);
        
        let mut result = String::new();
        let lines: Vec<&str> = complete_source.lines().collect();
        
        // Explicitly specify type as usize
        let mut indent_level: usize = 0;
        let mut within_comment = false;
        let mut had_content = false;
        
        for line in lines {
            let trimmed = line.trim();
            
            // 跳过文档注释
            if trimmed.starts_with("///") {
                continue;
            }
            
            // Handle multi-line comments
            if trimmed.starts_with("/*") {
                within_comment = true;
            }
            
            if within_comment {
                if trimmed.ends_with("*/") {
                    within_comment = false;
                }
                continue;
            }
            
            // Skip empty lines at the beginning, but preserve them after content
            if trimmed.is_empty() {
                if had_content {
                    result.push('\n');
                }
                continue;
            }
            
            had_content = true;
            
            // Count the braces before adjusting indentation for this line
            let start_with_close = trimmed.starts_with('}') || trimmed.starts_with(']') || trimmed.starts_with(')');
            
            if start_with_close && indent_level > 0 {
                indent_level -= 1;
            }
            
            // Add appropriate indentation
            let indent = "    ".repeat(indent_level);
            result.push_str(&format!("{}{}\n", indent, trimmed));
            
            // Adjust indent level for the next line based on braces in this line
            let open_count = trimmed.matches('{').count() + trimmed.matches('[').count() + 
                            (trimmed.matches('(').count() - trimmed.matches(')').count()).max(0);
            
            let close_count = trimmed.matches('}').count() + trimmed.matches(']').count() + 
                             (trimmed.matches(')').count() - trimmed.matches('(').count()).max(0);
            
            // Adjust for opening braces
            indent_level += open_count;
            
            // Adjust for closing braces that aren't at the start (already handled)
            if close_count > (if start_with_close { 1 } else { 0 }) {
                let extra_close = close_count - (if start_with_close { 1 } else { 0 });
                if indent_level >= extra_close {
                    indent_level -= extra_close;
                } else {
                    indent_level = 0;
                }
            }
        }
        
        result
    }
    
    // Try to ensure a function definition is complete by adding missing closing braces
    fn ensure_complete_function(source_code: &str) -> String {
        let mut open_braces = 0;
        let mut close_braces = 0;
        
        for c in source_code.chars() {
            if c == '{' {
                open_braces += 1;
            } else if c == '}' {
                close_braces += 1;
            }
        }
        
        if open_braces > close_braces {
            let mut complete_code = source_code.to_string();
            for _ in 0..(open_braces - close_braces) {
                complete_code.push_str("\n}");
            }
            return complete_code;
        }
        
        source_code.to_string()
    }
    
    // 修改write_results_to_file方法，只显示起始公开函数参数中使用的自定义类型
    fn write_results_to_file(&self, output_path: &Path) -> io::Result<()> {
        println!("正在写入结果到: {}", output_path.display());
        
        let file = File::create(output_path)?;
        let mut writer = BufWriter::new(file);
        
        let results = self.get_results();
        
        for result in &results {
            if !result.paths.is_empty() {
                // 文件标题
                writeln!(writer, "文件: {}", result.file_path)?;
                writeln!(writer, "找到 {} 条从公开函数到内部不安全函数的有效路径:", result.paths.len())?;
                
                // 每条路径及其函数实现
                for (i, path) in result.paths.iter().enumerate() {
                    // 路径信息
                    writeln!(writer, "{}. {}", i + 1, Self::format_path_with_visibility(path))?;
                    
                    // 只从路径的第一个节点（起始公开函数）收集参数中的自定义类型
                    if !path.is_empty() {
                        let first_node = &path[0];
                        let param_types = &first_node.param_custom_types;
                        
                        // 只处理函数参数中使用的自定义类型
                        if !param_types.is_empty() {
                            // 查找相关的类型定义
                            let mut types_to_print = HashSet::new();
                            for type_name in param_types {
                                for (type_path, _) in &result.type_definitions {
                                    if let Some(def_name) = type_path.split("::").last() {
                                        if def_name == type_name {
                                            types_to_print.insert(type_path.clone());
                                        }
                                    }
                                }
                            }
                            
                            // 打印相关的自定义类型定义和构造函数
                            if !types_to_print.is_empty() {
                                writeln!(writer, "\n// 相关自定义类型定义:")?;
                                for type_path in &types_to_print {
                                    if let Some(type_def) = result.type_definitions.get(type_path) {
                                        writeln!(writer, "// 类型: {}", type_path)?;
                                        
                                        // 输出类型定义
                                        let formatted_type = Self::beautify_source_code(&type_def.source_code);
                                        let visibility_prefix = type_def.visibility.to_string();
                                        if !formatted_type.trim_start().starts_with("pub ") && !visibility_prefix.is_empty() {
                                            writeln!(writer, "{}{}", visibility_prefix, formatted_type)?;
                                        } else {
                                            writeln!(writer, "{}", formatted_type)?;
                                        }
                                        
                                        // 输出构造函数
                                        for constructor in &type_def.constructors {
                                            let formatted_constructor = Self::beautify_source_code(constructor);
                                            writeln!(writer, "\n{}", formatted_constructor)?;
                                        }
                                        
                                        writeln!(writer, "")?;
                                    }
                                }
                                writeln!(writer, "")?;
                            }
                        }
                    }
                    
                    // 在路径后直接输出该路径中所有函数的完整实现
                    for node in path {
                        // 添加函数的完整路径作为注释和分隔线
                        writeln!(writer, "// 函数: {}", node.full_path)?;
                        
                        // 使用增强的格式化方法
                        let source_code = Self::beautify_source_code(&node.source_code);
                        writeln!(writer, "{}", source_code)?;
                        writeln!(writer, "")?;  // 函数间空行分隔
                    }
                    
                    // 路径之间添加空行
                    writeln!(writer, "")?;
                }
                
                // 分隔符
                writeln!(writer, "{}", "=".repeat(80))?;
            }
        }
        
        println!("成功写入 {} 个文件的分析结果", results.len());
        Ok(())
    }
    
    // 格式化路径，添加可见性信息
    fn format_path_with_visibility(path: &[PathNodeInfo]) -> String {
        path.iter()
            .enumerate()
            .map(|(i, node)| {
                // 获取函数名的最后一个部分（去掉模块路径）
                let simple_name = node.full_path.split("::").last().unwrap_or(&node.full_path);
                let visibility_prefix = node.visibility.to_string();
                
                if i == 0 {
                    format!("{}fn {}", visibility_prefix, simple_name)
                } else {
                    format!(" -> {}fn {}", visibility_prefix, simple_name)
                }
            })
            .collect::<String>()
    }
}

// main 函数
fn main() -> io::Result<()> {
    // 设置panic处理器以防止程序在panic时立即退出
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("程序遇到了严重错误: {:?}", panic_info);
        eprintln!("尝试恢复并继续处理...");
    }));

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: {} <Rust项目或文件路径> [输出文件]", args[0]);
        eprintln!("  <Rust项目或文件路径>: Rust文件或目录的路径");
        eprintln!("  [输出文件]: 可选的结果保存路径 (默认: unsafe_paths.txt)");
        return Ok(());
    }

    let input_path = PathBuf::from(&args[1]);
    let output_path = if args.len() >= 3 {
        PathBuf::from(&args[2])
    } else {
        PathBuf::from("unsafe_paths.txt")
    };
    
    // 配置参数
    let max_search_depth = 20;       // 最大搜索深度
    let file_size_limit_mb = 10;     // 文件大小限制(MB)
    let timeout_seconds = 30;        // 单个文件分析超时时间(秒)
    
    // 使用catch_unwind捕获所有可能的panic
    let result = std::panic::catch_unwind(|| {
        let analyzer = StaticAnalyzer::new(max_search_depth, file_size_limit_mb, timeout_seconds);
        
        // 验证路径是否存在
        if !input_path.exists() {
            eprintln!("错误: 路径不存在: {}", input_path.display());
            return Ok(());
        }
        
        println!("开始分析: {}", input_path.display());
        
        // 如果是目录，并行分析每个文件，否则分析单个文件
        if input_path.is_dir() {
            // 添加错误恢复处理
            if let Err(e) = analyzer.analyze_directory_parallel(&input_path) {
                eprintln!("分析目录错误: {}，但程序将继续处理已完成的文件", e);
                // 即使出错也继续尝试写入已处理的结果
            }
        } else if input_path.extension().map_or(false, |ext| ext == "rs") {
            match analyzer.analyze_file(&input_path) {
                Ok(Some(result)) => {
                    let mut results = analyzer.results.lock().unwrap();
                    results.push(result);
                },
                Ok(None) => {
                    println!("文件 {} 无需分析或无有效结果", input_path.display());
                },
                Err(e) => {
                    eprintln!("分析文件错误: {}, 但程序将继续执行", e);
                }
            }
        } else {
            eprintln!("路径必须是Rust文件(.rs)或包含Rust文件的目录: {}", 
                    input_path.display());
            return Ok(());
        }
        
        // 写入结果
        if let Err(e) = analyzer.write_results_to_file(&output_path) {
            eprintln!("写入结果错误: {}", e);
        }
        
        println!("分析完成! 结果已保存到: {}", output_path.display());
        
        Ok(())
    });

    // 处理整体的panic
    match result {
        Ok(io_result) => io_result,
        Err(_) => {
            eprintln!("程序遇到了无法恢复的错误，但已尝试保存现有结果");
            Ok(())
        }
    }
}