# WasmRust 编译器系统需求文档

## 项目概述

WasmRust 是一个专门优化的 Rust 到 WebAssembly 编译系统，旨在解决标准 Rust WASM 工具链的当前限制。系统提供五层架构，实现最小二进制大小、快速编译时间、无缝组件模型集成、高效的 JavaScript 互操作性，同时保持 Rust 的内存安全保证。

**设计理念**: WasmRust 是 rustc 扩展而非语言分支，采用以下实现策略:
- 核心部分 (80%): 标准 Rust 编译器 + 自定义代码生成后端
- 扩展部分 (15%): `wasm` crate + 过程宏实现 WASM 特定功能
- 编译器插件 (4%): `-Z` 不稳定标志用于高级优化
- 硬分支 (<1%): 仅在绝对必要时进行最小不兼容更改

**兼容性保证**: 标准 Rust 代码无需修改即可编译。WASM 特定功能通过 `wasm` crate 可选启用。

## 核心需求场景

### 场景 1: Web 应用开发优化
Web 开发者需要最小化的 WASM 二进制文件，以确保在移动设备和低带宽连接上快速加载应用。

### 场景 2: 快速开发迭代
开发者需要在开发过程中快速编译，以便快速迭代 WASM 应用。

### 场景 3: 跨语言组件集成
系统架构师希望组合来自不同语言的 WASM 模块，以便为每个组件使用最佳工具。

### 场景 4: 高性能 Web 应用
性能敏感的开发者需要在支持 WASM 环境中进行安全的并发编程，以便在可用时利用多核。

## 架构技术方案

### 五层架构设计

**第 1 层: 核心语言层**
- WASM 原生类型 (ExternRef, FuncRef, SharedSlice)
- 线性类型资源管理
- 安全抽象层

**第 2 层: 语言扩展层**
- 组件模型宏和 WIT 集成
- 能力注解系统
- 生命周期管理

**第 3 层: 运行时服务层**
- 内存管理 (Scoped arenas, 内存区域)
- 线程运行时 (结构化并发)
- 组件链接

**第 4 层: 编译器后端层**
- Cranelift 后端 (开发模式)
- LLVM 后端 (发布模式)
- 配置文件引导优化 (PGO)

**第 5 层: 工具分发层**
- cargo-wasm 命令行工具
- 注册表联邦系统
- 调试和分析工具

### 双后端编译策略

**开发配置 (Cranelift)**:
- 目标: 10k 行代码 < 5 秒编译
- 特性: 快速编译、可调试输出、确定性布局

**发布配置 (LLVM)**:
- 目标: 最大性能和大小优化
- 特性: 全 LLVM 优化流程 + wasm-opt、PGO 支持

## 主要功能需求

### 需求 1: 二进制大小优化
- 独立配置下 "hello world" 程序 < 15 KB
- 标准库应用大小最多为等效 C 程序的 3 倍
- 泛型函数的薄单态化减少 30%+ 代码重复
- 死代码消除和树摇优化
- 按函数大小分析工具

### 需求 2: 快速编译性能
- 10k 行代码开发模式 < 5 秒编译
- Cranelift 后端比 LLVM 快 5x+
- 增量编译支持
- 开发和发布构建配置分离

### 需求 3: 内存安全和类型系统
- 在 WasmRust 方言约束内强制执行 Rust 所有权和借用规则
- SharedSlice 类型通过类型系统约束防止数据竞争
- 线性类型强制执行 WASM 资源的一次使用语义
- ExternRef 和 FuncRef 的安全抽象

### 需求 4: JavaScript 互操作性
- 支持主机配置文件中的 JavaScript 函数零拷贝数据传输
- 调用边界成本 < 100 纳秒
- Pod 数据的直接内存访问避免序列化
- 类型安全的 JavaScript 对象访问

### 需求 5: 组件模型集成
- 生成组件模型兼容的 WASM 模块
- 双向 WIT 代码生成
- 外部模块的类型安全绑定
- 变化感知泛型支持组件替换

### 需求 6: 线程和并发
- 支持线程环境的结构化并发
- SharedSlice 类型实现编译时数据竞争预防
- 高效 WASM 原子指令生成
- 线程能力检测和单线程回退

### 需求 7: 开发工具链
- cargo-wasm 命令行项目管理工具
- 线性内存布局可视化调试器
- 运行时性能数据收集分析器
- 按函数和模块的大小分析器

## 实现细节

### 核心类型定义

