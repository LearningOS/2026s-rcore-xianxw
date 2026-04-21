### ch5
## 实现的功能:
本次实验实现了 sys_get_time、sys_mmap、sys_munmap、sys_spawn 和 sys_set_priority,并且完成了跨页情况下的时间信息写回，补充了内存映射/解除映射的页对齐、权限合法性及地址溢出检查，实现了用户地址空间的动态映射管理；同时支持按应用名创建新进程并加入就绪队列，并提供进程优先级设置功能，为调度机制提供支持.
## 问答:
1. 不会,因为如果用8 bit 无符号整数存储strid会发生溢出,当 p2 执行一个时间片后,8 bit 无符号整数会溢出成 4.
2. 因为pass=bigstride/priority,所以pass<=bihstride/2,所以每次最多增加bihstride/2,每次只会修改“最小的那个”,所以STRIDE_MAX – STRIDE_MIN <= BigStride / 2.
3. 
impl PartialOrd for Stride {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let diff = self.0.wrapping_sub(other.0);
        if diff < (1u64 << 63) {
            Some(Ordering::Less)
        } else {
            Some(Ordering::Greater)
        }
    }
}
## 荣誉准则
1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 copilot,codex 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：
fork,exec代码讲解
idle控制流本质
processor讲解
2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：
https://rcore-os.cn/rCore-Tutorial-Book-v3/index.html
https://zhuanlan.zhihu.com/p/351200492
3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。
4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。
