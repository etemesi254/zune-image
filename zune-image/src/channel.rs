//! This module encapsulates a single image channel instance
//!
//! The channel is analogous to C/C++ `void *` but comes with some safety
//! measures imposed by it's usage and the Rust interface in general
//!
//! It exposes few methods to have a small unsafe footprint
//! since it relies on unsafe primitives to transmute between types
//!
//! The channel is able to store multiple bit depths but has no internal
//! representation of what upper type it represents,i.e it doesn't distinguish between u8 and u16
//! as separate bit depths.
//! All are seen as u8 to it.
//!
use std::alloc::{alloc, dealloc, realloc, Layout};
use std::fmt::{Debug, Formatter};
use std::mem::size_of;

/// Minimum alignment for all types allocated in the channel
///
/// This makes it possible to reinterpret the channel data safely
/// as whatever type we so wish without worry that it would be wrongly
/// misaligned especially on platforms where reading unaligned data is UB
pub const MIN_ALIGNMENT: usize = 16;

/// Encapsulates errors that can occur
/// when manipulating channels
#[derive(Copy, Clone)]
pub enum ChannelErrors
{
    /// rarely, since all allocations are aligned to 16, but just in case
    UnalignedPointer(usize, usize),
    /// The length of the type does not evenly divide the channel length
    /// Indicating that wea re trying to align the channel data to something
    /// that does not evenly divide it
    UnevenLength(usize, usize)
}

impl Debug for ChannelErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            ChannelErrors::UnalignedPointer(expected, found) =>
            {
                writeln!(f, "Channel pointer {expected} is not aligned to {found}")
            }
            ChannelErrors::UnevenLength(length, size_of_1) =>
            {
                writeln!(
                    f,
                    "Size of {size_of_1} cannot evenly divide length {length}"
                )
            }
        }
    }
}

/// Encapsulates an image channel
///
/// A channel can be thought as a bag of bits, and has the same semantics
/// as a `Vec<T>`, but you can reinterpret the bits in different kind of ways
///
/// Most of the operations in the channel work by calling
/// `reinterpret` methods, both as reference and as mutable.
pub struct Channel
{
    ptr:      *mut u8,
    length:   usize,
    capacity: usize
}

unsafe impl Send for Channel {}

unsafe impl Sync for Channel {}

impl Clone for Channel
{
    fn clone(&self) -> Self
    {
        let mut new_channel = Channel::new_with_capacity(self.capacity());
        // copy items by calling extend
        // unwrap here is safe as the conditions for None.
        // do not apply to u8 types
        new_channel.extend(self.reinterpret_as::<u8>().unwrap());
        new_channel
    }
}

impl Debug for Channel
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        unsafe {
            let slice = std::slice::from_raw_parts(self.ptr, self.length);

            writeln!(f, "{slice:?}")
        }
    }
}

impl Channel
{
    /// Return the number of elements the
    /// channel can store without reallocating
    ///
    /// It returns the number of raw bytes, not respecting
    /// type stored
    pub const fn capacity(&self) -> usize
    {
        self.capacity
    }
    /// Return the length of the underlying array
    ///
    /// This returns the raw length, i.e the length
    /// if the array was viewed as a series of one byte representations
    ///
    ///
    /// Meaning if the pointer stored 10 u32's, the length would be 40
    /// since that is `10*4`, the 4 is because `core::mem::size_of::<u32>()` == 4.
    ///
    /// # Example
    ///
    pub const fn len(&self) -> usize
    {
        self.length
    }

    /// Return true whether this length is zero
    pub const fn is_empty(&self) -> bool
    {
        self.length == 0
    }

    /// Allocates some bytes using the system allocator.
    /// but align it to MIN_ALIGNMENT
    unsafe fn alloc(size: usize) -> *mut u8
    {
        let layout = Layout::from_size_align(size, MIN_ALIGNMENT).unwrap();
        alloc(layout)
    }
    /// Reallocate the pointer in place increasing
    /// it's capacity
    unsafe fn realloc(&mut self, new_size: usize)
    {
        let layout = Layout::from_size_align(new_size, MIN_ALIGNMENT).unwrap();

        // make pointer to be
        self.ptr = realloc(self.ptr, layout, new_size);
        // set capacity to be new size
        self.capacity = new_size;
    }
    /// Deallocate storage allocated
    unsafe fn dealloc(&mut self)
    {
        let layout = Layout::from_size_align(self.capacity, MIN_ALIGNMENT).unwrap();

        dealloc(self.ptr, layout);
    }

    /// Create a new channel
    pub fn new() -> Channel
    {
        Self::new_with_capacity(10)
    }
    /// Create a new channel with the specified lenght and capacity
    pub fn new_with_length(length: usize) -> Channel
    {
        let mut channel = Channel::new_with_capacity(length);
        channel.length = length;

        channel
    }
    /// Create a new channel with the specified capacity
    /// and zero length
    pub fn new_with_capacity(capacity: usize) -> Channel
    {
        unsafe {
            let ptr = Self::alloc(capacity);

            Self {
                ptr,
                length: 0,
                capacity
            }
        }
    }

