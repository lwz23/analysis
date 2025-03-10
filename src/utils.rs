use syn;
use std::panic::catch_unwind;

/// Attempt to beautify source code using prettyplease or fall back to basic formatting
pub fn beautify_source_code(source_code: &str) -> String {
    // 尝试使用prettyplease进行格式化
    let result = catch_unwind(|| {
        // 直接格式化方法
        if let Ok(parsed) = syn::parse_str::<syn::File>(&format!("{}", source_code)) {
            return prettyplease::unparse(&parsed);
        }
        
        // 尝试将函数包装在mod中
        let wrapped_code = format!("mod dummy {{ {} }}", source_code);
        if let Ok(parsed) = syn::parse_str::<syn::File>(&wrapped_code) {
            let formatted = prettyplease::unparse(&parsed);
            return extract_from_mod(&formatted);
        }
        
        // 尝试包装为impl块
        let wrapped_code = format!("impl Dummy {{ {} }}", source_code);
        if let Ok(parsed) = syn::parse_str::<syn::File>(&wrapped_code) {
            let formatted = prettyplease::unparse(&parsed);
            return extract_from_impl(&formatted);
        }
        
        // 如果所有尝试都失败，返回原始代码
        source_code.to_string()
    });

    // 如果prettyplease格式化失败，使用基本格式化
    match result {
        Ok(formatted) => formatted,
        Err(_) => {
            // 使用基本格式化作为后备方案
            enhanced_format_source_code(source_code)
        }
    }
}

/// Extract content from a dummy mod
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

/// Extract content from a dummy impl
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

/// Enhanced basic formatter with more robust brace handling
pub fn enhanced_format_source_code(source_code: &str) -> String {
    // First, check if the function is incomplete - if so, try to find the closing brace
    let complete_source = ensure_complete_function(source_code);
    
    let mut result = String::new();
    let lines: Vec<&str> = complete_source.lines().collect();
    
    // Explicitly specify type as usize
    let mut indent_level: usize = 0;
    let mut within_comment = false;
    let mut had_content = false;
    
    for line in lines {
        let trimmed = line.trim();
        
        // Skip doc comments
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

/// Try to ensure a function definition is complete by adding missing closing braces
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