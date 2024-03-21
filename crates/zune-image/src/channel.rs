/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

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
//! All are seen as u8 to it with the only difference being the latter is twice as big as the former.
//!
use std::alloc::{alloc_zeroed, dealloc, realloc, Layout};
use std::any::TypeId;
use std::fmt::{Debug, Formatter};
use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use zune_core::bit_depth::BitType;

/// Minimum alignment for all types allocated in the channel
///
/// This makes it possible to reinterpret the channel data safely
/// as whatever type we so wish without worry that it would be wrongly
/// misaligned especially on platforms where reading unaligned data is UB
///
///
/// 64 is chosen as it allows us to align to the highest
/// register type supported on machines I'm aware of
/// i.e its for AVX-512, and thenthis transitively means all other types
/// become aligned
pub const MIN_ALIGNMENT: usize = 64;

/// Encapsulates errors that can occur
/// when manipulating channels
#[derive(Copy, Clone)]
pub enum ChannelErrors {
    /// rarely, since all allocations are aligned to 16, but just in case
    UnalignedPointer(usize, usize),
    /// The length of the type does not evenly divide the channel length
    /// Indicating that wea re trying to align the channel data to something
    /// that does not evenly divide it
    UnevenLength(usize, usize),
    DifferentType(TypeId, TypeId)
}

impl Debug for ChannelErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelErrors::UnalignedPointer(expected, found) => {
                writeln!(f, "Channel pointer {expected} is not aligned to {found}")
            }
            ChannelErrors::UnevenLength(length, size_of_1) => {
                writeln!(
                    f,
                    "Size of {size_of_1} cannot evenly divide length {length}"
                )
            }
            ChannelErrors::DifferentType(expected, found) => {
                writeln!(f, "Different type id {:?} from expected {:?}. This indicates you are converting a channel
             to a type it wasn't instantiated with", expected, found)
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
#[derive(Eq)]
pub struct Channel {
    ptr:      *mut u8,
    length:   usize,
    capacity: usize,
    // type id for which the channel was created with
    type_id:  TypeId
}

// safety: The functions ae unsafe because the
// compiler cannot see that we own the data since self.ptr is a *mut 8
// which can be stored at a different location from the array,
// but since we own it and we do not expose it, this is safe
unsafe impl Send for Channel {}

unsafe impl Sync for Channel {}

impl Clone for Channel {
    fn clone(&self) -> Self {
        let mut new_channel = Channel::new_with_capacity_and_type(self.capacity(), self.type_id);
        // copy items by calling extend

        // Safety:
        //
        // no-slop-bytes: U8's cannot have slop bytes
        // ptr is valid since we own it, and memory is allocated by us.
        //
        unsafe {
            new_channel.extend_unchecked(self.reinterpret_as_unchecked::<u8>());
        }
        new_channel
    }
}

impl PartialEq for Channel {
    fn eq(&self, other: &Self) -> bool {
        // check if length matches
        if self.length != other.length {
            return false;
        }
        // check if type matches
        if self.type_id != other.type_id {
            return false;
        }
        unsafe {
            // interpret them as a bag of u8, and iterate

            // safety:
            // u8's can alias anything.

            // we confirmed that the items have the same length
            // and that they are of the same type
            let us = self.reinterpret_as_unchecked::<u8>();
            let them = other.reinterpret_as_unchecked::<u8>();

            for (a, b) in us.iter().zip(them) {
                if *a != *b {
                    return false;
                }
            }
        }
        // everything is good
        true
    }
}

impl Debug for Channel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // safety.
        // all types can alias u8,
        // length points to the length spanning the ptr
        let slice = unsafe { std::slice::from_raw_parts(self.ptr, self.length) };
        writeln!(f, "raw_bytes: {slice:?}")
    }
}

