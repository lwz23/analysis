use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use syn;
use syn::visit::Visit; // Import for the Visit trait
use walkdir::WalkDir;
use rayon::prelude::*;

use crate::visitors::{FunctionVisitor, CallVisitor};
use crate::models::{FileAnalysisResult, PathNodeInfo};
use crate::analysis::CallGraph;
use crate::utils;

/// Static analyzer for Rust code
pub struct StaticAnalyzer {
    results: Arc<Mutex<Vec<FileAnalysisResult>>>,
    max_search_depth: usize,
    file_size_limit: u64,
    timeout: Duration,
}

impl StaticAnalyzer {
    pub fn new(max_depth: usize, file_size_limit_mb: u64, timeout_seconds: u64) -> Self {
        StaticAnalyzer {
            results: Arc::new(Mutex::new(Vec::new())),
            max_search_depth: max_depth,
            file_size_limit: file_size_limit_mb * 1024 * 1024,
            timeout: Duration::from_secs(timeout_seconds),
        }
    }

    /// Quick check if file might contain code that needs analysis
    pub fn should_analyze_file(&self, file_path: &Path) -> io::Result<bool> {
        // Check file size
        let metadata = fs::metadata(file_path)?;
        if metadata.len() > self.file_size_limit {
            return Ok(false);
        }
        
        // Read file content
        let content = fs::read_to_string(file_path)?;
        
        // If file doesn't contain unsafe or pub fn, can skip
        if !content.contains("unsafe") || !content.contains("pub fn") {
            return Ok(false);
        }
        
        Ok(true)
    }

