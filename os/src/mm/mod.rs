//! Memory management implementation
//!
//! SV39 page-based virtual-memory architecture for RV64 systems, and
//! everything about memory management, like frame allocator, page table,
//! map area and memory set, is implemented here.
//!
//! Every task or process has a memory_set to control its virtual memory.

mod address;
mod frame_allocator;
mod heap_allocator;
mod memory_set;
pub mod page_table;

pub use address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use address::{StepByOne, VPNRange};
pub use frame_allocator::{frame_alloc, FrameTracker};
pub use memory_set::remap_test;
pub use memory_set::{kernel_stack_position, MapPermission, MemorySet, KERNEL_SPACE};
pub use page_table::{translated_byte_buffer, PageTableEntry};
pub use page_table::{PTEFlags, PageTable};
use crate::task::current_user_token;

/// initiate heap allocator, frame allocator and kernel space
pub fn init() {
    heap_allocator::init_heap();
    frame_allocator::init_frame_allocator();
    KERNEL_SPACE.exclusive_access().activate();
}

/// docs
pub fn transalte_ptr<T>( ptr: *const u8)-> *mut T {
    let page_table = PageTable::from_token(current_user_token());
    let v_add = ptr as usize;
    let v_add_ = VirtAddr::from(v_add);
    let v_pn = v_add_.floor();
    let p_te = page_table.translate(v_pn).unwrap().ppn();
    let offset: usize = v_add_.page_offset();
    let ppa: usize = p_te.into();
    let phys_ptr: *mut T = (ppa + offset) as *mut T;
    phys_ptr

}
/// judge
pub fn right_read( ptr: *const u8)-> bool {
    let page_table = PageTable::from_token(current_user_token());
    let v_add = ptr as usize;
    let v_add_ = VirtAddr::from(v_add);
    let v_pn = v_add_.floor();

    if let Some(pte) = page_table.translate(v_pn) {
        let flags = pte.flags();
        flags.contains(PTEFlags::V) && flags.contains(PTEFlags::R) && flags.contains(PTEFlags::U)
    } else {
        false
    }

}

/// judge
pub fn right_write(ptr: *const u8) -> bool {
    let page_table = PageTable::from_token(current_user_token());
    let v_add = ptr as usize;
    let v_add_ = VirtAddr::from(v_add);
    let v_pn = v_add_.floor();

    if let Some(pte) = page_table.translate(v_pn) {
        let flags = pte.flags();
        flags.contains(PTEFlags::V) && flags.contains(PTEFlags::W) && flags.contains(PTEFlags::U)
    } else {
        false
    }
}