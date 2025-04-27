use super::UPSafeCell;
use alloc::vec;
use alloc::vec::Vec;

///
pub struct DeadLockDetectContext {
    ///
    pub enable: bool,
    ///
    pub mutex_inner: UPSafeCell<DeadLockDetectContextInner>,
    ///
    pub semaphore_inner: UPSafeCell<DeadLockDetectContextInner>,
}

///
pub struct DeadLockDetectContextInner {
    ///
    pub available: Vec<usize>,
    ///
    pub allocation: Vec<Vec<usize>>,
    ///
    pub need: Vec<Vec<usize>>,
}

impl DeadLockDetectContext {
    /// create an empty DeadLockDetectContext
    pub fn new() -> Self {
        Self {
            enable: false,
            mutex_inner: unsafe { UPSafeCell::new(DeadLockDetectContextInner::new()) },
            semaphore_inner: unsafe { UPSafeCell::new(DeadLockDetectContextInner::new()) },
        }
    }
}

impl DeadLockDetectContextInner {
    pub fn new() -> Self {
        let mut allocation = Vec::new();
        allocation.push(Vec::new());
        let mut need = Vec::new();
        need.push(Vec::new());
        Self {
            available: Vec::new(),
            allocation,
            need,
        }
    }

    pub fn detect_deadlock(&self) -> bool {
        // self.print_matrices();
        let n = self.allocation.len();
        let m = self.available.len();
        let mut work = self.available.clone();
        let mut finish = vec![false; n];

        for i in 0..n {
            for j in 0..m {
                work[j] -= self.allocation[i][j];
            }
        }

        loop {
            let mut found = false;
            for i in 0..n {
                if !finish[i] {
                    let mut can_finish = true;
                    for j in 0..m {
                        if self.need[i][j] > work[j] {
                            can_finish = false;
                            break;
                        }
                    }
                    if can_finish {
                        for j in 0..m {
                            work[j] += self.allocation[i][j];
                        }
                        finish[i] = true;
                        found = true;
                    }
                }
            }
            if !found {
                break;
            }
        }
        for x in finish {
            if x == false {
                return false;
            }
        }
        true
    }

    /*fn print_matrices(&self) {
        println!("===== Deadlock Detection Debug Info =====");
        println!("Available Resources: {:?}", self.available);
        println!("\nAllocation Matrix:");
        for (tid, alloc) in self.allocation.iter().enumerate() {
            println!("Thread {}: {:?}", tid, alloc);
        }
        println!("\nNeed Matrix:");
        for (tid, need) in self.need.iter().enumerate() {
            println!("Thread {}: {:?}", tid, need);
        }
        println!("=========================================");
    }*/
}
