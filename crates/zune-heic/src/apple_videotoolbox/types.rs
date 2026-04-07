// ── Types ────────────────────────────────────────────────────────────────────
#![allow(non_snake_case)]
use std::ffi::c_void;

pub type OSStatus = i32;

// Opaque pointer types – we only ever hold/pass pointers, never deref them.
#[repr(C)] pub struct __CFAllocator(());
pub type CFAllocatorRef       = *const __CFAllocator;
pub type CFTypeRef            = *const core::ffi::c_void;

#[repr(C)] pub struct __CMBlockBuffer(());
pub type CMBlockBufferRef     = *mut __CMBlockBuffer;

#[repr(C)] pub struct __CMSampleBuffer(());
pub type CMSampleBufferRef    = *mut __CMSampleBuffer;

#[repr(C)] pub struct __CMVideoFormatDescription(());
pub type CMVideoFormatDescriptionRef = *mut __CMVideoFormatDescription;

#[repr(C)] pub struct __CVBuffer(());
pub type CVImageBufferRef     = *mut __CVBuffer;

#[repr(C)] pub struct __VTDecompressionSession(());
pub type VTDecompressionSessionRef = *mut __VTDecompressionSession;

pub type VTDecodeInfoFlags = u32;

/// Matches the CoreMedia definition exactly.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct CMTime {
    pub value:      i64,
    pub timescale:  i32,
    pub flags:      u32,
    pub epoch:      i64,
}

/// The two-field record VideoToolbox expects.
#[repr(C)]
pub struct VTDecompressionOutputCallbackRecord {
    pub decompressionOutputCallback: unsafe extern "C" fn(
        decompressionOutputRefCon: *mut c_void,
        sourceFrameRefCon:         *mut c_void,
        status:                    OSStatus,
        infoFlags:                 VTDecodeInfoFlags,
        imageBuffer:               CVImageBufferRef,
        presentationTimeStamp:     CMTime,
        presentationDuration:      CMTime,
    ),
    pub decompressionOutputRefCon: *mut c_void,
}

// kCFAllocatorNull is a global symbol exported by CoreFoundation.
// Binding it as an extern static is the correct approach.
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub static kCFAllocatorNull: CFAllocatorRef;
    pub fn CFRelease(cf: CFTypeRef);
}

#[link(name = "CoreMedia", kind = "framework")]
unsafe extern "C" {
    pub fn CMBlockBufferCreateWithMemoryBlock(
        structureAllocator:  CFAllocatorRef,
        memoryBlock:         *mut c_void,
        blockLength:         usize,
        blockAllocator:      CFAllocatorRef,
        customBlockSource:   *const c_void,
        offsetToData:        usize,
        dataLength:          usize,
        flags:               u32,
        blockBufferOut:      *mut CMBlockBufferRef,
    ) -> OSStatus;

    pub fn CMSampleBufferCreateReady(
        allocator:                CFAllocatorRef,
        dataBuffer:               CMBlockBufferRef,
        formatDescription:        CMVideoFormatDescriptionRef,
        numSamples:               isize,
        numSampleTimingEntries:   isize,
        sampleTimingArray:        *const c_void,
        numSampleSizeEntries:     isize,
        sampleSizeArray:          *const usize,
        sampleBufferOut:          *mut CMSampleBufferRef,
    ) -> OSStatus;

    pub fn CMVideoFormatDescriptionCreateFromHEVCParameterSets(
        allocator:                CFAllocatorRef,
        parameterSetCount:        usize,
        parameterSetPointers:     *const *const u8,
        parameterSetSizes:        *const usize,
        nalUnitHeaderLength:      i32,
        extensions:               *const c_void,
        formatDescriptionOut:     *mut CMVideoFormatDescriptionRef,
    ) -> OSStatus;
}

#[link(name = "VideoToolbox", kind = "framework")]
unsafe extern "C" {
    pub fn VTDecompressionSessionCreate(
        allocator:                    CFAllocatorRef,
        videoFormatDescription:       CMVideoFormatDescriptionRef,
        videoDecoderSpecification:    *mut c_void,
        destinationImageBufferAttributes: *mut c_void,
        outputCallback:               *const VTDecompressionOutputCallbackRecord,
        decompressionSessionOut:      *mut VTDecompressionSessionRef,
    ) -> OSStatus;

    pub fn VTDecompressionSessionDecodeFrame(
        session:           VTDecompressionSessionRef,
        sampleBuffer:      CMSampleBufferRef,
        decodeFlags:       u32,
        sourceFrameRefCon: *mut c_void,
        infoFlagsOut:      *mut VTDecodeInfoFlags,
    ) -> OSStatus;

    pub fn VTDecompressionSessionWaitForAsynchronousFrames(
        session: VTDecompressionSessionRef,
    ) -> OSStatus;
}

#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    pub fn CVPixelBufferLockBaseAddress(pixelBuffer: CVImageBufferRef, lockFlags: u32) -> i32;
    pub fn CVPixelBufferUnlockBaseAddress(pixelBuffer: CVImageBufferRef, lockFlags: u32) -> i32;
    pub fn CVPixelBufferGetWidth(pixelBuffer: CVImageBufferRef) -> usize;
    pub fn CVPixelBufferGetHeight(pixelBuffer: CVImageBufferRef) -> usize;
    pub fn CVPixelBufferGetBaseAddressOfPlane(pixelBuffer: CVImageBufferRef, planeIndex: usize) -> *mut u8;
    pub fn CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer: CVImageBufferRef, planeIndex: usize) -> usize;
}