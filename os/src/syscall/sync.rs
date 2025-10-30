use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::vec;
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
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
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
    let tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    {
        let mut inner = process.inner_exclusive_access();
        let mutex_count = inner.mutex_list.len();
        while inner.mutex_allocation.len() <= tid {
            inner.mutex_allocation.push(vec![0; mutex_count]);
        }
        while inner.mutex_allocation[tid].len() <= mutex_id {
            inner.mutex_allocation[tid].push(0);
        }
    }
    // 死锁检测
    if check_mutex_deadlock(mutex_id) {
        return -0xdead;
    }
    {
        let inner = process.inner_exclusive_access();
        let mutex = Arc::clone(inner.mutex_list[mutex_id].as_ref().unwrap());
        drop(inner);
        mutex.lock();
    }
    {
        let mut inner = process.inner_exclusive_access();
        inner.mutex_allocation[tid][mutex_id] += 1;
    }
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
    let tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    {
        let mut inner = process.inner_exclusive_access();
        if inner.mutex_allocation.len() > tid && inner.mutex_allocation[tid].len() > mutex_id {
            if inner.mutex_allocation[tid][mutex_id] > 0 {
                inner.mutex_allocation[tid][mutex_id] -= 1;
            }
        }
    }
    {
        let inner = process.inner_exclusive_access();
        let mutex = Arc::clone(inner.mutex_list[mutex_id].as_ref().unwrap());
        drop(inner);
        mutex.unlock();
    }
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
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
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
    let mut inner = process.inner_exclusive_access();
    let tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    if inner.semaphore_allocation.len() > tid && inner.semaphore_allocation[tid].len() > sem_id {
        if inner.semaphore_allocation[tid][sem_id] > 0 {
            inner.semaphore_allocation[tid][sem_id] -= 1;
        }
    }
    let sem = Arc::clone(inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(inner);
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
    let mut inner = process.inner_exclusive_access();
    let tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    let sem_count = inner.semaphore_list.len();
    while inner.semaphore_allocation.len() <= tid {
        inner.semaphore_allocation.push(vec![0; sem_count]);
    }
    while inner.semaphore_allocation[tid].len() <= sem_id {
        inner.semaphore_allocation[tid].push(0);
    }
    drop(inner);
    if check_semaphore_deadlock(sem_id) {
        println!("[DEBUG] tid={} sem_id={} deadlock detected, return -0xdead",
            tid, sem_id);
        return -0xdead;
    }
    let process = current_process();
    let inner = process.inner_exclusive_access();
    let sem = Arc::clone(inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(inner);
    sem.down();
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    inner.semaphore_allocation[tid][sem_id] += 1;
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
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect NOT IMPLEMENTED");
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if enabled == 1 {
        inner.deadlock_detect_enabled = true;
        0
    } else if enabled == 0 {
        inner.deadlock_detect_enabled = false;
        0
    } else {
        -1
    }
}
fn check_semaphore_deadlock(sem_id: usize) -> bool {
    let process = current_process();
    let inner = process.inner_exclusive_access();
    if !inner.deadlock_detect_enabled {
        return false;
    }
    let tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    let n = inner.thread_count();
    let m = inner.semaphore_list.len();

    let mut available = Vec::with_capacity(m);
    for sem_opt in &inner.semaphore_list {
        if let Some(sem) = sem_opt {
            let count = sem.inner.exclusive_access().count;
            available.push(if count >= 0 { count as usize } else { 0 });
        } else {
            available.push(0);
        }
    }
    println!("[DEBUG] available: {:?}", available);

    let mut allocation = vec![vec![0; m]; n];
    for i in 0..n {
        if i < inner.semaphore_allocation.len() {
            for j in 0..m.min(inner.semaphore_allocation[i].len()) {
                allocation[i][j] = inner.semaphore_allocation[i][j];
            }
        }
    }
    println!("[DEBUG] allocation: {:?}", allocation);

    // 如果请求后持有数量超过总资源，直接死锁
    if allocation[tid][sem_id] + 1 > available[sem_id] + allocation[tid][sem_id] {
        println!("[DEBUG] tid={} sem_id={} request exceeds total resource, deadlock", tid, sem_id);
        return true;
    }

    available[sem_id] -= 1;
    allocation[tid][sem_id] += 1;
    println!("[DEBUG] after request: available={:?}, allocation={:?}", available, allocation);

    let mut work = available.clone();
    let mut finish = vec![false; n];
    let mut round = 0;
    loop {
        let mut found = false;
        for i in 0..n {
            if finish[i] {
                continue;
            }
            let mut can_finish = true;
            for j in 0..m {
                // 每个线程最多只会再请求 1 个资源
                if work[j] < 1 {
                    can_finish = false;
                    break;
                }
            }
            if can_finish {
                println!("[DEBUG] round {}: thread {} can finish, work before: {:?}", round, i, work);
                for j in 0..m {
                    work[j] += allocation[i][j];
                }
                finish[i] = true;
                found = true;
                println!("[DEBUG] round {}: thread {} finished, work after: {:?}", round, i, work);
            }
        }
        round += 1;
        if !found {
            break;
        }
    }
    println!("[DEBUG] finish: {:?}", finish);
    let deadlock = !finish.iter().all(|&x| x);
    println!("[DEBUG] deadlock_detect result: {}", deadlock);
    deadlock
}
/// check mutex deadlock using Banker's Algorithm
fn check_mutex_deadlock(mutex_id: usize) -> bool {
    let process = current_process();
    let inner = process.inner_exclusive_access();
    if !inner.deadlock_detect_enabled {
        return false;
    }
    let tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    let n = inner.thread_count();
    let m = inner.mutex_list.len();

    // available: 1 表示未锁，0 表示已锁
    let mut available = vec![1; m];
    for i in 0..m {
        for t in 0..n {
            if t < inner.mutex_allocation.len()
                && inner.mutex_allocation[t].len() > i
                && inner.mutex_allocation[t][i] > 0
            {
                available[i] = 0;
                break;
            }
        }
    }
    println!("[DEBUG] mutex available: {:?}", available);

    let mut allocation = vec![vec![0; m]; n];
    for t in 0..n {
        if t < inner.mutex_allocation.len() {
            for i in 0..m.min(inner.mutex_allocation[t].len()) {
                allocation[t][i] = inner.mutex_allocation[t][i];
            }
        }
    }
    println!("[DEBUG] mutex allocation: {:?}", allocation);

    // 银行家算法
    if available[mutex_id] == 0 {
        println!("[DEBUG] tid={} mutex_id={} request blocked: available=0", tid, mutex_id);
        return true;
    }
    available[mutex_id] -= 1;
    allocation[tid][mutex_id] += 1;
    println!("[DEBUG] after request: available={:?}, allocation={:?}", available, allocation);

    let mut work = available.clone();
    let mut finish = vec![false; n];
    loop {
        let mut found = false;
        for i in 0..n {
            if finish[i] {
                continue;
            }
            let mut can_finish = true;
            for j in 0..m {
                if allocation[i][j] > work[j] {
                    can_finish = false;
                    break;
                }
            }
            if can_finish {
                for j in 0..m {
                    work[j] += allocation[i][j];
                }
                finish[i] = true;
                found = true;
            }
        }
        if !found {
            break;
        }
    }
    println!("[DEBUG] finish: {:?}", finish);
    if n == 1 && allocation[0][mutex_id] == 1 {
        println!("[DEBUG] single thread, not deadlock");
        return false;
    }
    let deadlock = !finish.iter().all(|&x| x);
    println!("[DEBUG] deadlock_detect result: {}", deadlock);
    deadlock
}
