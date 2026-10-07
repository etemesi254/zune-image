//! Per-CTU-row ownership of picture-sized state.
//!
//! With wavefront parallel processing (WPP) each CTU row of a slice is
//! decoded by its own thread. Every row only *writes* state belonging to its
//! own rows, and only *reads* the state of the row directly above it. So each
//! row decoder gets exclusive (`&mut`) access to its own band of rows, split
//! off the picture-sized buffer with `chunks_mut`/`split_at_mut`, plus a
//! private copy of the one row above it (`above`), which the WPP hand-off
//! fills in as the row above finishes each CTU.
//!
//! The band is addressed with *picture* indices, so code written against the
//! whole picture works unchanged: reads just above the band come from
//! `above`, anything else outside the band is a bug and panics.

use alloc::vec::Vec;
use core::ops::{Index, IndexMut};

pub struct Band<'a, T> {
    rows:        &'a mut [T],
    /// picture index of `band[0]`
    start:       usize,
    /// copy of the row directly above the band (empty for the top band)
    pub above:   Vec<T>,
    above_start: usize
}

impl<'a, T: Copy + Default> Band<'a, T> {
    /// A band whose first element has picture index `start`, with room for
    /// `above_len` elements of the row above it.
    pub fn new(band: &'a mut [T], start: usize, above_len: usize) -> Self {
        Self {
            rows: band,
            start,
            above: vec![T::default(); above_len],
            above_start: start - above_len
        }
    }

    /// The band's own elements (picture indices `start..start + len`)
    #[cfg(feature = "std")]
    pub fn own(&self) -> &[T] {
        self.rows
    }

    #[inline(always)]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        index
            .checked_sub(self.start)
            .and_then(|i| self.rows.get_mut(i))
    }
}

impl<T> Index<usize> for Band<'_, T> {
    type Output = T;

    #[inline(always)]
    fn index(&self, index: usize) -> &T {
        if index >= self.start {
            &self.rows[index - self.start]
        } else {
            index
                .checked_sub(self.above_start)
                .and_then(|i| self.above.get(i))
                .expect("access outside this CTU row and the row above it")
        }
    }
}

impl<T> IndexMut<usize> for Band<'_, T> {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut T {
        let i = index
            .checked_sub(self.start)
            .expect("write outside this CTU row");
        &mut self.rows[i]
    }
}
