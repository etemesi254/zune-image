use std::num::NonZeroUsize;

/// This influences global image operation options and allows one to set global options like
/// whether to use multi-thread or single threaded execution for operations
/// implemented under image operations in `zune-imageprocs`
///
/// The functionalities are defined here so that each individual image can carry its associated
/// functionality inside itself instead of it being a global setting
#[derive(Copy, Clone, Debug)]
pub struct ImageOperationOptions {
    // whether or not operations should use threads
    use_threads: bool,
    // how many threads to use
    num_threads_child: usize,
}

impl Default for ImageOperationOptions {
    fn default() -> Self {
        let use_threads = cfg!(feature = "threads");
        let num_threads_child = if use_threads {
            // conservative 4 threads if we can't determine actual threads
            // chosen by random dice
            let global_threads = std::thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(4).unwrap())
                .get();
            // so now for local threads we must ensure its not less than 1, so we do a simple div_ceil
            // and assume its RGB for which we have 3, we get (global+2)/3 which is good enough
            global_threads.div_ceil(3)
        } else {
            1 /*conservative*/
        };
        ImageOperationOptions::new(use_threads, num_threads_child)
    }
}
impl ImageOperationOptions {
    /// Create a new operation options items
    pub fn new(use_threads: bool, num_threads_child: usize) -> Self {
        ImageOperationOptions {
            use_threads,
            num_threads_child,
        }
    }
    /// Set whether to use threads for operations where necessary
    pub fn set_use_threads(&mut self, use_threads: bool) -> &mut Self {
        self.use_threads = use_threads;
        self
    }
    /// Set number of threads per channel used in processing
    pub fn set_num_threads_child(&mut self, num_threads: usize) -> &mut Self {
        self.num_threads_child = num_threads;
        self
    }
    /// Return how many threads a child operation is allowed to consume
    pub const fn num_threads_child(&self) -> usize {
        self.num_threads_child
    }
    /// Return whether we can use threads
    pub const fn use_threads(&self) -> bool {
        self.use_threads
    }
}
