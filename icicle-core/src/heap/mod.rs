//! Garbage-collected heaps.
//!
//! Garbage-collected heaps can be implemented entirely in safe Rust,
//! with a layer of indirection using abstract "object identifiers".
//! However, this implementation uses pointers to objects directly.
//! With the desire to keep the interface safe, this is non-trivial.
//! The interface hence provides various ways to
//! interact with the heap and heap-allocated objects,
//! with different ways to keep track of root references.
//! Accessing objects through root references is completely safe,
//! and by using the right types of root references, has little overhead.
//!
//! # Components
//!
//! The diagram below shows the relationships between
//! the different types of objects in this module.
//!
#![doc = include_str!(concat!(env!("OUT_DIR"), "/heap_diagram.drawio.svg"))]
//!
//! ## Notes
//!
//!  1. The object borrows using a pointer or reference.
//!  2. The object borrows using pointer arithmetic on itself.
//!  3. The object registers and unregisters itself
//!     during construction, cloning, and dropping.

pub use self::{block::*, pre_alloc::*, refs::*};

use {
    non_zero_ext::NonZeroExt,
    scope_exit::scope_exit,
    std::{
        cell::{RefCell, UnsafeCell},
        collections::{HashMap, HashSet},
        marker::{PhantomData, PhantomPinned},
        mem::{ManuallyDrop, replace},
        num::NonZeroU64,
        pin::Pin,
        ptr::NonNull,
        sync::Mutex,
    },
};

pub mod object;

mod block;
mod pre_alloc;
mod refs;

/// Ensure that `'h` is an invariant lifetime.
type HeapId<'h> = PhantomData<fn(&'h ()) -> &'h ()>;

/// Garbage-collected heap.
///
/// The `'h` parameter identifies the heap at the type level.
/// This prevents objects from pointing to objects on different heaps,
/// which would cause the garbage collector to crash horribly.
/// The `'h` parameter can also be used as a lifetime for the heap.
pub struct Heap<'h>
{
    /// Uniquely identifies this heap.
    _heap_id: HeapId<'h>,

    /// Heaps are referenced all over the place.
    _pinned: PhantomPinned,

    /// Pre-allocated objects.
    pub pre_alloc: PreAlloc<'h>,

    /// Non-allocator blocks that constitute the heap.
    ///
    /// Note that more blocks can be found in the mutators.
    blocks: Mutex<Vec<Block<'h>>>,

    /// Tracks the existence of each mutator.
    ///
    /// Each mutator must be known to the garbage collector,
    /// so that the garbage collector can see its stack root batches.
    /// The entries in this set are automatically maintained
    /// by [`Mutator::new`] and [`Mutator::drop`].
    mutators: Mutex<HashSet<NonNull<Mutator<'h>>>>,

    /// Tracks the existence of each pinned root.
    ///
    /// This map stores for each object how many pinned roots reference it.
    /// If an object exists in this map, the garbage collector
    /// is prohibited from moving or garbage collecting the object.
    /// The entries in this map are automatically maintained
    /// by [`PinnedRoot::new`] and [`PinnedRoot::drop`].
    pinned_roots: Mutex<HashMap<UnsafeRef<'h>, NonZeroU64>>,
}

impl<'h> Heap<'h>
{
    /// Create a heap with a unique `'h` parameter.
    ///
    /// The heap is passed to the given function.
    /// When the function returns or panics, the heap is destroyed.
    pub fn with<F, R>(f: F) -> R
        // NOTE: Using Self here would allow the caller to choose 'h.
        //       That could result in multiple heaps with the same 'h.
        // NOTE: The heap must be behind a reference.
        //       Otherwise the given function could move it.
        //       This must be prevented, because heaps are referenced
        //       in several places the borrow checker is unaware of.
        where F: for<'i> FnOnce(&'i Heap<'i>) -> R
    {
        let heap = Heap{
            _heap_id: PhantomData,
            _pinned: PhantomPinned,
            pre_alloc: PreAlloc::dangling(),
            blocks: Mutex::new(Vec::new()),
            mutators: Mutex::new(HashSet::new()),
            pinned_roots: Mutex::new(HashMap::new()),
        };

        // SAFETY: Called exactly once during heap construction.
        unsafe { heap.pre_alloc.init(&heap); }

        f(&heap)
    }

