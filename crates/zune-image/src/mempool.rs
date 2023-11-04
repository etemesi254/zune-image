// use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
// use std::sync::Mutex;
// use crate::channel::Channel;
//
// struct MemPool {
//     pool: Vec<Mutex<Channel>>,
//     used_pool: Vec<AtomicBool>,
// }
//
// impl MemPool {
//     //
//     // pub fn new(size:usize){
//     //     MemPool{
//     //         pool: vec![],
//     //         used_pool: vec![],
//     //     }
//     // }
//
//     pub fn request_memory(&self, size: usize) {
//         // find the first pool which meets size
//         for (pos, pool_ptr) in self.used_pool.iter().enumerate() {
//             if let Ok(val) = pool_ptr.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed) {
//                 if false {
//
//                 }
//             }
//         }
//     }
// }
