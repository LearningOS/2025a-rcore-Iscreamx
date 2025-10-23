# Chapter 3 实验报告

## 编程作业

功能: 

实现了一个多功能的系统调用sys_trace(系统调用号410), 可根据不同的请求类型执行以下操作:
- 内存读取: 当请求类型为0时, 从制定内存地址读取一个字节的数据并返回. 
- 内存写入: 当请求类型为1时, 向指定内存写入一个字节的数据.
- 系统调用统计: 当请求类型为2时, 返回指定系统调用的调用次数(每个应用分开计数).

设计思路:

通过拓展TaskManagerInner中的结构来为每个应用的每个系统调用维护一个值,记录该系统调用的调用次数, 调用次数的更新在内核态的系统调用分发前进行.

## 简答作业

1. 正确进入 U 态后，程序的特征还应有：使用 S 态特权指令，访问 S 态寄存器后会报错。 请同学们可以自行测试这些内容（运行 三个 bad 测例 (ch2b_bad_*.rs) ）， 描述程序出错行为，同时注意注明你使用的 sbi 及其版本。

   - sbi: RustSBI 版本 0.3.0-alpha.2，适配 RISC-V SBI v1.0.0，实现为 RustSBI-QEMU 版本 0.2.0-alpha.2

   - ch2b_bad_address故意执行一个非法的内存操作, 尝试向内存地址0x0写入一个值, 触发了PageFult异常,被操作系统捕获并终止.
     - 报错信息: [kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003a4, kernel killed it.
     - 报错过程: 当处理器检测到访问地址0x0时, 自动生成页错误异常, 此时硬件自动保存异常状态(sepc: 保存发生异常的指令地址(0x804003a4), scause: 存入异常类型, stval: 存入导致错误的地址(0x0)). 之后处理器从用户模式自动切换到监管者模式, PC自动被设置为stvec中的地址, 即跳转到__alltrap. __alltrap首先切换到内核栈, 然后在内核栈保存所有必要的状态, 之后调用trap_handler根据异常原因进行处理.
   - ch2b_bad_instructions
     - 报错信息: [kernel] IllegalInstruction in application, kernel killed it.
     - 报错过程: 程序尝试执行 sret 指令, 这是一个 RISC-V 架构中的特权指令. sret 指"supervisor return", 通常用于内核处理完异常或系统调用后从S模式返回U模式, 指令执行时会根据sstatus寄存器的SPP位设置处理器的权限级别. 当用户程序尝试执行sret时, 处理器会检测到权限不足, 产生"Illegal Instruction Exception", 然后将控制权转移到操作系统的异常处理程序.
   - ch2b_bad_register
     - 报错信息: [kernel] IllegalInstruction in application, kernel killed it.
     - 报错过程: sstatus是risc-v中的特权控制状态寄存器, 包含关键的处理器状态信息, 只应在S模式下被访问. 当用户程序尝试执行csrr 读取 sstatus时, 处理器检测到权限不足, 产生非法指令异常, 控制权被转移到操作系统的异常处理程序.

2. 深入理解 trap.S 中两个函数 __alltraps 和 __restore 的作用，并回答如下问题:
- L40：刚进入 __restore 时，sp 代表了什么值。请指出 __restore 的两种使用情景。
  - sp代表内核栈指针. 指向内核栈上的一个TrapContext实例.
  - __restore可用于从异常/中断处理返回用户态
  - 在批处理操作系统中，__restore用于启动新应用程序，通过直接调用__restore函数并传入包含用户程序入口点和栈指针的TrapContext地址，实现从内核态到用户态的切换，开始执行新的应用程序。
  - 在分时多任务系统中, 每个任务的TaskContext.ra被初始化为__restore函数地址, 当对某个任务首次执行__switch时, ret指令会自动跳转到__restore, 通过__restore完成从内核态到用户态的切换开始执行新的应用.

- L43-L48：这几行汇编代码特殊处理了哪些寄存器？这些寄存器的的值对于进入用户态有何意义？请分别解释。
```
ld t0, 32*8(sp)
ld t1, 33*8(sp)
ld t2, 2*8(sp)
csrw sstatus, t0
csrw sepc, t1
csrw sscratch, t2
```
  - 这段代码负责从内核栈恢复三个CSR.
  - sstatus控制处理器的运行状态和权限级别.
    - SPP 控制sret返回后的特权模式
    - SIE 控制S模式下的中断是否启用
  - sepc存储异常返回地址
    - 决定sret指令执行后处理器跳转的目标地址
  - sscratch临时存储值
    - 此处用于临时存储用户栈指针, 在sret返回前将值传递给sp

- L50-L56：为何跳过了 x2 和 x4？
```
ld x1, 1*8(sp)
ld x3, 3*8(sp)
.set n, 5
.rept 27
   LOAD_GP %n
   .set n, n+1
.endr
```
  - x2 是 sp, 需要特殊处理.
  - x4 是 tp, 没有用到无需保存. 

- L60：该指令之后，sp 和 sscratch 中的值分别有什么意义？
```
csrrw sp, sscratch, sp
```
  - 执行后sp为用户栈指针, sscratch为内核栈指针

- __restore：中发生状态切换在哪一条指令？为何该指令执行之后会进入用户态？

  - 状态切换发生在sret指令, 执行这条指令硬件会根据sstatus中的SPP位设置处理器的特权级别, 并将PC设置为保存在sepc中的地址.

- L13：该指令之后，sp 和 sscratch 中的值分别有什么意义？
```
csrrw sp, sscratch, sp
```
  - 执行该指令之后, sp执行内核栈, sscratch指向用户栈 

- 从 U 态进入 S 态是哪一条指令发生的？
  - ecall指令
  - 用户执行ecall指令后, 处理器自动执行以下操作:
    - 将当前PC保存到sepc寄存器
    - 将当前特权级别保存到sstatus.SPP位
    - 清除sstatus.SIE
    - 设置异常原因代码为UserEnvCall
    - 将特权级别切换到S
    - 将PC设置为stvec寄存器指向的地址
  - 异常和中断也会触发从U态进入S态

## 荣誉准则
- 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：
  - 无
- 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：
  - [rCore-Tutorial-Book 第三版](https://rcore-os.cn/rCore-Tutorial-Book-v3/index.html)
  - [RISC-V 手册](http://riscvbook.com/chinese/RISC-V-Reader-Chinese-v2p1.pdf)
  - [测试用例](https://github.com/LearningOS/rCore-Tutorial-Test-2025S)
- 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。
- 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。