    /// Analyze a single file
    pub fn analyze_file(&self, file_path: &Path) -> io::Result<Option<FileAnalysisResult>> {
        // Quick check if file needs analysis
        if !self.should_analyze_file(file_path)? {
            return Ok(None);
        }
        
        let start_time = Instant::now();
        
        // Read file content, add more error handling
        let source = match fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error reading file {}: {}", file_path.display(), e);
                return Ok(None); // Return None instead of error for reading errors
            }
        };
        
        // Parse source code
        let syntax = match syn::parse_file(&source) {
            Ok(parsed) => parsed,
            Err(e) => {
                eprintln!("Error parsing file {}: {}", file_path.display(), e);
                return Ok(None); // Return parsing errors as None, not error
            }
        };
        
        // Collect function information
        let file_path_str = file_path.to_string_lossy().to_string();
        
        // Use defensive programming to catch possible panics
        let fn_visitor_result = std::panic::catch_unwind(|| {
            let mut fn_visitor = FunctionVisitor::new(file_path_str.clone(), source.clone());
            fn_visitor.visit_file(&syntax);
            fn_visitor
        });
        
        let fn_visitor = match fn_visitor_result {
            Ok(visitor) => visitor,
            Err(_) => {
                eprintln!("Function visitor crashed while processing file {}", file_path.display());
                return Ok(None);
            }
        };
        
        // Same defensive handling for call visitor
        let call_visitor_result = std::panic::catch_unwind(|| {
            let mut call_visitor = CallVisitor::new();
            call_visitor.visit_file(&syntax);
            call_visitor
        });
        
        let call_visitor = match call_visitor_result {
            Ok(visitor) => visitor,
            Err(_) => {
                eprintln!("Call visitor crashed while processing file {}", file_path.display());
                return Ok(None);
            }
        };
        
        // Check timeout
        if start_time.elapsed() >= self.timeout {
            eprintln!("Analysis timeout: {}", file_path.display());
            return Ok(None);
        }

        // Create call graph and analyze
        let mut call_graph = CallGraph::new(self.max_search_depth);
        
        // Add functions and call relationships
        for (path, info) in fn_visitor.functions {
            call_graph.add_function(path, info);
        }
        
        for call in call_visitor.calls {
            call_graph.add_call(call.caller, call.callee);
        }
        
        // Find paths, now returns paths with detailed function info
        let paths = call_graph.find_paths_to_unsafe();
        
        if paths.is_empty() {
            return Ok(None);
        }
        
        // Find relevant type definitions - FIXED VERSION
        let mut path_type_defs = std::collections::HashMap::new();
        for path in &paths {
            if path.is_empty() {
                continue;
            }
            
            // Only collect custom types in parameters of starting function
            let first_node = &path[0];
            let param_types = &first_node.param_custom_types;
            
            // We don't need this variable, so removed it to fix the warning
            
            for type_name in param_types {
                for (type_path, def) in &fn_visitor.type_definitions {
                    let def_name = type_path.split("::").last().unwrap_or(type_path);
                    if def_name == *type_name {
                        // Get or create type definition for this path
                        let type_def_entry = path_type_defs
                            .entry(type_path.clone())
                            .or_insert_with(|| def.clone());
                        
                        // Add constructors for this type (only once per type)
                        if type_def_entry.constructors.is_empty() {
                            type_def_entry.constructors = def.constructors.clone();
                        }
                        
                        // Then add functions from THIS SPECIFIC call chain
                        // Find the index of this path in the paths collection (using safe comparison)
                        let path_index = {
                            let mut idx = 0;
                            let mut found = false;
                            
                            // Look for a matching path by comparing full_path values
                            for (i, other_path) in paths.iter().enumerate() {
                                if other_path.len() == path.len() {
                                    let mut all_match = true;
                                    for (j, node) in path.iter().enumerate() {
                                        if other_path[j].full_path != node.full_path {
                                            all_match = false;
                                            break;
                                        }
                                    }
                                    if all_match {
                                        idx = i;
                                        found = true;
                                        break;
                                    }
                                }
                            }
                            
                            if found { idx + 1 } else { 1 }
                        };
                        
                        for (step_index, node) in path.iter().enumerate() {
                            // Only include functions that are actually in this path and operate on the type
                            let operates_on_type = node.param_custom_types.contains(type_name) || 
                                                  node.return_custom_types.contains(type_name);
                            
                            if operates_on_type {
                                // Extract function name
                                let fn_name = node.full_path.split("::").last().unwrap_or(&node.full_path);
                                
                                // Check how function uses this type
                                let relation_type = if node.param_custom_types.contains(type_name) && 
                                                      node.return_custom_types.contains(type_name) {
                                    "parameters and return value"
                                } else if node.param_custom_types.contains(type_name) {
                                    "parameters"
                                } else {
                                    "return value"
                                };
                                
                                // Format as impl block style, and add step comments
                                let method_code = format!(
                                    "impl {} {{\n    // Call chain #{} - Step #{} - Function: {} - Uses type as: {}\n    {}\n}}", 
                                    def_name,
                                    path_index,
                                    step_index + 1,
                                    fn_name,
                                    relation_type,
                                    utils::enhanced_format_source_code(&node.source_code)
                                );
                                
                                // Check if this function implementation is already in the list
                                let mut already_exists = false;
                                for existing_code in &type_def_entry.constructors {
                                    if existing_code.contains(&node.source_code) {
                                        already_exists = true;
                                        break;
                                    }
                                }
                                
                                // Add to results if not a duplicate
                                if !already_exists {
                                    type_def_entry.constructors.push(method_code);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(Some(FileAnalysisResult {
            file_path: file_path_str,
            paths,
            type_definitions: path_type_defs,
        }))
    }

    /// Parallel analyze directory
    pub fn analyze_directory_parallel(&self, dir_path: &Path) -> io::Result<()> {
        let start_time = Instant::now();
        
        // Collect all Rust file paths
        let rust_files = self.collect_rust_files(dir_path)?;
        let total_files = rust_files.len();
        
        println!("Found {} Rust files, starting parallel analysis...", total_files);
        
        // Create progress counter
        let processed_count = Arc::new(Mutex::new(0));
        let results = self.results.clone();
        let error_count = Arc::new(Mutex::new(0));
        
        // Process files in parallel, using rayon's try_for_each to catch possible errors
        let process_result = rust_files.par_iter().try_for_each(|path| -> Result<(), io::Error> {
            // Use catch_unwind to capture serious errors, prevent a single file from stopping all processing
            let file_result = std::panic::catch_unwind(|| {
                self.analyze_file(path)
            });
            
            match file_result {
                Ok(Ok(Some(file_result))) => {
                    // Normal case: file analysis successful with results
                    let mut results_guard = results.lock().unwrap();
                    results_guard.push(file_result);
                },
                Ok(Ok(None)) => {
                    // Normal case: file analysis successful but no results
                },
                Ok(Err(e)) => {
                    // File IO error
                    eprintln!("File IO error analyzing {}: {}", path.display(), e);
                    let mut count = error_count.lock().unwrap();
                    *count += 1;
                },
                Err(_) => {
                    // Parsing error or other serious error
                    eprintln!("Serious error occurred while parsing {}", path.display());
                    let mut count = error_count.lock().unwrap();
                    *count += 1;
                }
            }
            
            // Update progress
            let mut count = processed_count.lock().unwrap();
            *count += 1;
            if *count % 100 == 0 || *count == total_files {
                println!("Processed: {}/{} files ({:.1}%) Time: {:?}", 
                         *count, total_files, 
                         (*count as f64 / total_files as f64) * 100.0,
                         start_time.elapsed());
            }
            
            // Continue to next file
            Ok(())
        });
        
        // Handle overall error
        if let Err(e) = process_result {
            eprintln!("Error occurred during parallel file processing: {}", e);
        }
        
        let error_count = *error_count.lock().unwrap();
        println!("Analysis complete! Processed {} files, {} files had errors, Time: {:?}", 
                 total_files, error_count, start_time.elapsed());
        
        Ok(())
    }
    
    /// Collect all Rust files in directory
    pub fn collect_rust_files(&self, dir_path: &Path) -> io::Result<Vec<PathBuf>> {
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
    
    /// Add a result to the results collection
    pub fn add_result(&self, result: FileAnalysisResult) {
        let mut guard = self.results.lock().unwrap();
        guard.push(result);
    }
    
    /// Get analysis results
    pub fn get_results(&self) -> Vec<FileAnalysisResult> {
        let guard = self.results.lock().unwrap();
        guard.clone()
    }

    /// Write results to file - new implementation that groups paths by destination unsafe function
    pub fn write_results_to_file(&self, output_path: &Path) -> io::Result<()> {
        println!("Writing results to: {}", output_path.display());
        
        let file = File::create(output_path)?;
        let mut writer = BufWriter::new(file);
        
        // 添加文件头部注释和模块声明
        writeln!(writer, "// 自动生成的Rust代码文件 - 包含不安全函数调用路径分析结果")?;
        writeln!(writer, "// 此文件可以被编译器解析，具有语法高亮")?;
        writeln!(writer, "\n// 注意：此文件仅用于查看，不应直接编译或运行")?;
        writeln!(writer, "// 生成时间: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"))?;
        writeln!(writer, "\n#![allow(dead_code)]")?;
        writeln!(writer, "#![allow(unused_variables)]")?;
        writeln!(writer, "#![allow(unused_imports)]")?;
        writeln!(writer, "#![allow(non_snake_case)]")?;
        writeln!(writer, "\n// 分析结果开始\n")?;
        
        let results = self.get_results();
        
        for result in &results {
            if result.paths.is_empty() {
                continue;
            }
            
            // 文件标题作为模块注释
            writeln!(writer, "// ============================================================")?;
            writeln!(writer, "// 文件: {}", result.file_path)?;
            writeln!(writer, "// ============================================================\n")?;
            
            // 为每个文件创建一个模块
            let module_name = Path::new(&result.file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown_module")
                .replace("-", "_")
                .replace(".", "_");
            
            writeln!(writer, "pub mod {} {{", module_name)?;
            
            // Group paths by their destination function (the unsafe function)
            let mut paths_by_destination: HashMap<String, Vec<Vec<PathNodeInfo>>> = HashMap::new();
            
            // Collect all paths leading to the same unsafe function
            for path in &result.paths {
                if path.is_empty() {
                    continue;
                }
                
                // The last function in the path is the unsafe function
                let unsafe_fn = &path.last().unwrap().full_path;
                paths_by_destination.entry(unsafe_fn.clone())
                    .or_default()
                    .push(path.clone());
            }
            
            writeln!(writer, "    // 发现 {} 组通向不安全函数的路径", paths_by_destination.len())?;
            
            // Process each group of paths leading to the same unsafe function
            for (group_idx, (unsafe_fn, paths)) in paths_by_destination.into_iter().enumerate() {
                // 为每个组创建一个子模块
                let unsafe_fn_name = unsafe_fn.split("::").last().unwrap_or(&unsafe_fn);
                let group_module_name = format!("group_{}", group_idx + 1);
                
                writeln!(writer, "\n    // 组 {}: 通向不安全函数的路径: {}", group_idx + 1, unsafe_fn_name)?;
                writeln!(writer, "    pub mod {} {{", group_module_name)?;
                
                // 添加路径信息作为注释
                writeln!(writer, "        // 路径列表:")?;
                for (path_idx, path) in paths.iter().enumerate() {
                    writeln!(writer, "        // {}.{} {}", 
                        group_idx + 1, 
                        path_idx + 1, 
                        Self::format_path_with_visibility(path))?;
                }
                
                // Collect all unique functions involved in any path of this group
                let mut all_functions = HashSet::new();
                let mut entry_functions = HashSet::new();
                let mut all_types = HashSet::new();
                
                for path in &paths {
                    if path.is_empty() {
                        continue;
                    }
                    
                    // Add all functions in this path
                    for node in path {
                        all_functions.insert(node.full_path.clone());
                    }
                    
                    // Add the first function (entry point) to track types
                    entry_functions.insert(path[0].full_path.clone());
                    
                    // Collect custom types from entry function parameters
                    let param_types = &path[0].param_custom_types;
                    for type_name in param_types {
                        for (type_path, _) in &result.type_definitions {
                            if let Some(def_name) = type_path.split("::").last() {
                                if def_name == type_name {
                                    all_types.insert(type_path.clone());
                                }
                            }
                        }
                    }
                }
                
                // List all relevant type definitions
                if !all_types.is_empty() {
                    writeln!(writer, "\n        // 相关自定义类型定义:")?;
                    for type_path in &all_types {
                        if let Some(type_def) = result.type_definitions.get(type_path) {
                            writeln!(writer, "        // 类型: {}", type_path)?;
                            
                            // Output type definition with proper indentation
                            let formatted_type = utils::beautify_source_code(&type_def.source_code);
                            
                            // 处理可能的冗余pub关键字
                            let visibility_prefix = type_def.visibility.to_string();
                            
                            // 将格式化后的代码分割成行
                            let lines: Vec<&str> = formatted_type.lines().collect();
                            let mut processed_lines = Vec::new();
                            
                            for line in lines {
                                let trimmed = line.trim();
                                
                                // 检查是否是结构体或枚举的开始
                                if trimmed.contains("struct ") || trimmed.contains("enum ") {
                                    // 如果这行包含pub并且我们要添加pub前缀
                                    if trimmed.starts_with("pub ") && !visibility_prefix.is_empty() {
                                        // 移除原有的pub
                                        let without_pub = trimmed.replacen("pub ", "", 1);
                                        processed_lines.push(format!("        {}{}", visibility_prefix, without_pub));
                                    } else if !trimmed.starts_with("pub ") && !visibility_prefix.is_empty() {
                                        // 添加可见性前缀
                                        processed_lines.push(format!("        {}{}", visibility_prefix, trimmed));
                                    } else {
                                        // 保持原样，只添加缩进
                                        processed_lines.push(format!("        {}", trimmed));
                                    }
                                } else {
                                    // 其他行（包括注释和结构体结束的大括号）保持原样，只添加缩进
                                    processed_lines.push(format!("        {}", line.trim_matches(' ')));
                                }
                            }
                            
                            // 将处理后的行合并成最终的字符串
                            let final_output = processed_lines.join("\n");
                            writeln!(writer, "{}", final_output)?;
                            
                            // Output constructors once with proper indentation
                            for constructor in &type_def.constructors {
                                // Only output constructors, not path-specific methods
                                if !constructor.contains("Call chain") {
                                    let formatted_constructor = utils::beautify_source_code(constructor)
                                        .lines()
                                        .map(|line| format!("        {}", line))
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    
                                    writeln!(writer, "\n{}", formatted_constructor)?;
                                }
                            }
                            
                            writeln!(writer, "")?;
                        }
                    }
                }
                
                // Output implementations of all functions in this group
                writeln!(writer, "        // 函数实现:")?;
                
                // 1. Output the public entry point functions first
                let mut seen_entry_points = HashSet::new();
                for path in &paths {
                    if !path.is_empty() {
                        let entry_node = &path[0];
                        if seen_entry_points.insert(entry_node.full_path.clone()) {
                            writeln!(writer, "        // 公共入口点: {}", entry_node.full_path)?;
                            let source_code = utils::beautify_source_code(&entry_node.source_code)
                                .lines()
                                .map(|line| format!("        {}", line))
                                .collect::<Vec<_>>()
                                .join("\n");
                            
                            writeln!(writer, "{}", source_code)?;
                            writeln!(writer, "")?;
                        }
                    }
                }
                
                // 2. Output the unsafe destination function
                for path in &paths {
                    if !path.is_empty() {
                        let unsafe_node = path.last().unwrap();
                        writeln!(writer, "        // 不安全实现: {}", unsafe_node.full_path)?;
                        let source_code = utils::beautify_source_code(&unsafe_node.source_code)
                            .lines()
                            .map(|line| format!("        {}", line))
                            .collect::<Vec<_>>()
                            .join("\n");
                        
                        writeln!(writer, "{}", source_code)?;
                        writeln!(writer, "")?;
                        break; // Only need to output it once
                    }
                }
                
                // 3. Output any intermediate functions (that aren't entry points or the unsafe function)
                // FIXED: Use a HashMap instead of HashSet to avoid the Hash trait requirement
                let mut intermediate_functions = HashMap::new();
                for path in &paths {
                    if path.len() > 2 { // Only paths with intermediates
                        for i in 1..path.len()-1 {
                            let node = &path[i];
                            intermediate_functions.insert(node.full_path.clone(), node);
                        }
                    }
                }
                
                for (_, node) in intermediate_functions {
                    writeln!(writer, "        // 中间函数: {}", node.full_path)?;
                    let source_code = utils::beautify_source_code(&node.source_code)
                        .lines()
                        .map(|line| format!("        {}", line))
                        .collect::<Vec<_>>()
                        .join("\n");
                    
                    writeln!(writer, "{}", source_code)?;
                    writeln!(writer, "")?;
                }
                
                // 关闭组模块
                writeln!(writer, "    }} // end of module {}", group_module_name)?;
            }
            
            // 关闭文件模块
            writeln!(writer, "}} // end of module {}\n", module_name)?;
        }
        
        println!("成功写入 {} 个文件的分析结果", results.len());
        Ok(())
    }
    
    /// Format path with visibility information
    fn format_path_with_visibility(path: &[PathNodeInfo]) -> String {
        path.iter()
            .enumerate()
            .map(|(i, node)| {
                // Get last part of function name (without module path)
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