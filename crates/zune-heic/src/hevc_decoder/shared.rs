//! A fixed-size buffer that several WPP row decoders can access at once.
//!
//! With wavefront parallel processing (WPP) each CTU row of a slice is decoded
//! by its own thread. Rows only ever *write* state belonging to their own CTU
//! row, and only *read* state of the row above once that row has signalled
//! (through an atomic progress counter with Release/Acquire ordering) that the
//! CTUs being read are complete. So no element is ever accessed by two
//! threads at the same time, but different threads do touch different
//! elements of the same buffer concurrently, which a `&mut [T]` cannot
//! express.
//!
//! `SharedBuf` stores its elements in `UnsafeCell`s. Each thread works on its
//! own handle (`&mut self` methods hand out element references); additional
//! handles to the same storage can only be created with the `unsafe`
//! [`SharedBuf::shared_view`], whose caller takes over the obligation above.

use std::cell::UnsafeCell;
use std::ops::{Index, IndexMut};
use std::sync::Arc;

pub struct SharedBuf<T> {
    data: Arc<[UnsafeCell<T>]>
}

// SAFETY: access to the elements is coordinated by the WPP progress counters
// (see module docs and `shared_view`).
unsafe impl<T: Send> Send for SharedBuf<T> {}
unsafe impl<T: Send + Sync> Sync for SharedBuf<T> {}

impl<T: Clone> SharedBuf<T> {
    pub fn new(len: usize, value: T) -> Self {
        Self::from_vec(vec![value; len])
    }

    /// Copy the current contents out (call once no other handle is in use).
    pub fn to_vec(&self) -> Vec<T> {
        (0..self.len()).map(|i| self[i].clone()).collect()
    }
}

impl<T> SharedBuf<T> {
    pub fn from_vec(values: Vec<T>) -> Self {
        Self {
            data: values.into_iter().map(UnsafeCell::new).collect()
        }
    }

    /// Create another handle to the same storage.
    ///
    /// # Safety
    /// While more than one handle is alive, the caller must guarantee that no
    /// element is written through one handle while it is read or written
    /// through another, unless those accesses are ordered by a
    /// happens-before relationship (e.g. an atomic Release store / Acquire
    /// load of WPP row progress).
    pub unsafe fn shared_view(&self) -> Self {
        Self {
            data: Arc::clone(&self.data)
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[inline(always)]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        // SAFETY: see `shared_view`.
        self.data.get(index).map(|c| unsafe { &mut *c.get() })
    }
}

impl<T> Index<usize> for SharedBuf<T> {
    type Output = T;

    #[inline(always)]
    fn index(&self, index: usize) -> &T {
        // SAFETY: see `shared_view`.
        unsafe { &*self.data[index].get() }
    }
}

impl<T> IndexMut<usize> for SharedBuf<T> {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut T {
        // SAFETY: see `shared_view`.
        unsafe { &mut *self.data[index].get() }
    }
}
