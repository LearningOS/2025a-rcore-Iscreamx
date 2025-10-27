//!Implementation of [`Processor`] and Intersection of control flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use crate::mm::{self, VirtAddr};
use alloc::sync::Arc;
use lazy_static::*;

/// Processor management structure
pub struct Processor {
    ///The task currently executing on the current processor
    current: Option<Arc<TaskControlBlock>>,

    ///The basic control flow of each core, helping to select and switch process
    idle_task_cx: TaskContext,
}

impl Processor {
    ///Create an empty Processor
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    ///Get mutable reference to `idle_task_cx`
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }

    ///Get current task in moving semanteme
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    ///Get current task in cloning semanteme
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }

    ///Map a memory region for the current task
    fn map_memory(&mut self, start: usize, len: usize, port: usize) -> isize {
        if start % crate::config::PAGE_SIZE != 0 { return -1; }
        if port & !0x7 != 0 || port & 0x7 == 0 { return -1; }

        let inner = self.current().unwrap();
        let mut task = inner.inner_exclusive_access();
        let start_va = VirtAddr::from(start);
        let end_va = VirtAddr::from(start + len);

        if task.memory_set.is_overlapped(start, start + len) {
            return -1;
        }

        let mut perm = mm::MapPermission::U;
        if port & 0x1 != 0 { perm |= mm::MapPermission::R; }
        if port & 0x2 != 0 { perm |= mm::MapPermission::W; }
        if port & 0x4 != 0 { perm |= mm::MapPermission::X; }

        println!("[DEBUG] Mapping memory: start={:#x}, len={}, perm={:?}", start, len, perm);
        task.memory_set.insert_framed_area(start_va, end_va, perm);
        0
    }

    ///Unmap a memory region for the current task
    fn unmap_memory(&mut self, start: usize, len: usize) -> isize {
        if start % crate::config::PAGE_SIZE != 0 { return -1; }

        let inner = self.current().unwrap();
        let mut task = inner.inner_exclusive_access();
        let start_va = VirtAddr::from(start);
        let end_va = VirtAddr::from(start + len);
        let start_vpn = start_va.floor();
        let end_vpn = end_va.ceil();

        task.memory_set.remove_area(start_vpn, end_vpn)
    }
}

lazy_static! {
    /// Creation of initial PROCESSOR
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

///The main part of process execution and scheduling
///Loop `fetch_task` to get the process that needs to run, and switch the process through `__switch`
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            // release coming task_inner manually
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            warn!("no tasks available in run_tasks");
        }
    }
}

/// Get current task through take, leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// Get a copy of the current task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// Get the current user token(addr of page table)
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.get_user_token()
}

///Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

///Return to idle control flow for new scheduling
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}

///Map a memory region for the current task
pub fn map_memory(start: usize, len: usize, port: usize) -> isize {
    PROCESSOR.exclusive_access().map_memory(start, len, port)
}

///Unmap a memory region for the current task
pub fn unmap_memory(start: usize, len: usize) -> isize {
    PROCESSOR.exclusive_access().unmap_memory(start, len)
}
