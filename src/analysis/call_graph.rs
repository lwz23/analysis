use std::collections::{HashMap, HashSet, VecDeque};
use crate::models::{FunctionInfo, PathNodeInfo, VisibilityKind};

/// Function call graph representation
pub struct CallGraph {
    /// Mapping from function full path to function information
    pub functions: HashMap<String, FunctionInfo>,
    /// Mapping from caller to callees
    pub calls: HashMap<String, HashSet<String>>,
    /// Mapping from callee to callers (reverse graph)
    pub reverse_calls: HashMap<String, HashSet<String>>,
    /// Functions containing internal unsafe code
    pub unsafe_functions: HashSet<String>,
    /// Public functions
    pub public_functions: HashSet<String>,
    /// Public functions that contain unsafe code
    pub public_unsafe_functions: HashSet<String>,
    /// Public and non-unsafe-declared functions
    pub public_non_unsafe_functions: HashSet<String>,
    /// Maximum search depth
    pub max_search_depth: usize,
    /// Mapping from function path to custom types used in its parameters
    pub param_custom_types: HashMap<String, HashSet<String>>,
    /// Mapping from function path to custom types used in its return value
    pub return_custom_types: HashMap<String, HashSet<String>>,
}

impl CallGraph {
    pub fn new(max_depth: usize) -> Self {
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

    /// Add function to graph
    pub fn add_function(&mut self, path: String, info: FunctionInfo) {
        if info.visibility == VisibilityKind::Public {
            self.public_functions.insert(path.clone());
            
            // If function is public and not unsafe-declared, add to public_non_unsafe_functions set
            if !info.is_unsafe_fn {
                self.public_non_unsafe_functions.insert(path.clone());
            }
            
            // If function is both public and contains unsafe code, add to new set
            if info.has_internal_unsafe {
                self.public_unsafe_functions.insert(path.clone());
            }
        }
        
        if info.has_internal_unsafe {
            self.unsafe_functions.insert(path.clone());
        }
        
        // Save custom types used in function parameters and return value
        if !info.param_custom_types.is_empty() {
            self.param_custom_types.insert(path.clone(), info.param_custom_types.clone());
        }
        
        if !info.return_custom_types.is_empty() {
            self.return_custom_types.insert(path.clone(), info.return_custom_types.clone());
        }
        
        self.functions.insert(path, info);
    }

    /// Add function call relationship
    pub fn add_call(&mut self, caller: String, callee: String) {
        self.calls.entry(caller.clone()).or_insert_with(HashSet::new).insert(callee.clone());
        self.reverse_calls.entry(callee).or_insert_with(HashSet::new).insert(caller);
    }

    /// Check if path is valid, using public_non_unsafe_functions instead of public_functions
    /// for checking the first node
    pub fn is_valid_path(&self, path: &[String]) -> bool {
        // 修改：检查路径长度必须等于1（而不是大于1）
        if path.len() != 1 {
            return false;
        }
        
        // 检查节点必须是公共非unsafe声明函数
        if !self.public_non_unsafe_functions.contains(&path[0]) {
            return false;
        }
        
        // 检查节点必须是内部unsafe函数
        if !self.unsafe_functions.contains(&path[0]) {
            return false;
        }
        
        true
    }
    
    /// Check if path is minimal (no public functions except starting node)
    pub fn is_minimal_path(&self, path: &[String]) -> bool {
        // Skip first node, check if subsequent nodes have public functions
        for i in 1..path.len() {
            if self.public_functions.contains(&path[i]) {
                return false;  // Found public function in the middle, not minimal path
            }
        }
        true
    }

    /// Convert path to node info format with function details
    pub fn convert_path_to_node_info(&self, path: Vec<String>) -> Vec<PathNodeInfo> {
        path.into_iter()
            .map(|full_path| {
                if let Some(info) = self.functions.get(&full_path) {
                    let param_types = self.param_custom_types.get(&full_path)
                        .cloned()
                        .unwrap_or_else(HashSet::new);
                    
                    let return_types = self.return_custom_types.get(&full_path)
                        .cloned()
                        .unwrap_or_else(HashSet::new);
                    
                    //println!("Function: {}, has_self_param: {}, owner_type: {:?}", 
                        //full_path, info.has_self_param, info.owner_type);
                    
                    PathNodeInfo {
                        full_path,
                        visibility: info.visibility.clone(),
                        source_code: info.source_code.clone(),
                        param_custom_types: param_types,
                        return_custom_types: return_types,
                        has_self_param: info.has_self_param,
                        owner_type: info.owner_type.clone(),
                        unsafe_operations: info.unsafe_operations.clone(),
                    }
                } else {
                    // Default value, normally shouldn't reach here
                    PathNodeInfo {
                        full_path,
                        visibility: VisibilityKind::Module,
                        source_code: String::new(),
                        param_custom_types: HashSet::new(),
                        return_custom_types: HashSet::new(),
                        has_self_param: false,
                        owner_type: None,
                        unsafe_operations: Vec::new(),
                    }
                }
            })
            .collect()
    }

    /// Find all valid paths from public functions to internal unsafe functions,
    /// return paths with detailed function information
    pub fn find_paths_to_unsafe(&self) -> Vec<Vec<PathNodeInfo>> {
        let mut all_paths = Vec::new();
        
        // 只添加直接的公共不安全函数
        for pub_unsafe_fn in &self.public_unsafe_functions {
            // 只添加那些非unsafe声明的公共函数
            if self.public_non_unsafe_functions.contains(pub_unsafe_fn) {
                let mut path = Vec::new();
                path.push(pub_unsafe_fn.clone());
                
                // 修改：使用is_valid_path验证，这里会检查路径长度是否为1
                if self.is_valid_path(&path) {
                    all_paths.push(path);
                }
            }
        }
        
        // 将路径转换为详细格式
        all_paths.into_iter()
            .map(|path| self.convert_path_to_node_info(path))
            .collect()
    }

    /// Pre-compute reachable target functions, reducing search space
    pub fn precompute_reachable_targets(&self, start: &String, targets: &HashSet<String>) -> HashSet<String> {
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

    /// Find all valid paths from start function to any function in target set
    pub fn find_valid_paths(&self, start: &String, targets: &HashSet<String>) -> Vec<Vec<String>> {
        let mut all_paths = Vec::new();
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        
        self.dfs_find_valid_paths(start, targets, &mut visited, &mut path, &mut all_paths, 0);
        
        all_paths
    }

    /// Depth-first search to find valid paths, with depth limit
    pub fn dfs_find_valid_paths(
        &self,
        current: &String,
        targets: &HashSet<String>,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
        all_paths: &mut Vec<Vec<String>>,
        depth: usize,
    ) {
        // Exceed depth limit, terminate search
        if depth > self.max_search_depth {
            return;
        }
        
        if visited.contains(current) {
            return; // Avoid cycles
        }
        
        // Check if current node is intermediate node, if so check if it's unsafe or public unsafe
        if !path.is_empty() && !targets.contains(current) {
            // Skip paths with intermediate nodes that are unsafe or public unsafe
            if self.unsafe_functions.contains(current) || self.public_unsafe_functions.contains(current) {
                return;
            }
        }
        
        visited.insert(current.clone());
        path.push(current.clone());
        
        if targets.contains(current) {
            all_paths.push(path.clone()); // Found a valid path
        } else if let Some(callees) = self.calls.get(current) {
            for callee in callees {
                self.dfs_find_valid_paths(callee, targets, visited, path, all_paths, depth + 1);
            }
        }
        
        // Backtrack
        path.pop();
        visited.remove(current);
    }
}