    /// Add a block to the heap.
    fn add_block(&self, block: Block<'h>)
    {
        let mut blocks = self.blocks.lock().unwrap();
        blocks.push(block);
    }

    /// Register a mutator with the heap.
    ///
    /// # Safety
    ///
    /// Must only be called by [`Mutator::new`].
    unsafe fn register_mutator(
        &'h self,
        mutator: NonNull<Mutator<'h>>,
    )
    {
        let mut set = self.mutators.lock().unwrap();
        set.insert(mutator);
    }

    /// Unregister a mutator with the heap.
    ///
    /// # Safety
    ///
    /// Must only be called by [`Mutator::drop`].
    unsafe fn unregister_mutator(
        &'h self,
        mutator: NonNull<Mutator<'h>>,
    )
    {
        let mut set = self.mutators.lock().unwrap();
        set.take(&mutator).expect("Use-after-drop of mutator");
    }

    /// Increment the pinned root count for an object.
    ///
    /// # Safety
    ///
    /// Must only be called by [`PinnedRoot::new`].
    unsafe fn retain_pinned_root(&self, object: UnsafeRef<'h>)
    {
        const ERR: &str = "Too many pinned roots for object";
        let mut pinned_roots = self.pinned_roots.lock().unwrap();
        pinned_roots.entry(object)
            .and_modify(|n| *n = n.checked_add(1).expect(ERR))
            .or_insert(NonZeroU64::ONE);
    }

    /// Decrement the pinned root count for an object.
    ///
    /// # Safety
    ///
    /// Must only be called by [`PinnedRoot::drop`].
    unsafe fn release_pinned_root(&self, object: UnsafeRef<'h>)
    {
        use std::collections::hash_map::Entry::*;
        let mut pinned_roots = self.pinned_roots.lock().unwrap();
        match pinned_roots.entry(object) {
            Occupied(mut entry) =>
                match NonZeroU64::new(entry.get().get() - 1) {
                    Some(n) => { entry.insert(n); },
                    None    => { entry.remove_entry(); },
                },
            Vacant(..) =>
                unreachable!("Use-after-drop of pinned root"),
        }
    }
}

/// Thread-local state regarding garbage-collected heaps.
///
/// Locking a mutex for every single operation is too slow.
/// Mutators provide thread-local state for many operations,
/// which is synchronized with the heap only during
/// [safe points] and other infrequent events.
///
/// [safe points]: `Self::safe_point`
pub struct Mutator<'h>
{
    /// The heap to which this mutator belongs.
    pub heap: &'h Heap<'h>,

    /// Mutators are referenced by heaps.
    _pinned: PhantomPinned,

    /// Block on which new small objects are allocated.
    allocator: ManuallyDrop<UnsafeCell<Block<'h>>>,

    /// Active stack root batches maintained by [`with_stack_roots`].
    ///
    /// [`with_stack_roots`]: `Self::with_stack_roots`
    stack_root_batches: RefCell<Vec<*const [StackRoot<'h>]>>,

    /// Active pinned stack roots maintained by [`with_pinned_stack_root`].
    ///
    /// [`with_pinned_stack_root`]: `Self::with_pinned_stack_root`
    pinned_stack_roots: RefCell<Vec<UnsafeRef<'h>>>,
}

impl<'h> Mutator<'h>
{
    /// Create a mutator for a heap.
    ///
    /// Creating a mutator is not a zero-cost operation.
    /// Please create one such state per thread and keep it around.
    pub fn new(heap: &'h Heap<'h>) -> Pin<Box<Self>>
    {
        let this = Self{
            heap,
            _pinned: PhantomPinned,
            allocator: ManuallyDrop::new(UnsafeCell::new(Block::new(heap))),
            stack_root_batches: RefCell::new(Vec::new()),
            pinned_stack_roots: RefCell::new(Vec::new()),
        };

        let this = Box::into_pin(Box::new(this));
        let ptr = NonNull::from(this.as_ref().get_ref());

        // SAFETY: Called from Mutator::new.
        unsafe { heap.register_mutator(ptr) };

        this
    }

    /// Enter a safe point.
    ///
    /// Once a garbage collection cycle is planned,
    /// the garbage collector must ensure all mutators
    /// have entered a safe point, during which they cannot mutate.
    /// This method blocks until the planned garbage collection cycle finishes.
    /// This approach is known as "stop the world".
    ///
    /// If no garbage collection cycle is planned,
    /// this method returns immediately.
    pub fn safe_point(&self)
    {
        // SAFETY: The passed function does nothing.
        unsafe { self.safe_point_with(|| ()); }
    }

    /// Enter a safe point but don't block immediately.
    ///
    /// A safe point is entered, and the given function is called immediately.
    /// The function will run in parallel with the garbage collector.
    /// This is similar to [`safe_point`][`Self::safe_point`],
    /// but blocking does not occur until the function returns.
    /// The main purpose of this method is to allow a safe point to exist
    /// (and hence garbage collections to proceed) during FFI calls.
    ///
    /// # Safety
    ///
    /// The given function must not perform allocations,
    /// mutate objects, or read unpinned objects.
    pub unsafe fn safe_point_with<F, R>(&self, f: F) -> R
        where F: FnOnce() -> R
    {
        // TODO: Implement the safe point logic.
        f()
    }

    /// Allocate memory for an object.
    ///
    /// # Safety
    ///
    /// The caller must initialize the allocated memory
    /// before the next garbage collection cycle.
    pub unsafe fn alloc(&self, size: usize) -> NonNull<()>
    {
        if size > DEFAULT_BLOCK_SIZE {
            return self.alloc_large(size);
        }

        if let Some(ptr) = self.alloc_small_fast(size) {
            return ptr;
        }

        self.alloc_small_slow(size)
    }

    /// Allocate an ad-hoc block for this one value.
    #[inline(never)]
    unsafe fn alloc_large(&self, size: usize) -> NonNull<()>
    {
        let mut block = Block::with_capacity(self.heap, size);
        let ptr = block.try_alloc(size)
            .expect("Block should have sufficient space");
        self.heap.add_block(block);
        return ptr;
    }

    /// Try a pointer bump allocation for the value.
    ///
    /// If the value does not fit within the allocator,
    /// this method returns [`None`] and the caller must try elsewhere.
    #[inline(always)]
    unsafe fn alloc_small_fast(&self, size: usize) -> Option<NonNull<()>>
    {
        let block = self.allocator.get();
        (*block).try_alloc(size)
    }

    /// Allocate a new block and allocate the value in there.
    ///
    /// The new block becomes the new allocator for this mutator.
    #[inline(never)]
    unsafe fn alloc_small_slow(&self, size: usize) -> NonNull<()>
    {
        let block = self.allocator.get();

        let mut new_block = Block::new(self.heap);
        let ptr = new_block.try_alloc(size)
            .expect("Block should have sufficient space");

        let old_block = replace(&mut *block, new_block);
        self.heap.add_block(old_block);

        ptr
    }

    /// Allocate stack space for roots.
    ///
    /// A stack root ensures the referenced object won't be garbage collected.
    /// Stack roots are so called because they always live on the Rust stack.
    /// Calling `with_stack_roots::<N>` is more efficient
    /// than working with `N` individual [pinned roots],
    /// but the usage of stack roots is restricted to subsequent stack frames.
    /// The efficiency of stack roots originates from the following factors:
    ///
    ///  - Stack roots are immovable, because they are always behind a `&`.
    ///    Therefore the garbage collector can track their locations,
    ///    and they do not inhibit moving of objects by the garbage collector.
    ///  - The garbage collector keeps track of stack roots in batches,
    ///    rather than keeping track of every individual root separately.
    ///  - Stack root batches are always created and destroyed in LIFO order,
    ///    because of the "with" interface presented by this method.
    ///    This simplifies bookkeeping (using a vec instead of a hash map).
    ///
    /// The stack roots are passed to the given function.
    /// When the function returns or panics, the stack roots are destroyed.
    /// The function may freely [`set`] the stack roots;
    /// the garbage collector will notice their new objects.
    /// Each stack root is initialized to a reference to undef.
    ///
    /// [pinned roots]: `PinnedRoot`
    /// [`set`]: `StackRoot::set`
    pub fn with_stack_roots<const N: usize, F, R>(&self, f: F) -> R
        where F: FnOnce(&[StackRoot<'h>; N]) -> R
    {
        // Initialize the stack root batch with undefs.
        // StackRoot doesn't impl Copy so we can't use [StackRoot{..}; N].
        let undef = self.heap.pre_alloc.undef();
        let new_root = |()| unsafe { StackRoot::new(undef) };
        let batch = [(); N].map(new_root);

        // Add the batch to the stack that the garbage collector traverses.
        let mut batches = self.stack_root_batches.borrow_mut();
        batches.push(&batch);
        drop(batches);

        // Remove the batch from the stack when f returns or panics.
        scope_exit! {
            let mut batches = self.stack_root_batches.borrow_mut();
            batches.pop().expect("stack_root_batches should not be empty");
        }

        // Call the given function with the batch.
        f(&batch)
    }

    /// Create a pinned stack root.
    ///
    /// This is nearly identical to [`with_stack_roots`].
    /// Like stack roots, pinned stack roots are
    /// much more efficient to work with than pinned roots.
    /// The following differences exist between
    /// stack roots and pinned stack roots:
    ///
    ///  - The pinned stack root is initialized using the given reference.
    ///  - The pinned stack root cannot be modified after its creation.
    ///  - The pinned stack root inhibits moving of the object
    ///    by the garbage collector.
    ///
    /// [`with_stack_roots`]: `Self::with_stack_roots`
    pub fn with_pinned_stack_root<R>(
        &self,
        object: &impl BorrowRef<'h>,
        f: impl FnOnce(&PinnedStackRoot<'h>) -> R,
    ) -> R
    {
        unsafe { self.with_pinned_stack_root_unsafe(object.borrow_ref(), f) }
    }

    /// Create a pinned stack root.
    ///
    /// # Safety
    ///
    /// The reference must reference a live object.
    pub unsafe fn with_pinned_stack_root_unsafe<R>(
        &self,
        object: UnsafeRef<'h>,
        f: impl FnOnce(&PinnedStackRoot<'h>) -> R,
    ) -> R
    {
        let root = PinnedStackRoot::new(object);

        let mut roots = self.pinned_stack_roots.borrow_mut();
        roots.push(object);
        drop(roots);

        scope_exit! {
            let mut roots = self.pinned_stack_roots.borrow_mut();
            roots.pop().expect("pinned_stack_roots should not be empty");
        }

        // Call the given function with the root.
        f(&root)
    }
}

impl<'h> Drop for Mutator<'h>
{
    fn drop(&mut self)
    {
        // Make sure the allocator is not dropped,
        // by transferring ownership of it to the heap.
        // SAFETY: Allocator is not used anymore after.
        let allocator = unsafe { ManuallyDrop::take(&mut self.allocator) };
        self.heap.add_block(allocator.into_inner());

        // SAFETY: Called from Mutator::drop.
        unsafe { self.heap.unregister_mutator(NonNull::from(self)); }
    }
}
