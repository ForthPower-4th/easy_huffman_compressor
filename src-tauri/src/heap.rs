

#[derive(Debug)]
pub struct Heap<T> where T: Ord{
    heap_vec: Vec<T>, // simulate a full tree by a vector
}

impl<T : Ord > Heap<T>{
    pub fn new(items: impl IntoIterator<Item = T> ) -> Heap<T>{ // build a heap from an iterator
        let mut heap_vec:Vec<T> = items.into_iter().collect();
        let length = heap_vec.len();
        for i in (0..length / 2).rev(){ // heapify
            Self::sift_down(&mut heap_vec, i);
        }
        Self{heap_vec}
    }
    fn sift_down(heap :&mut Vec<T>, mut i: usize){
        let len = heap.len();
        if i >= len{
            debug_assert!(i < len, "sift_down: i={i} out of bounds, len={len}");
        }
        loop{
            let l = 2 * i + 1;
            let r = 2 * i + 2;
            if l >= len{
                break; // leaf node
            }
            if r >= len{
                if heap[l] < heap[i]{
                    heap.swap(i, l); // only check the size relationship between left child and current node
                }
                break;
            }
            let smallest = if heap[l] <= heap[i] && heap[l] < heap[r] {
                l
            } else if heap[r] < heap[i] && heap[r] <= heap[l] {
                r
            }
            else{
                break;
            };
            heap.swap(i, smallest);
            i = smallest;
        }
    }
    pub fn push(&mut self, item: T){
        self.heap_vec.push(item);
        let mut current_idx = self.heap_vec.len() - 1;
        while current_idx > 0{
            let parent_idx = (current_idx - 1) / 2;
            if self.heap_vec[current_idx] < self.heap_vec[parent_idx]{
                self.heap_vec.swap(parent_idx, current_idx);
            }
            else{
                break;
            }
            current_idx = parent_idx;
        }
    }
    pub fn pop(&mut self) -> Option<T>{
        if self.heap_vec.is_empty(){
            return None;
        }
        let len = self.heap_vec.len();
        self.heap_vec.swap(0,len - 1);// move the top to the end
        let result = self.heap_vec.pop(); // pop out the end, i.e. original top
        if !self.heap_vec.is_empty(){
            Self::sift_down(&mut self.heap_vec, 0);
        }
        result
    }
    pub fn len(&self) -> usize {
        self.heap_vec.len()
    }
}