    /// Creates  a new channel capable of storing T*length items and
    /// fill it with elm symbols
    pub fn from_elm<T: Copy>(length: usize, elm: T) -> Channel
    {
        // new currently zeroes memory
        let mut new_chan = Channel::new_with_length(length * size_of::<T>());

        new_chan.fill(elm).unwrap();

        new_chan
    }
    /// Return true if we can store `extra`
    /// items without resizing/reallocating
    fn has_capacity(&self, extra: usize) -> bool
    {
        self.length.saturating_add(extra) <= self.capacity
    }
    /// Extend this channel with items from data
    ///
    ///
    pub fn extend<T: Copy>(&mut self, data: &[T])
    {
        // get size of the generic type
        let data_size = core::mem::size_of::<T>();
        // get number of items we need to store
        let items = data.len().saturating_mul(data_size);
        // check if we need to realloc
        if !self.has_capacity(items)
        {
            // reallocate to handle enough of the length.
            // realloc will set the new capacity
            // but as callers we have to set the new length
            unsafe {
                self.realloc(self.capacity.saturating_add(items).saturating_add(10));
            }
        }
        // now we have enough space, extend
        unsafe {
            self.ptr.add(self.length).copy_from(
                data.as_ptr().cast::<u8>(),
                data.len().saturating_mul(data_size)
            );
        }
        // new length becomes old length + items added
        self.length += items;
    }

    /// Reinterpret the channel as being composed of the following type
    ///
    /// The length of the new slice is defined
    /// as size of T over the length of the stored items in the pointer
    pub fn reinterpret_as<T: Default>(&self) -> Option<&[T]>
    {
        // Get size of pointer
        let size = core::mem::size_of::<T>();

        let new_length = self.length / size;

        // check if the alignment is correct
        // plus we can evenly divide this
        if self.confirm_suspicions::<T>().is_err()
        {
            return None;
        }

        let new_ptr = self.ptr.cast::<T>();
        // Safety:
        // 1- Data is aligned correctly
        let new_slice = unsafe { std::slice::from_raw_parts(new_ptr, new_length) };

        Some(new_slice)
    }

    pub fn reinterpret_as_mut<T: Default>(&mut self) -> Option<&mut [T]>
    {
        // Get size of pointer
        let size = core::mem::size_of::<T>();
        // check if the alignment is correct + size evenly divides
        if self.confirm_suspicions::<T>().is_err()
        {
            return None;
        }
        let new_length = self.length / size;

        let new_ptr = self.ptr.cast::<T>();
        let new_slice = unsafe { std::slice::from_raw_parts_mut(new_ptr, new_length) };

        Some(new_slice)
    }

    /// Push a single element to the channel.
    ///
    /// unlike `Vec::push()`, you can push arbitrary types of items
    /// and the data type will be reinterpreted as it's byte representation
    /// and raw byte representations will be copied to the channel.
    ///
    /// # Example
    /// ```
    /// use core::mem::size_of;
    /// use zune_image::channel::Channel;
    /// let mut channel = Channel::new();
    /// // push a u32 first
    /// channel.push::<u32>(123);
    /// // then a u64
    /// channel.push::<u64>(456);
    /// // then u8
    /// channel.push::<u8>(123);
    ///
    /// let len = size_of::<u8>()+size_of::<u32>()+size_of::<u64>();
    /// // assert that length matches
    /// assert_eq!(channel.len(),len);
    /// ```
    pub fn push<T: Copy>(&mut self, elm: T)
    {
        let size = core::mem::size_of::<T>(); // compile time

        if !self.has_capacity(size)
        {
            unsafe {
                // extend
                self.realloc(self.capacity.saturating_add((size * 3) / 2));
            }
        }
        unsafe {
            // Store elm in a 1 element array in order to cast it's
            // pointer to u8 so that we can copy it
            let arr = [elm];

            self.ptr
                .add(self.length)
                .copy_from(arr.as_ptr().cast(), size);
        }
        // increment length by number of bytes it takes to represent this type.
        self.length += size;
    }
    /// Fill this channel with the element `T`
    pub fn fill<T: Copy>(&mut self, element: T) -> Result<(), ChannelErrors>
    {
        let size = core::mem::size_of::<T>();

        // Check safety under for loop
        self.confirm_suspicions::<T>()?;

        // Data is correctly aligned,
        // T evenly divides self.channel
        let new_cast = self.ptr.cast::<T>();

        let new_length = self.length / size;

        for offset in 0..new_length
        {
            // Finally write the whole item
            // Safety:
            //  - We know that the length is
            //    in bounds in ptr and we only write enough data to define
            //    length.
            //  - We also check that T will fill length evenly by using
            //    confirm_suspicions()
            //  - We know we are writing to aligned memory, confirmed by
            //    confirm_suspicions() (it confirms it's aligned)
            //
            unsafe {
                new_cast.add(offset).write(element);
            }
        }

        Ok(())
    }
    /// Confirm that data is aligned and
    ///
    /// the type T can evenly divide length
    fn confirm_suspicions<T>(&self) -> Result<(), ChannelErrors>
    {
        // confirm the data is aligned for T
        if !is_aligned::<T>(self.ptr)
        {
            return Err(ChannelErrors::UnalignedPointer(
                self.ptr as usize,
                size_of::<T>()
            ));
        }

        // confirm we can evenly divide length
        if self.length % size_of::<T>() != 0
        {
            return Err(ChannelErrors::UnevenLength(self.length, size_of::<T>()));
        }

        Ok(())
    }
}

impl Drop for Channel
{
    fn drop(&mut self)
    {
        // dealloc storage
        unsafe {
            self.dealloc();
        }
    }
}

/// Check if a pointer is aligned.
fn is_aligned<T>(ptr: *const u8) -> bool
{
    let size = core::mem::size_of::<T>();

    (ptr as usize) & ((size) - 1) == 0
}