impl Channel {
    /// Return the number of elements the
    /// channel can store without reallocating
    ///
    /// It returns the number of raw bytes, not respecting
    /// type stored
    pub const fn capacity(&self) -> usize {
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
    pub const fn len(&self) -> usize {
        self.length
    }

    /// Return true whether this channel length is zero
    ///
    /// # Example
    /// ```
    /// use zune_image::channel::Channel;
    /// let c = Channel::new::<u8>();
    ///assert!(c.is_empty());
    /// ```
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Allocates some bytes using the system allocator.
    /// but align it to MIN_ALIGNMENT
    ///
    /// It is not unsafe to call this, it's just left as unsafe
    /// to remind one to be careful of what they are doing
    unsafe fn alloc(size: usize) -> *mut u8 {
        let layout = Layout::from_size_align(size, MIN_ALIGNMENT).unwrap();
        // Safety
        //  alloc zeroed == alloc + std::mem::zeroed()
        // and we are bound by the zeroed trait, hence we are sure that
        // for whatever type we are going to allocate for,
        // it can be represented with a bit-representation of zero.
        alloc_zeroed(layout)
    }
    /// Reallocate the pointer in place increasing
    /// it's capacity
    unsafe fn realloc(&mut self, new_size: usize) {
        let layout = Layout::from_size_align(new_size, MIN_ALIGNMENT).unwrap();

        self.ptr = realloc(self.ptr, layout, new_size);
        // set capacity to be new size
        self.capacity = new_size;
    }
    /// Deallocate storage allocated for this channel
    unsafe fn dealloc(&mut self) {
        let layout = Layout::from_size_align(self.capacity, MIN_ALIGNMENT).unwrap();

        // safety
        // - The same layout alignment we used for alloc is the same we are using for
        //  dealloc
        dealloc(self.ptr, layout);
    }

    /// Create a new channel
    ///
    ///
    /// This stores a single plane for an image
    pub fn new<T: 'static + Zeroable>() -> Channel {
        Self::new_with_capacity::<T>(10)
    }
    /// Create a new channel with the specified length and capacity
    ///
    /// The array is initialized to zero
    ///
    /// # Arguments
    ///  - length: The length of the new channel
    pub fn new_with_length<T: 'static + Zeroable>(length: usize) -> Channel {
        let mut channel = Channel::new_with_capacity::<T>(length);
        channel.length = length;

