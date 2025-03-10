use syn;
use std::panic::catch_unwind;

/// Attempt to beautify source code using prettyplease or fall back to basic formatting
pub fn beautify_source_code(source_code: &str) -> String {
    // 检查源代码是否包含注释
    let has_comments = source_code.contains("//") || source_code.contains("/*");
    
    // 如果包含注释，使用enhanced_format_source_code会更好地保留注释
    if has_comments {
        return enhanced_format_source_code(source_code);
    }
    
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
    
    // 如果只有一行并且很长，尝试基于分隔符进行格式化
    if lines.len() == 1 && lines[0].len() > 100 {
        return format_single_long_line(lines[0]);
    }
    
    // 如果是只有少量行数的简单函数，可以直接返回原始代码
    if lines.len() <= 15 {
        return complete_source.to_string();
    }
    
    // Explicitly specify type as usize
    let mut indent_level: usize = 0;
    let mut within_comment = false;
    let mut had_content = false;
    
    for line in lines {
        let trimmed = line.trim();
        
        // Handle multi-line comments
        if trimmed.starts_with("/*") {
            within_comment = true;
        }
        
        if within_comment {
            // 在注释中保持原有格式
            result.push_str(line);
            result.push('\n');
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
        
        // Apply indentation (each level is 4 spaces)
        if !trimmed.starts_with("#") {  // Skip indentation for attributes
            for _ in 0..indent_level {
                result.push_str("    ");
            }
        }
        
        // Add the line content
        result.push_str(trimmed);
        result.push('\n');
        
        // Count opening and closing braces
        let chars: Vec<char> = trimmed.chars().collect();
        let mut string_literal = false;
        let mut char_literal = false;
        
        for i in 0..chars.len() {
            // Skip counting braces in string/char literals
            if chars[i] == '"' && (i == 0 || chars[i-1] != '\\') {
                string_literal = !string_literal;
                continue;
            }
            
            if chars[i] == '\'' && (i == 0 || chars[i-1] != '\\') {
                char_literal = !char_literal;
                continue;
            }
            
            if !string_literal && !char_literal {
                if chars[i] == '{' || chars[i] == '[' || chars[i] == '(' {
                    // But don't add indent for lambdas like |x| { x+1 } if the brace is after |
                    let is_lambda = chars[i] == '{' && i > 0 && chars[i-1] == '|';
                    if !is_lambda {
                        indent_level += 1;
                    }
                } else if chars[i] == '}' || chars[i] == ']' || chars[i] == ')' {
                    // Avoid underflow; we've already adjusted for braces at line start
                    if !(i == 0 && (chars[i] == '}' || chars[i] == ']' || chars[i] == ')')) && indent_level > 0 {
                        indent_level -= 1;
                    }
                }
            }
        }
    }
    
    result
}

/// 格式化单行长代码（简单分割成多行）
fn format_single_long_line(line: &str) -> String {
    let mut result = String::new();
    let mut current_indent = 0;
    let mut in_string = false;
    let mut current_line = String::new();
    
    for ch in line.chars() {
        // 处理字符串字面量
        if ch == '"' {
            in_string = !in_string;
        }
        
        // 只有不在字符串内时才处理缩进和换行
        if !in_string {
            match ch {
                '{' => {
                    current_line.push(ch);
                    result.push_str(&current_line);
                    result.push('\n');
                    current_indent += 1;
                    current_line = " ".repeat(current_indent * 4);
                    continue;
                },
                '}' => {
                    if current_indent > 0 {
                        current_indent -= 1;
                    }
                    if !current_line.trim().is_empty() {
                        result.push_str(&current_line);
                        result.push('\n');
                    }
                    current_line = " ".repeat(current_indent * 4);
                    current_line.push(ch);
                    continue;
                },
                ';' => {
                    current_line.push(ch);
                    result.push_str(&current_line);
                    result.push('\n');
                    current_line = " ".repeat(current_indent * 4);
                    continue;
                },
                _ => {}
            }
        }
        
        current_line.push(ch);
    }
    
    if !current_line.trim().is_empty() {
        result.push_str(&current_line);
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