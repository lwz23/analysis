# Rust代码安全性分析工具

这是一个用于静态分析Rust代码中unsafe代码路径的工具。该工具可以识别从公开API到内部unsafe代码块的调用路径，帮助开发者理解和审查项目中的不安全代码使用情况。

## 功能特点

- 分析Rust项目中从公开API到unsafe代码块的调用路径
- 支持并行处理大型代码库，提高分析效率
- 识别自定义类型和它们在不安全代码路径中的使用情况
- 生成详细的分析报告，包括完整的调用链和相关源代码
- 具备超时机制和文件大小限制，防止处理超大文件时资源耗尽
- 错误处理机制，确保即使部分文件分析失败也不会中断整体分析过程

## 安装

确保您的系统上已安装Rust及Cargo工具链。

```bash
# 克隆仓库
git clone [仓库URL]
cd analysis

# 编译项目
cargo build --release
```

## 使用方法

```bash
# 分析单个Rust文件
cargo run --release -- path/to/file.rs [输出文件路径]

# 分析整个Rust项目目录
cargo run --release -- path/to/rust/project [输出文件路径]
```

如果不指定输出文件路径，结果将保存在当前目录下，文件名基于输入路径自动生成。

## 配置选项

该工具提供了一些默认配置常量，可以在源码中修改：

- `DEFAULT_MAX_SEARCH_DEPTH`: 搜索调用链的最大深度 (默认: 20)
- `DEFAULT_FILE_SIZE_LIMIT`: 分析文件的大小上限，单位MB (默认: 10MB)
- `DEFAULT_TIMEOUT_SECONDS`: 分析单个文件的超时时间，单位秒 (默认: 30秒)

## 项目结构

```
analysis/
├── src/
│   ├── main.rs            # 程序入口点
│   ├── lib.rs             # 库定义和常量
│   ├── models.rs          # 数据模型定义
│   ├── utils.rs           # 工具函数
│   ├── analysis/          # 分析模块
│   │   ├── analyzer.rs    # 静态分析器实现
│   │   ├── call_graph.rs  # 调用图构建
│   │   └── mod.rs         # 模块定义
│   └── visitors/          # 代码访问器
│       ├── function.rs    # 函数信息收集访问器
│       ├── call.rs        # 函数调用关系访问器
│       └── mod.rs         # 模块定义
├── tests/                 # 测试文件
├── Cargo.toml             # 项目依赖定义
└── Cargo.lock             # 依赖锁定文件
```

## 技术实现

该工具基于以下核心技术实现：

- **syn**: 用于解析Rust代码，支持完整的语法树遍历和分析
- **rayon**: 用于并行处理多个文件，提高分析效率
- **walkdir**: 用于遍历目录树，收集所有待分析的Rust源码文件

该工具通过两阶段分析过程实现：
1. 首先收集项目中所有函数定义和它们的安全性信息
2. 然后分析函数间的调用关系，构建从公开API到unsafe代码块的调用路径

## 输出结果

分析结果以Rust源码的形式保存，包含以下信息：

- 从公开API到unsafe代码块的完整调用路径
- 每个函数的可见性信息
- 相关的自定义类型定义
- 路径中的源代码片段

## 限制条件

- 当前版本不支持分析宏展开内部的unsafe代码
- 跨crate的调用链分析可能不完整
- 对于非常大型的项目，可能需要调整默认配置参数

## 贡献指南

欢迎提交问题报告和功能请求！如果您想贡献代码，请遵循以下步骤：

1. Fork本仓库
2. 创建您的特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交您的更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 打开一个Pull Request

## 许可证

[添加您的许可证信息] 