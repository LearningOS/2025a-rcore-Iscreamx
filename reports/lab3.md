# Chapter 5 实验报告


## 编程作业

- 迁移 sys_get_time、sys_mmap、sys_munmap 适配新进程结构。sys_get_time 使用 translated_byte_buffer 处理跨页数据；sys_mmap/munmap 通过处理器的内存管理接口实现用户态地址空间的动态映射与解映射。
- 实现 sys_spawn（ID 400）直接从 ELF 创建新进程。与 fork+exec 不同，spawn 通过 MemorySet::from_elf 直接解析并建立新地址空间，避免复制父进程内存的开销，分配新 PID 和内核栈后设置父子关系并加入调度队列。
- 在 TCB 中添加 stride（初始 0）和 pass（初始 16）字段。就绪队列按 stride 升序排序，每次取最小者执行并更新 stride += pass。实现 sys_set_priority（ID 140），通过 pass = BIG_STRIDE / priority 确保 CPU 时间按优先级比例分配。

## 简答作业

1. stride 算法原理非常简单，但是有一个比较大的问题。例如两个 pass = 10 的进程，使用 8bit 无符号整形储存 stride， p1.stride = 255, p2.stride = 250，在 p2 执行一个时间片后，理论上下一次应该 p1 执行。
- 实际情况是轮到 p1 执行吗？为什么？
  - 不会, 8bit的stride最大为255, (250 + 10) % 256 = 4 < 255, p2的stride溢出导致stride反而更小了.

我们之前要求进程优先级 >= 2 其实就是为了解决这个问题。可以证明， 在不考虑溢出的情况下 , 在进程优先级全部 >= 2 的情况下，如果严格按照算法执行，那么 STRIDE_MAX – STRIDE_MIN <= BigStride / 2。

- 为什么？尝试简单说明（不要求严格证明）。
  - 当优先级 >= 2时, pass <= BigStride / 2, 若某进程一直不被调度, 其余进程的stride每次增加pass <= BigStride / 2;
由于每次增加量 ≤ BigStride / 2，当差距接近 BigStride / 2 时，原本不被调度的进程会变成"最小 stride"，开始被调度，从而防止差距进一步扩大。
- 已知以上结论，考虑溢出的情况下，可以为 Stride 设计特别的比较器，让 BinaryHeap<Stride> 的 pop 方法能返回真正最小的 Stride。补全下列代码中的 partial_cmp 函数，假设两个 Stride 永远不会相等。
```
use core::cmp::Ordering;

struct Stride(u64);

impl PartialOrd for Stride {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        const BIG_STRIDE: u64 = u64::MAX;
        let diff = self.0.wrapping_sub(other.0);
        
        if diff < BIG_STRIDE / 2 {
            Some(Ordering::Greater)
        } else {
            Some(Ordering::Less)
        }
    }
}

impl PartialEq for Stride {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}
```



## 荣誉准则
- 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：
  - 无
- 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：
  - [rCore-Tutorial-Book 第三版](https://rcore-os.cn/rCore-Tutorial-Book-v3/index.html)
  - [RISC-V 手册](http://riscvbook.com/chinese/RISC-V-Reader-Chinese-v2p1.pdf)
  - [测试用例](https://github.com/LearningOS/rCore-Tutorial-Test-2025S)
- 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。
- 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。