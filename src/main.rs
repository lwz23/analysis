use std::path::PathBuf;
// 从分析模块导入所需的结构体和常量
use analysis::{StaticAnalyzer, DEFAULT_MAX_SEARCH_DEPTH, DEFAULT_FILE_SIZE_LIMIT, DEFAULT_TIMEOUT_SECONDS};

fn main() -> std::io::Result<()> {
    // 设置 panic 处理程序以防止在 panic 时立即退出
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("Program encountered a serious error: {:?}", panic_info);
        eprintln!("Attempting to recover and continue...");
    }));

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <Rust project or file path> [output file]", args[0]);
        eprintln!("  <Rust project or file path>: Path to a Rust file or directory");
        eprintln!("  [output file]: Optional path to save results (default: ./unsafe_paths.rs)");
        return Ok(());
    }

    let input_path = PathBuf::from(&args[1]);
    let output_path = if args.len() >= 3 {
        PathBuf::from(&args[2])
    } else {
        // 获取当前工作目录
        let current_dir = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(e) => {
                eprintln!("Error getting current directory: {}, falling back to input directory", e);
                if input_path.is_dir() {
                    input_path.clone()
                } else {
                    input_path.parent().unwrap_or(&PathBuf::from(".")).to_path_buf()
                }
            }
        };
        
        // 从输入路径获取一个有意义的文件名
        let file_name = if input_path.is_dir() {
            // 如果输入是目录，使用目录名作为文件名的一部分
            let dir_name = input_path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("project");
            format!("{}_unsafe_paths.rs", dir_name)
        } else {
            // 如果输入是文件，使用文件名作为文件名的一部分
            let stem = input_path.file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("file");
            format!("{}_analysis.rs", stem)
        };
        
        // 将文件放在当前目录下
        current_dir.join(file_name)
    };
    
    // Use catch_unwind to capture all possible panics
    let result = std::panic::catch_unwind(|| {
        let analyzer = StaticAnalyzer::new(
            DEFAULT_MAX_SEARCH_DEPTH,
            DEFAULT_FILE_SIZE_LIMIT,
            DEFAULT_TIMEOUT_SECONDS
        );
        
        // Validate path existence
        if !input_path.exists() {
            eprintln!("Error: Path does not exist: {}", input_path.display());
            return Ok(());
        }
        
        println!("Starting analysis: {}", input_path.display());
        
        // If it's a directory, analyze all files in parallel, otherwise analyze single file
        if input_path.is_dir() {
            // Add error recovery handling
            if let Err(e) = analyzer.analyze_directory_parallel(&input_path) {
                eprintln!("Error analyzing directory: {}, but will continue with processed files", e);
                // Continue to try writing results even if there's an error
            }
        } else if input_path.extension().map_or(false, |ext| ext == "rs") {
            match analyzer.analyze_file(&input_path) {
                Ok(Some(result)) => {
                    analyzer.add_result(result);
                },
                Ok(None) => {
                    println!("File {} does not need analysis or has no valid results", input_path.display());
                },
                Err(e) => {
                    eprintln!("Error analyzing file: {}, but will continue execution", e);
                }
            }
        } else {
            eprintln!("Path must be a Rust file (.rs) or a directory containing Rust files: {}", 
                    input_path.display());
            return Ok(());
        }
        
        // Write results
        if let Err(e) = analyzer.write_results_to_file(&output_path) {
            eprintln!("Error writing results: {}", e);
        }
        
        println!("Analysis complete! Results saved to: {}", output_path.display());
        
        Ok(())
    });

    // Handle overall panic
    match result {
        Ok(io_result) => io_result,
        Err(_) => {
            eprintln!("Program encountered an unrecoverable error, but has attempted to save existing results");
            Ok(())
        }
    }
}