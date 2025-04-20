//!Implementation of [`Processor`] and Intersection of control flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::mm::{can_alloc, MapPermission, VPNRange, VirtAddr};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
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

    ///
    pub fn map_memory(
        &self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
    ) -> isize {
        let page_count = end_va.ceil().0 - start_va.floor().0;
        let task = self.current.as_ref().unwrap();
        let mut inner = task.inner_exclusive_access();
        let memory_set = &mut inner.memory_set;
        let areas = &mut memory_set.areas;
        
        for area in areas {
            if area
                .vpn_range
                .overlaps(&VPNRange::new(start_va.floor(), end_va.ceil()))
            {
                return -1;
            }
        }
        
        if !can_alloc(page_count as usize) {
            return -1;
        }

        memory_set.insert_framed_area(start_va, end_va, permission);

        /*let vpn = start_va.floor();
        let pte = memory_set.page_table.find_pte(vpn);
        if let Some(pte) = pte {
            if pte.is_valid() {
                println!("[debug] vpn {:?} already mapped!", vpn);
            }
            println!("The test has passed?");
        }
        println!("fuck?");

        let last_area = memory_set.areas.len() - 1;
        let page_table = &mut memory_set.page_table;
        memory_set.areas[last_area].map(page_table);*/
        0
    }

    ///
    pub fn unmap_memory(&self, start_va: VirtAddr, end_va: VirtAddr) -> isize {
        let task = self.current.as_ref().unwrap();
        let mut inner = task.inner_exclusive_access();
        inner.memory_set.unmap_memory(start_va, end_va)
    }
}

lazy_static! {
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

///
pub fn map_memory(start_va: VirtAddr, end_va: VirtAddr, permission: MapPermission) -> isize {
    PROCESSOR
        .exclusive_access()
        .map_memory(start_va, end_va, permission)
}

///
pub fn unmap_memory(start_va: VirtAddr, end_va: VirtAddr) -> isize {
    PROCESSOR.exclusive_access().unmap_memory(start_va, end_va)
}

