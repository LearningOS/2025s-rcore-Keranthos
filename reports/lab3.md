荣誉准则 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

无

此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

无

我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

实现功能总结 完成新的系统调用sys_trace,依据传入的参数完成

1、读取/写入当前程序部分内存的效果

2、查询当前用户程序特定系统调用次数的功能

实现功能
完成sys_get_time/sys_mmap/sys_munmap的迁移与改写

实现sys_spawn系统调用

实现简单的stride调度方法，与其对应的sys_set_priority系统调用

问答作业
1、实际上不会是p1执行，因为p2在溢出后会重新从0开始计数

2、P.pass = BigStride / P.priority在P.priority > 2时，P1.pass - P2.pass <= BigStride / 2 - BigStirde / ∞即差值会小于等于BigStride / 2

3、

impl PartialOrd for Stride {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if (self.0 > other.0 && self.0 - other.0 <= BigStride / 2) || (self.0 < other.0 && other.0 - self.0 > BigStride / 2) return Some(Ordering::Greater);
        Some(Ordering::Less)
    }
}