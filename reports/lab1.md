# ch3
## 实现的功能:实现了ch3的系统调用次数的追踪功能,首先在进程的TaskControlBlock内新加入一个数组,使其最大范围大于系统调用的最大值,用于给每个单独的进程的系统调用计数,而后内核的调用入口syscall对指定索引进行计数,最后又完善了sys_trace的功能,分别实现了trace_request 为 0(返回id地址处的值),1(写入data到id地址处),2(查询并返回调用次数)的功能.
## 问答题
### 题目1:
[rustsbi] RustSBI version 0.3.0-alpha.2, adapting to RISC-V SBI v1.0.0
ch2b_bad_address显示触发PageFault后被内核终止
ch2b_bad_instructions显示触发IllegalInstruction后被内核拒绝
ch2b_bad_registe显示触发IllegalInstruction
### 题目2:
1. 刚进入__restore 时，sp 代表了内核栈上TrapContext的基地址(内核栈的栈顶)
__restore的两次使用场景:第一是首次进入某个用户任务,调用__restore完成从内核态到用户态的第一次切换,启动用户应用.第二是trap处理后返回原用户的上下文.
2. 处理了sstatus,sepc,sscratch三个寄存器
sstasus:其中的SPP的值决定了sret返回的特权级的类型,0则返回用户态,1则返回内核态.
sepc:保存返回用户态后要执行的下一条指令的地址.
sscratch:把用户态 sp 先放回 sscratch，是为了后面再用 csrrw sp,sscratch, sp  一次性交换回用户栈.
3. x2是sp栈指针,若提前切换,会导致后续指令在用户栈进行操作,导致错误,应当最后csrrw sp, sscratch, sp进行操作.
x4是tp线程指针,尚未实现器功能.
4. 执行之后sp代表用户栈的栈指针,而sscratch代表内核栈的栈指针
5. sret sret会根据SPP切换特权级
6. 执行之后sp代表内核栈栈指针,而sscratch代表用户栈的栈指针
7. ecall或者异常或者中断从而导致trap的发生,从而使硬件实现从U到S的切换
## 荣誉准则
1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与CHATGPT-5.3 CODEX就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：
TrapContext 初始化（init_app_cx）
构造“用户态初始现场”：用户入口 PC、用户栈指针、状态寄存器等
放到该任务的内核栈上，供 __restore 使用
TaskContext 初始化（goto_restore）
设置 ra = __restore，sp = 内核栈中那份 TrapContext 的位置
这样第一次被 __switch 切入后，ret 会跳到 __restore，再 sret 进用户态

2. 此外，我也参考了 risc-v 的以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：
https://www.taoyukai.com/docs/riscv/risc-v%E6%B1%87%E7%BC%96%E6%8C%87%E4%BB%A4-RV32I/risc-v%E6%B1%87%E7%BC%96%E6%8C%87%E4%BB%A4%E6%80%BB%E8%A7%88

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

## optional
难度适当




