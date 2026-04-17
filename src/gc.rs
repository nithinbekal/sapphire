/// Handle into the GC heap. A `u32` index into `GcHeap::objects`.
pub type GcRef = u32;

/// Types managed by the GC must implement `Trace` so the collector can
/// follow interior references during the mark phase.
pub trait Trace {
    fn trace(&self, out: &mut Vec<GcRef>);
}

/// A simple stop-the-world mark-and-sweep heap.
///
/// Slots are reused via a free-list; the backing `Vec` only grows.
/// After each collection the threshold is set to `max(live * 2, 256)` so
/// that allocation cost amortises over object lifetime.
pub struct GcHeap<T: Trace> {
    objects: Vec<Option<T>>,
    marks: Vec<bool>,
    free_list: Vec<u32>,
    allocated: usize,
    threshold: usize,
}

impl<T: Trace> Default for GcHeap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Trace> GcHeap<T> {
    pub fn new() -> Self {
        GcHeap {
            objects: Vec::new(),
            marks: Vec::new(),
            free_list: Vec::new(),
            allocated: 0,
            threshold: 256,
        }
    }

    /// Allocate a new object and return its handle.
    pub fn alloc(&mut self, obj: T) -> GcRef {
        self.allocated += 1;
        if let Some(slot) = self.free_list.pop() {
            self.objects[slot as usize] = Some(obj);
            self.marks[slot as usize] = false;
            slot
        } else {
            let slot = self.objects.len() as u32;
            self.objects.push(Some(obj));
            self.marks.push(false);
            slot
        }
    }

    pub fn get(&self, r: GcRef) -> &T {
        self.objects[r as usize].as_ref().expect("dangling GcRef")
    }

    pub fn get_mut(&mut self, r: GcRef) -> &mut T {
        self.objects[r as usize].as_mut().expect("dangling GcRef")
    }

    pub fn should_collect(&self) -> bool {
        self.allocated >= self.threshold
    }

    /// Mark all objects reachable from `roots`, then sweep the rest.
    pub fn collect(&mut self, roots: &[GcRef]) {
        // Clear marks.
        self.marks.iter_mut().for_each(|m| *m = false);

        // Mark phase: iterative DFS from roots.
        let mut worklist = roots.to_vec();
        while let Some(r) = worklist.pop() {
            let i = r as usize;
            if i >= self.marks.len() || self.marks[i] {
                continue;
            }
            self.marks[i] = true;
            if let Some(obj) = &self.objects[i] {
                obj.trace(&mut worklist);
            }
        }

        // Sweep phase: free unmarked slots.
        for i in 0..self.objects.len() {
            if self.objects[i].is_some() && !self.marks[i] {
                self.objects[i] = None;
                self.free_list.push(i as u32);
            }
        }

        let live = self.objects.iter().filter(|o| o.is_some()).count();
        self.threshold = (live * 2).max(256);
        self.allocated = 0;
    }
}