```rust
// ExternRef: JavaScript 对象的托管引用
#[repr(transparent)]
pub struct ExternRef<T> {
    handle: u32, // 运行时引用表索引
    _marker: PhantomData<T>,
}

// FuncRef: 托管函数引用
#[repr(transparent)]
pub struct FuncRef {
    handle: u32, // 函数表索引
}

// SharedSlice: 安全的共享内存访问
pub struct SharedSlice<'a, T: wasm::Pod> {
    ptr: *const T,
    len: usize,
    _marker: PhantomData<&'a [T]>,
}
```

### 线性类型系统

```rust
#[wasm::linear]
struct CanvasContext(wasm::Handle);

impl CanvasContext {
    fn draw(&mut self) { /* ... */ }
    fn into_bitmap(self) -> ImageData { /* 消费方法，移动所有权 */ }
}
```

### 组件模型集成

```rust
#[wasm::component(name = "image-filter", version = "1.0.0")]
mod filter {
    use wasm::{f32x4, externref, memory::View};

    #[wasm::import("gfx@2.1")]
    extern "wasm" { 
        fn convolve_3x3(pixels: View<u8>, kernel: [f32; 9]) -> Vec<u8>; 
    }

    #[wasm::export]
    pub fn sharpen(pixels: View<u8>, width: u32, height: u32) -> Vec<u8> {
        let kernel = [0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0];
        convolve_3x3(pixels, kernel)
    }
}
```

## 影响文件

### 核心编译器文件
- `src/lib.rs` - 主编译器入口点
- `src/frontend/` - Rust 前端处理
- `src/wasmir/` - WasmIR 中间表示
- `src/backend/cranelift/` - Cranelift 后端实现
- `src/backend/llvm/` - LLVM 后端实现

### 语言扩展文件
- `src/rtypes/` - WASM 原生类型定义
- `crates/wasm/src/lib.rs` - wasm 核心库
- `crates/wasm-macros/src/lib.rs` - 过程宏实现

### 运行时服务文件
- `src/runtime/memory.rs` - 内存管理
- `src/runtime/threading.rs` - 线程运行时
- `src/runtime/components.rs` - 组件链接

### 工具链文件
- `crates/cargo-wasm/src/main.rs` - 命令行工具
- `src/registry/` - 注册表系统
- `src/debugger/` - 调试工具

## 边界条件与异常处理

### 编译时错误处理
- 线性类型违规检测和报告
- 组件 ABI 不兼容验证
- 能力缺失的编译时警告

### 运行时错误恢复
- 线程不可用时的单线程回退
- 组件加载失败时的降级处理
- 内存区域意图验证失败

### 跨平台兼容性
- 主机配置文件能力检测
- 不同执行环境的适配
- 功能不可用时的优雅降级

## 数据流动路径

### 编译流程
1. Rust 源码 → HIR/MIR
2. HIR/MIR → WasmIR (稳定边界)
3. WasmIR → {Cranelift|LLVM} 后端
4. 后端输出 → wasm-opt 优化
5. 优化结果 → 组件模型包装

### 运行时流程
1. 组件加载和验证
2. 主机配置文件能力检测
3. 内存区域意图验证
4. 运行时服务初始化
5. 执行环境准备

### 互操作流程
1. JavaScript 函数调用
2. 参数序列化/反序列化
3. 托管引用表管理
4. 所有权语义处理
5. 结果返回

## 预期成果

### 技术指标
- 二进制大小: 独立程序 < 15KB，标准库应用 ≤ C 程序 3 倍
- 编译速度: 10k 行代码开发模式 < 5 秒
- 运行性能: JavaScript 调用边界成本 < 100ns
- 内存安全: 编译时预防所有内存安全违规

### 开发体验
- 无缝的 Rust 开发工作流集成
- 丰富的调试和分析工具
- 清晰的错误消息和修复建议
- 完整的文档和示例

### 生态系统
- 组件注册表联邦系统
- 多语言组件支持
- 现有 crates.io 生态兼容
- 企业级私有注册表支持

## 安全考虑

### 编译器安全
- 输入源码验证和恶意模式检测
- 生成产物的密码学签名
- 编译单元间隔离

### 运行时安全
- 组件模型安全边界强制执行
- 跨组件调用的运行时验证
- 组件间未授权内存访问防护

### 注册表安全
- 所有发布组件的密码学签名要求
- 组件下载和更新的审计跟踪
- 受损组件的撤销支持
