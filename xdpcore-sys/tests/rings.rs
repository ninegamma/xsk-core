use std::ptr;
use xdpcore_sys::*;

const SIZE: u32 = 4;

// Kernel-shared memory is held through raw pointers for the complete test;
// no Rust reference aliases the locations accessed by upstream C helpers.
struct RingMemory {
    indices: *mut [u32; 3],
    descriptors: *mut [xdp_desc],
}

impl RingMemory {
    fn new(position: u32) -> Self {
        Self {
            indices: Box::into_raw(Box::new([position, position, 0])),
            descriptors: Box::into_raw(
                vec![
                    xdp_desc {
                        addr: 0,
                        len: 0,
                        options: 0
                    };
                    SIZE as usize
                ]
                .into_boxed_slice(),
            ),
        }
    }

    fn producer(&self) -> *mut u32 {
        self.indices.cast()
    }

    fn consumer(&self) -> *mut u32 {
        // SAFETY: index 1 is inside the three-element indices allocation.
        unsafe { self.indices.cast::<u32>().add(1) }
    }

    fn flags(&self) -> *mut u32 {
        // SAFETY: index 2 is inside the three-element indices allocation.
        unsafe { self.indices.cast::<u32>().add(2) }
    }

    fn producer_ring(&self, position: u32) -> xsk_ring_prod {
        xsk_ring_prod {
            cached_prod: position,
            cached_cons: position.wrapping_add(SIZE),
            mask: SIZE - 1,
            size: SIZE,
            producer: self.producer(),
            consumer: self.consumer(),
            ring: self.descriptors.cast(),
            flags: self.flags(),
        }
    }

    fn consumer_ring(&self, position: u32) -> xsk_ring_cons {
        xsk_ring_cons {
            cached_prod: position,
            cached_cons: position,
            mask: SIZE - 1,
            size: SIZE,
            producer: self.producer(),
            consumer: self.consumer(),
            ring: self.descriptors.cast(),
            flags: self.flags(),
        }
    }
}

impl Drop for RingMemory {
    fn drop(&mut self) {
        // SAFETY: both allocations came from Box::into_raw above. All rings
        // borrowing them have left scope before this fixture is dropped.
        unsafe {
            drop(Box::from_raw(self.indices));
            drop(Box::from_raw(self.descriptors));
        }
    }
}

#[test]
fn producer_helpers_publish_descriptors_across_index_wraparound() {
    let initial = u32::MAX - 1;
    let memory = RingMemory::new(initial);
    let mut ring = memory.producer_ring(initial);
    let mut index = 0;

    // SAFETY: the initialized ring points to aligned, live fixture storage.
    // This thread is its only accessor; all operations stay within SIZE slots.
    unsafe {
        assert_eq!(xsk_ring_prod__reserve(&mut ring, SIZE, &mut index), SIZE);
        assert_eq!(index, initial);
        for offset in 0..SIZE {
            let slot = index.wrapping_add(offset);
            let descriptor = xsk_ring_prod__tx_desc(&mut ring, slot);
            assert_eq!(
                descriptor,
                memory
                    .descriptors
                    .cast::<xdp_desc>()
                    .add((slot & 3) as usize)
            );
            ptr::write(
                descriptor,
                xdp_desc {
                    addr: u64::from(offset) * 4096,
                    len: 64 + offset,
                    options: 0,
                },
            );
        }
        assert_eq!(xsk_ring_prod__reserve(&mut ring, 1, &mut index), 0);
        assert_eq!(*memory.producer(), initial, "reserve must not publish");
        xsk_ring_prod__submit(&mut ring, SIZE);
        assert_eq!(*memory.producer(), initial.wrapping_add(SIZE));

        // Simulate the kernel returning three slots, then force cache refresh.
        *memory.consumer() = initial.wrapping_add(3);
        assert_eq!(xsk_prod_nb_free(&mut ring, SIZE), 3);
        assert_eq!(xsk_ring_prod__reserve(&mut ring, 3, &mut index), 3);
        assert_eq!(index, initial.wrapping_add(SIZE));

        assert_eq!(xsk_ring_prod__needs_wakeup(&ring), 0);
        *memory.flags() = XDP_RING_NEED_WAKEUP;
        assert_ne!(xsk_ring_prod__needs_wakeup(&ring), 0);
    }
}

#[test]
fn consumer_helpers_cancel_and_release_across_index_wraparound() {
    let initial = u32::MAX - 1;
    let memory = RingMemory::new(initial);
    let mut ring = memory.consumer_ring(initial);
    let mut index = 0;

    // SAFETY: the ring's backing indices and descriptors remain initialized,
    // aligned, live, and exclusively accessed by this test thread.
    unsafe {
        *memory.producer() = initial.wrapping_add(3);
        assert_eq!(xsk_ring_cons__peek(&mut ring, SIZE, &mut index), 3);
        assert_eq!(index, initial);
        assert_eq!(
            xsk_ring_cons__rx_desc(&ring, index),
            memory.descriptors.cast::<xdp_desc>().add(2)
        );
        xsk_ring_cons__cancel(&mut ring, 1);
        xsk_ring_cons__release(&mut ring, 2);
        assert_eq!(*memory.consumer(), initial.wrapping_add(2));
        assert_eq!(xsk_ring_cons__peek(&mut ring, SIZE, &mut index), 1);
        assert_eq!(index, initial.wrapping_add(2));
        xsk_ring_cons__release(&mut ring, 1);
        assert_eq!(xsk_cons_nb_avail(&mut ring, SIZE), 0);

        *memory.producer() = initial.wrapping_add(6);
        assert_eq!(xsk_ring_cons__peek(&mut ring, SIZE, &mut index), 3);
        assert_eq!(index, initial.wrapping_add(3));
        xsk_ring_cons__release(&mut ring, 3);
        assert_eq!(*memory.consumer(), initial.wrapping_add(6));
    }
}

#[test]
fn address_helpers_apply_the_ring_mask() {
    let memory = RingMemory::new(0);
    let mut producer = memory.producer_ring(0);
    let consumer = memory.consumer_ring(0);
    // SAFETY: both helpers only compute in-bounds pointers into the aligned,
    // live descriptor allocation, which also has room for SIZE u64 addresses.
    unsafe {
        let address = xsk_ring_prod__fill_addr(&mut producer, SIZE + 2);
        assert_eq!(address, memory.descriptors.cast::<u64>().add(2));
        *address = 8192;
        assert_eq!(*xsk_ring_cons__comp_addr(&consumer, SIZE * 2 + 2), 8192);
    }
}
