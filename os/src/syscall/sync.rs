use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    // let mut mutex_inner = process_inner.deadlock_matrix.mutex_inner.exclusive_access();
    let tasks_num = process_inner.tasks.len();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        process_inner.deadlock_matrix.mutex_inner.exclusive_access().available[id] = 1;
        for i in 0..tasks_num {
            process_inner.deadlock_matrix.mutex_inner.exclusive_access().allocation[i][id] = 0;
        }
        for i in 0..tasks_num {
            process_inner.deadlock_matrix.mutex_inner.exclusive_access().need[i][id] = 0;
        }
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.deadlock_matrix.mutex_inner.exclusive_access().available.push(1);
        for i in 0..tasks_num {
            process_inner.deadlock_matrix.mutex_inner.exclusive_access().allocation[i].push(0);
        }
        for i in 0..tasks_num {
            process_inner.deadlock_matrix.mutex_inner.exclusive_access().need[i].push(0);
        }
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let tid = current_task().unwrap().get_pid();
    let mut m_inner = process_inner.deadlock_matrix.mutex_inner.exclusive_access();

    m_inner.need[tid][mutex_id] += 1;

    if process_inner.deadlock_matrix.enable == true && !m_inner.detect_deadlock() {
        m_inner.need[tid][mutex_id] -= 1; 
        println!("blocked!fuckyou");
        drop(m_inner);
        drop(process_inner);
        drop(process);
        return -0xDEAD;
    }
    m_inner.need[tid][mutex_id] -= 1; 

    drop(m_inner);
    drop(process_inner);
    drop(process);
    mutex.lock(mutex_id);

    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let mut mutex_inner = process_inner.deadlock_matrix.mutex_inner.exclusive_access();
    mutex_inner.available[mutex_id] = 1; // available
    let tid = current_task().unwrap().get_pid();
    mutex_inner.allocation[tid][mutex_id] = 0; // allocation
    drop(mutex_inner);
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let tasks_num = process_inner.tasks.len();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        process_inner.deadlock_matrix.semaphore_inner.exclusive_access().available[id] = res_count;
        for i in 0..tasks_num {
            process_inner.deadlock_matrix.semaphore_inner.exclusive_access().allocation[i][id] = 0;
        }
        for i in 0..tasks_num {
            process_inner.deadlock_matrix.semaphore_inner.exclusive_access().need[i][id] = 0;
        }
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.deadlock_matrix.semaphore_inner.exclusive_access().available.push(res_count);
        for i in 0..tasks_num {
            process_inner.deadlock_matrix.semaphore_inner.exclusive_access().allocation[i].push(0);
        }
        for i in 0..tasks_num {
            process_inner.deadlock_matrix.semaphore_inner.exclusive_access().need[i].push(0);
        }
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    let mut semaphore_inner = process_inner.deadlock_matrix.semaphore_inner.exclusive_access();
    semaphore_inner.available[sem_id] += 1; // available
    let tid = current_task().unwrap().get_pid();
    semaphore_inner.allocation[tid][sem_id] = 0; // allocation
    drop(semaphore_inner);
    drop(process_inner);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    // println!("sem_down part1!");
    let tid = current_task().unwrap().get_pid();
    let mut s_inner = process_inner.deadlock_matrix.semaphore_inner.exclusive_access();

    s_inner.need[tid][sem_id] += 1;

    if process_inner.deadlock_matrix.enable == true && !s_inner.detect_deadlock() {
        s_inner.need[tid][sem_id] -= 1; 
        // println!("blocked!fuckyou");
        return -0xDEAD;
    }
    s_inner.need[tid][sem_id] -= 1;
    // println!("sem_down part2!");

    drop(s_inner);
    drop(process_inner);
    // println!("sem_down part3!");
    sem.down(sem_id);
    // println!("sem_down part4!");
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex, mutex_id);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect");
    if enabled != 1 && enabled != 0 {
        println!("fuck you!");
        return -1;
    }
    let process = current_process();
    process.modify_deadlock_detect(enabled);
    1
}
