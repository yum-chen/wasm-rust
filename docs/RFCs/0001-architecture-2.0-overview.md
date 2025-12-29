# RFC 0001: Architecture 2.0 Overview - Dual Compilation Strategy

## 元数据

- **RFC编号**: 0001
- **标题**: Architecture 2.0 Overview - Dual Compilation Strategy  
- **作者**: WasmRust Core Team
- **状态**: Draft
- **创建日期**: 2024-01-15
- **最后更新**: 2024-01-15

## 摘要

本RFC提出了WasmRust Architecture 2.0的核心架构，定义了一个支持**双重编译策略**的系统，允许同一份Rust代码同时面向原生系统和WebAssembly目标进行优化编译，特别针对WASM的GC特性提供专门支持。

## 动机

当前Rust到WASM的编译存在以下限制：

1. **语义损失**: 传统编译管道过早丢失WASM特定语义
2. **优化有限**: 无法针对WASM GC特性进行专门优化  
3. **生态分裂**: 需要为WASM目标维护单独的代码分支
4. **迁移困难**: 从原生到WASM的迁移需要大量重构

Architecture 2.0旨在解决这些问题，提供：
- 统一的代码库支持双重目标
- GC就绪的编译策略
- 渐进式迁移路径
- 性能优化潜力

## 详细设计

### 核心概念：双重身份编译器

WasmRust Architecture 2.0将编译器重新定义为**双重身份系统**：

```
同一Rust源码 → 编译器上下文切换 → 目标特定优化
    │                          │
    ├─ 原生目标: 传统所有权语义
    └─ WASM目标: GC就绪语义 + 专门优化
```

### 技术架构

#### 1. 条件编译基础设施

```rust
// 架构2.0核心: 条件性GC语义
#[wasm::gc]  // 编译时属性，仅WASM目标生效
struct GcString {
    inner: String,  // 在WASM中映射到GC字符串
}

// 条件类型别名系统
#[cfg(target_family = "wasm")]
type PlatformString = wasm::gc::GcString;

#[cfg(not(target_family = "wasm"))]  
type PlatformString = std::string::String;
```

#### 2. 双重Lowering策略

**阶段1: 通用Lowering (MIR级别)**
- 统一的前端处理
- 语义分析和验证
- 通用优化通道

**阶段2: 目标特定Lowering**
- **原生目标**: 标准所有权Lowering
- **WASM目标**: GC-aware Lowering + 专门优化

#### 3. GC运行时集成

```rust
// GC类型系统接口
trait GcManaged {
    fn gc_mark(&self);
    fn gc_trace(&self);
}

// 主机GC集成
trait HostGcIntegration {
    fn allocate_gc<T: GcManaged>(value: T) -> GcRef<T>;
    fn collect_garbage();
}
```

### 编译管道重构

#### 现有管道 (Architecture 1.0)
```
Rust Source → HIR → MIR → LLVM → WASM
```

#### 新管道 (Architecture 2.0)
```
Rust Source → HIR → MIR → WasmIR (语义边界)
    ├─ Cranelift Backend (开发构建)
    └─ LLVM Backend (发布构建)
        └─ wasm-opt → Component Model包装
```

### 关键组件

#### 1. WasmIR - 稳定语义边界
- 捕获WASM特定语义（引用类型、线性内存等）
- 提供稳定的优化接口
- 支持双重后端代码生成

#### 2. 条件属性系统
- `#[wasm::gc]` - GC语义标记
- `#[wasm::linear]` - 线性类型支持  
- `#[wasm::component]` - 组件模型集成

#### 3. 双重后端架构
- **Cranelift**: 快速开发迭代（~2秒构建）
- **LLVM**: 生产级优化（最大性能）

## 实施计划

### 阶段0: 架构基础 (当前)
- [ ] RFC文档和设计验证
- [ ] 条件编译基础设施原型
- [ ] GC类型系统设计

### 阶段1: 核心功能 (3-6个月)
- [ ] WasmIR实现和集成
- [ ] 双重后端支持
- [ ] 基础GC类型系统

### 阶段2: 功能完善 (6-12个月)  
- [ ] 完整GC运行时集成
- [ ] 组件模型支持
- [ ] 性能优化和基准测试

### 阶段3: 生产就绪 (12-18个月)
- [ ] 稳定性和性能验证
- [ ] 生态系统集成
- [ ] 文档和工具完善

## 向后兼容性

### 保证级别
- **完全兼容**: 现有Rust代码无需修改继续工作
- **渐进增强**: 可选择性地采用GC优化
- **无破坏性变更**: 所有变更都是可选的增强

### 迁移路径
1. **阶段0**: 现有代码无修改工作
2. **阶段1**: 选择性添加GC注解进行优化
3. **阶段2**: 完全GC就绪，享受完整性能优势

## 替代方案考虑

### 方案A: 语言分叉
- **优点**: 更激进的WASM优化
- **缺点**: 生态系统分裂，维护负担重
- **拒绝理由**: 不符合兼容性目标

### 方案B: 纯库方案  
- **优点**: 简单，无需编译器修改
- **缺点**: 优化潜力有限，语义表达能力弱
- **拒绝理由**: 无法实现GC级别的优化

### 方案C: 运行时GC切换
- **优点**: 动态适应性
- **缺点**: 运行时开销，复杂性高
- **拒绝理由**: 性能不可预测

## 未解决的问题

1. **GC性能边界**: GC与非GC代码的交互性能影响
2. **工具链集成**: 调试和分析工具的GC感知
3. **多语言互操作**: 与其他WASM语言的GC交互

## 未来工作

1. **高级GC特性**: 分代GC、并发GC支持
2. **组件模型扩展**: 完整的WIT支持
3. **性能分析工具**: GC-aware性能分析

## 总结

Architecture 2.0代表了WasmRust从"Rust到WASM编译器"到"混合系统语言平台"的战略转型。通过双重编译策略和条件性GC支持，我们能够在保持Rust生态系统完整性的同时，为WebAssembly目标提供一流的性能和特性支持。

这项工作的成功将确立Rust作为WebAssembly一等公民的地位，为未来的多语言、多目标系统开发奠定坚实基础。