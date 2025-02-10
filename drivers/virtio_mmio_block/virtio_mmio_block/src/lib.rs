//! This crate should implement the block device driver according to the VirtIO specification.
//! The [virtio-blk](virtio_blk) crate provides the safety abstraction for the VirtIO registers and buffers.
//! So this crate should only implement the driver logic with safe Rust code.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
use alloc::boxed::Box;
use core::{
    fmt::{Debug, Formatter},
    ops::Range,
};

use basic::{
    io::SafeIORegion,
    println,
    sync::{Mutex, Once, OnceGet},
    AlienResult,
};
use interface::{define_unwind_for_BlkDeviceDomain, Basic, BlkDeviceDomain, DeviceBase};
use shared_heap::{DBox, DVec};
use virtio_drivers::{device::block::VirtIOBlk, transport::mmio::MmioTransport};
use virtio_mmio_common::{HalImpl, SafeIORW};

pub struct BlkDomain {
    blk: Once<Mutex<VirtIOBlk<HalImpl, MmioTransport>>>,
}

impl BlkDomain {
    pub fn new() -> Self {
        Self { blk: Once::new() }
    }
}

impl Debug for BlkDomain {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str("BlkDomain")
    }
}

impl Basic for BlkDomain {
    fn domain_id(&self) -> u64 {
        shared_heap::domain_id()
    }
}

impl DeviceBase for BlkDomain {
    fn handle_irq(&self) -> AlienResult<()> {
        todo!()
    }
}

impl BlkDeviceDomain for BlkDomain {
    fn init(&self, device_info: &Range<usize>) -> AlienResult<()> {
        let region = device_info;
        println!("virtio_blk_addr: {:#x}-{:#x}", region.start, region.end);
        let io_region = SafeIORW(SafeIORegion::from(device_info.clone()));
        let transport = MmioTransport::new(Box::new(io_region)).unwrap();
        let blk = VirtIOBlk::<HalImpl, MmioTransport>::new(transport)
            .expect("failed to create virtio_blk");
        // blk.enable_receive_interrupt()?;
        self.blk.call_once(|| Mutex::new(blk));
        Ok(())
    }
    fn read_block(&self, block: u32, mut data: DVec<u8>) -> AlienResult<DVec<u8>> {
        #[cfg(feature = "crash")]
        if basic::blk_crash_trick() {
            panic!("blk crash trick");
        }
        self.blk
            .get_must()
            .lock()
            .read_blocks(block as _, data.as_mut_slice())
            .expect("failed to read block");
        Ok(data)
    }
    fn write_block(&self, block: u32, data: &DVec<u8>) -> AlienResult<usize> {
        self.blk
            .get_must()
            .lock()
            .write_blocks(block as _, data.as_slice())
            .expect("failed to write block");
        Ok(data.len())
    }
    fn get_capacity(&self) -> AlienResult<u64> {
        let size = self
            .blk
            .get_must()
            .lock()
            .capacity()
            .expect("failed to get capacity");
        Ok(size)
    }
    fn flush(&self) -> AlienResult<()> {
        self.blk.get_must().lock().flush().expect("failed to flush");
        Ok(())
    }
}

define_unwind_for_BlkDeviceDomain!(BlkDomain);

pub fn main() -> Box<dyn BlkDeviceDomain> {
    Box::new(UnwindWrap::new(BlkDomain::new()))
}