        channel
    }
    /// Create a new channel with the specified length and capacity
    ///
    /// and type
    ///
    /// # Arguments
    ///  - length: The new lenghth of the array
    ///  - type_id: The type id of the type this is supposed to store
    ///
    pub fn new_with_length_and_type(length: usize, type_id: TypeId) -> Channel {
        let mut channel = Channel::new_with_capacity_and_type(length, type_id);
        channel.length = length;

        channel
    }

    /// Create a new channel that can store items
    /// of a certain bit type
    ///
    /// # Arguments
    ///
    /// * `length`: The length of the new channel
    /// * `depth`:
    ///
    /// returns: Channel
    ///
    /// # Examples
    ///
    /// ```
    /// use zune_core::bit_depth::BitType;
    /// use zune_image::channel::Channel;
    /// let channel = Channel::new_with_bit_type(0,BitType::U8);
    /// ```
    pub fn new_with_bit_type(length: usize, depth: BitType) -> Channel {
        let t_r = match depth {
            BitType::U8 => TypeId::of::<u8>(),
            BitType::U16 => TypeId::of::<u16>(),
            BitType::F32 => TypeId::of::<f32>(),
            _ => unimplemented!("Bit-depth :{:?}", depth)
        };

        Self::new_with_length_and_type(length, t_r)
    }

    /// Return the type id which gives the representation of the bytes
    /// in the image
    ///
    /// This allows some sort of dynamic type checking
    ///
    /// # Example
    /// ```
    /// use std::any::{Any, TypeId};
    /// use zune_image::channel::Channel;
    /// let channel = Channel::new::<u8>();
    ///
    /// assert_eq!(channel.type_id(),TypeId::of::<u8>());
    /// ```
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }
    /// Create a new channel with the specified capacity
    /// and zero length
    ///
    /// # Example
    /// ```
    /// use zune_image::channel::Channel;
    /// let channel = Channel::new_with_capacity::<u16>(100);    
    /// assert!(channel.is_empty());
    /// ```
    pub fn new_with_capacity<T: 'static + Zeroable>(capacity: usize) -> Channel {
        Self::new_with_capacity_and_type(capacity, TypeId::of::<T>())
    }

    /// Create a new channel with a specified
    /// capacity and type
    ///
    /// # Arguments
    ///
    /// * `capacity`: The capacity of the new channel
    /// * `type_id`:  The type the channel will be storing
    ///
    /// returns: Channel
    ///
    pub(crate) fn new_with_capacity_and_type(capacity: usize, type_id: TypeId) -> Channel {
        let ptr = unsafe { Self::alloc(capacity) };

        Self {
            ptr,
            length: 0,
            capacity,
            type_id
        }
    }

    ///  
    ///
    /// # Arguments
    ///
    /// * `length`: The length of the items to create
    /// * `elm`:  The element to fill it with
    ///
    /// returns: Channel
    ///
    /// # Examples
    ///
    /// ```
    /// use zune_image::channel::Channel;
    /// let chan = Channel::from_elm(100,90_u16);
    /// assert_eq!(chan.reinterpret_as::<u16>().unwrap(),&[90;100]);
    /// ```
    pub fn from_elm<T>(length: usize, elm: T) -> Channel
    where
        T: Clone + Copy + 'static + Zeroable + Pod
    {
        // new currently zeroes memory
        let mut new_chan = Channel::new_with_length::<T>(length * size_of::<T>());

        new_chan.fill(elm).unwrap();

        new_chan
    }
    /// Return true if we can store `extra`
    /// items without resizing/reallocating
    fn has_capacity(&self, extra: usize) -> bool {
        self.length.saturating_add(extra) <= self.capacity
    }
    /// Extend this channel with items from data
    ///
    ///
    pub fn extend<T: Copy + 'static + Zeroable>(&mut self, data: &[T]) {
        assert_eq!(
            TypeId::of::<T>(),
            self.type_id,
            "Type Id's do not match, trying to extend the channel
       with a type it wasn't created with"
        );
        unsafe {
            self.extend_unchecked(data);
        }
    }
    /// Extend items from an array
    ///
    /// # Safety
    ///
    /// - Type of element should match, otherwise behaviour is undefined
    /// - Alignment must match
    unsafe fn extend_unchecked<T: Copy + 'static + Zeroable>(&mut self, data: &[T]) {
        // get size of the generic type
        let data_size = core::mem::size_of::<T>();
        // get number of items we need to store
        let items = data.len().saturating_mul(data_size);
        // check if we need to realloc
        if !self.has_capacity(items) {
            // reallocate to handle enough of the length.
            // realloc will set the new capacity
            // but as callers we have to set the new length
            self.realloc(self.capacity.saturating_add(items).saturating_add(10));
        }
        // now we have enough space, extend

        // Safety
        // - self.ptr+length cannot overflow, since it's usize
        // -  data is valid for data size
        //
        self.ptr.wrapping_add(self.length).copy_from(
            data.as_ptr().cast::<u8>(),
            data.len().saturating_mul(data_size)
        );

        // new length becomes old length + items added
        self.length = self.length.checked_add(items).unwrap();
    }

    /// Reinterpret the channel as being composed of the following type
    ///
    /// The length of the new slice is defined
    /// as size of T over the length of the stored items in the pointer
    pub fn reinterpret_as<T: Default + 'static>(&self) -> Result<&[T], ChannelErrors> {
        // check if the alignment is correct
        // plus we can evenly divide this
        self.confirm_suspicions::<T>()?;

        // safety:
        //  - we confirmed that the type we are reinterpreting as
        //    is the same type the channel was initialized with
        //  - alignment is not an issue since we align all reads
        // to 16 bytes(bytes, not bits), the highest type we have
        // has an alignment of
        Ok(unsafe { self.reinterpret_as_unchecked() })
    }

    /// Reinterpret a slice of `&[u8]` to another type
    ///
    ///  # Safety
    /// - Invariants for [`std::slice::from_raw_parts`] should be upheld
    /// - There should be no sloppy bytes at the end of the memory location
    ///
    /// # Returns
    /// - `Some(&[T])`: THe re-interpreted bits
    unsafe fn reinterpret_as_unchecked<T: Default + 'static>(&self) -> &[T] {
        // Safety:
        //  validity: We own the data
        //  well aligned: You cannot have u8 having bad alignment as the least bit denomination
        // of alignment is a byte and u8==1 byte
        //
        let new_slice = unsafe { std::slice::from_raw_parts_mut::<u8>(self.ptr, self.length) };

        let (a, b, c) = new_slice.align_to();

        assert!(a.is_empty(), "extra sloppy bytes");
        assert!(c.is_empty(), "extra sloppy bytes");

        b
    }
    /// Reinterpret a slice of `&[u8]` into another type
    pub fn reinterpret_as_mut<T: 'static + Pod>(&mut self) -> Result<&mut [T], ChannelErrors> {
        // Get size of pointer
        // check if the alignment is correct + size evenly divides
        self.confirm_suspicions::<T>()?;

        // Safety:
        //  validity: We own the data
        //  well aligned: You cannot have u8 having bad alignment
        //
        let new_slice = unsafe { std::slice::from_raw_parts_mut::<u8>(self.ptr, self.length) };

        let (a, b, c) = bytemuck::pod_align_to_mut(new_slice);

        assert!(a.is_empty(), "extra sloppy bytes");
        assert!(c.is_empty(), "extra sloppy bytes");

        Ok(b)
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
    /// let mut channel = Channel::new::<u8>();
    /// // push a u8
    /// channel.push::<u8>(123);
    /// // then u16
    /// channel.push::<u16>(12553);
    ///
    /// let len = size_of::<u8>()+size_of::<u16>();
    /// // assert that length matches
    /// assert_eq!(channel.len(),len);
    /// ```
    pub fn push<T: Copy + 'static + Zeroable>(&mut self, elm: T) {
        let size = core::mem::size_of::<T>(); // compile time

        if !self.has_capacity(size) {
            unsafe {
                // extend
                // use 3/2 formula
                self.realloc(self.capacity.saturating_mul(size.saturating_mul(3)) / 2);
            }
        }
        // safety:
        // We ensured above there that we have space enough for one more element
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

    /// Fill the channel with a specific element
    ///
    /// # Arguments
    ///
    /// * `element`:  The element to fill the channel
    ///
    /// returns: Result<(), ChannelErrors>
    ///
    /// # Examples
    ///
    /// ```
    /// use zune_image::channel::Channel;
    /// let mut channel = Channel::new_with_length::<u16>(100);
    /// channel.fill(100_u16).unwrap();
    /// assert_eq!(channel.reinterpret_as::<u16>().unwrap(),&[100;50]);
    /// ```
    pub fn fill<T>(&mut self, element: T) -> Result<(), ChannelErrors>
    where
        T: Clone + Copy + 'static + Pod
    {
        // reinterpret to be type T
        let array = self.reinterpret_as_mut()?;
        // Then fill elements
        array.fill(element);

        Ok(())
    }
    /// Confirm that data is aligned and
    ///
    /// the type T can evenly divide length
    fn confirm_suspicions<T: 'static>(&self) -> Result<(), ChannelErrors> {
        // confirm the data is aligned for T
        if !is_aligned::<T>(self.ptr) {
            return Err(ChannelErrors::UnalignedPointer(
                self.ptr as usize,
                size_of::<T>()
            ));
        }

        // confirm we can evenly divide length
        if self.length % size_of::<T>() != 0 {
            return Err(ChannelErrors::UnevenLength(self.length, size_of::<T>()));
        }
        let converted_type_id = TypeId::of::<T>();

        if converted_type_id != self.type_id {
            return Err(ChannelErrors::DifferentType(
                self.type_id,
                converted_type_id
            ));
        }

        Ok(())
    }

    /// Return the raw memory layout of the channel as `&[u8]`
    ///
    /// # Safety
    /// This is unsafe just as a remainder that the memory is just
    /// a bag of bytes and may not be just `&[u8]`.
    pub unsafe fn alias(&self) -> &[u8] {
        std::slice::from_raw_parts(self.ptr, self.length)
    }

    /// Return the raw memory layout of the channel as `mut &[u8]`
    ///
    /// # Safety
    /// This is unsafe just as a remainder that the memory is just
    /// a bag of bytes and may not be just `mut &[u8]`.
    pub unsafe fn alias_mut(&mut self) -> &mut [u8] {
        std::slice::from_raw_parts_mut(self.ptr, self.length)
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        // dealloc storage
        unsafe {
            self.dealloc();
        }
    }
}

/// Check if a pointer is aligned.
fn is_aligned<T>(ptr: *const u8) -> bool {
    let size = core::mem::size_of::<T>();

    (ptr as usize) & ((size) - 1) == 0
}

#[allow(unused_imports)]
mod tests {
    use crate::channel::Channel;

    /// check that we cant convert from a type we made
    #[test]
    fn test_wrong_interpretation() {
        let ch = Channel::new::<u8>();
        assert!(ch.reinterpret_as::<u16>().is_err());
    }

    // test that we return for interpretations that match
    #[test]
    fn test_correct_interpretation() {
        let mut ch = Channel::new::<u16>();
        ch.push(70_u16);
        let expected = [70_u16];
        assert_eq!(ch.reinterpret_as::<u16>().unwrap(), expected);
    }

    #[test]
    fn test_clone_works() {
        let mut ch = Channel::new::<u8>();
        ch.extend::<u8>(&[10; 10]);
        // test clone works
        // clone has some special things
        let ch2 = ch.clone();

        assert_eq!(ch, ch2);
    }
}
