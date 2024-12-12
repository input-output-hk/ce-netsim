pub struct LinkedList<T> {
    head: *mut Entry<T>,
    tail: *mut Entry<T>,
}

#[derive(Debug, PartialEq, Eq)]
struct Entry<T> {
    prev: *mut Entry<T>,
    next: *mut Entry<T>,
    value: Option<T>,
}

pub struct CursorMut<'a, T> {
    entry: *mut Entry<T>,
    _marker: std::marker::PhantomData<&'a mut LinkedList<T>>,
}

impl<T> Entry<T> {
    fn new(value: T) -> Self {
        Self {
            prev: std::ptr::null_mut(),
            next: std::ptr::null_mut(),
            value: Some(value),
        }
    }

    fn sigil() -> Self {
        Self {
            prev: std::ptr::null_mut(),
            next: std::ptr::null_mut(),
            value: None,
        }
    }

    fn detach(&mut self) {
        unsafe {
            (*self.prev).next = self.next;
            (*self.next).prev = self.prev;
        }
    }
}

impl<T> LinkedList<T> {
    pub fn new() -> Self {
        let head = Box::into_raw(Box::new(Entry::sigil()));
        let tail = Box::into_raw(Box::new(Entry::sigil()));

        unsafe {
            (*head).next = tail;
            (*tail).prev = head;
        }

        Self { head, tail }
    }

    pub fn is_empty(&self) -> bool {
        let Some(head) = (unsafe { self.head.as_ref() }) else {
            // we have a sigil in the head/tail so we should always
            // have a value here.

            unreachable!(
                "This usecase should never happen and if you see this error please \
                open an issue on the librarie's github repository."
            )
        };

        head.next.is_null() || std::ptr::addr_eq(head.next, self.tail)
    }

    pub fn push(&mut self, value: T) {
        let entry = Box::into_raw(Box::new(Entry::new(value)));

        self.attach(entry);
    }

    pub fn clear(&mut self) {
        while self.pop().is_some() {}
    }

    pub fn pop(&mut self) -> Option<T> {
        let mut entry_ptr = self.remove_last()?;

        // move the value and let the Box de-allocate the memory
        // on drop (without destruct the value since it has been taken)
        entry_ptr.value.take()
    }

    pub fn cursor_mut(&mut self) -> CursorMut<'_, T> {
        CursorMut {
            entry: unsafe { (*self.head).next },
            _marker: std::marker::PhantomData,
        }
    }

    fn remove_last(&mut self) -> Option<Box<Entry<T>>> {
        let entry_ptr = unsafe { self.tail.as_mut() }?.prev;

        if unsafe { entry_ptr.as_ref() }?.value.is_some() {
            unsafe {
                (*entry_ptr).detach();
                Some(Box::from_raw(entry_ptr))
            }
        } else {
            None
        }
    }

    // Attaches `node` after the sigil `self.head` node.
    fn attach(&mut self, entry: *mut Entry<T>) {
        unsafe {
            (*entry).next = (*self.head).next;
            (*entry).prev = self.head;
            (*self.head).next = entry;
            (*(*entry).next).prev = entry;
        }
    }
}

impl<'a, T> CursorMut<'a, T> {
    pub fn as_ref<'b>(&'b self) -> Option<&'b T>
    where
        'a: 'b, // 'a outlives 'b
    {
        (unsafe { self.entry.as_ref() }?).value.as_ref()
    }

    pub fn as_mut<'b>(&'b mut self) -> Option<&'b mut T>
    where
        'a: 'b, // 'a outlives 'b
    {
        (unsafe { self.entry.as_mut() }?).value.as_mut()
    }

    pub fn move_next(&mut self) {
        if let Some(entry) = unsafe { self.entry.as_mut() } {
            self.entry = entry.next;
        }
    }

    pub fn remove_entry(&mut self) -> Option<T> {
        let next = unsafe { self.entry.as_ref() }?.next;
        let entry = std::mem::replace(&mut self.entry, next);

        let mut entry = unsafe { Box::from_raw(entry) };

        entry.detach();

        entry.value.take()
    }
}

unsafe impl<T> Send for LinkedList<T> where T: Send {}

impl<T> Default for LinkedList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for LinkedList<T> {
    fn drop(&mut self) {
        self.clear();

        unsafe {
            let _ = Box::from_raw(self.head);
            let _ = Box::from_raw(self.tail);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_sigil_pre_conditions() {
        let entry: Entry<()> = Entry::sigil();

        assert_eq!(entry.value, None);
        assert_eq!(entry.next, std::ptr::null_mut());
        assert_eq!(entry.prev, std::ptr::null_mut());
    }

    #[test]
    fn entry_pre_conditions() {
        let entry: Entry<()> = Entry::new(());

        assert_eq!(entry.value, Some(()));
        assert_eq!(entry.next, std::ptr::null_mut());
        assert_eq!(entry.prev, std::ptr::null_mut());
    }

    #[test]
    fn is_empty() {
        let ll: LinkedList<()> = LinkedList::new();

        assert!(ll.is_empty());
    }

    #[test]
    fn pop_empty() {
        let mut ll: LinkedList<()> = LinkedList::new();

        assert_eq!(ll.pop(), None);
    }

    #[test]
    fn push_one() {
        let mut ll: LinkedList<()> = LinkedList::default();

        ll.push(());

        assert!(!ll.is_empty());
    }

    #[test]
    fn pop() {
        let mut ll: LinkedList<()> = LinkedList::new();
        ll.push(());

        assert_eq!(ll.pop(), Some(()));
        assert!(ll.is_empty());
    }

    #[test]
    fn cursor() {
        let mut ll: LinkedList<()> = LinkedList::new();
        for _ in 0..2 {
            ll.push(());
        }

        let mut cursor = ll.cursor_mut();

        assert!(cursor.as_ref().is_some());
        cursor.move_next();
        assert!(cursor.as_ref().is_some());
        cursor.move_next();
        assert!(cursor.as_ref().is_none());
    }

    #[test]
    fn cursor_remove() {
        let mut ll: LinkedList<()> = LinkedList::new();
        for _ in 0..2 {
            ll.push(());
        }

        let mut cursor = ll.cursor_mut();

        assert!(cursor.remove_entry().is_some());
        assert!(cursor.remove_entry().is_some());
        assert!(cursor.as_ref().is_none());
    }

    #[test]
    fn cursor_ref_remove() {
        let mut ll: LinkedList<u8> = LinkedList::new();
        for _ in 0..2 {
            ll.push(0);
        }

        let mut cursor = ll.cursor_mut();

        {
            let value = cursor.as_mut().unwrap();
            *value += 1;
        }

        let value = cursor.remove_entry().unwrap();
        assert_eq!(value, 1);
        assert!(cursor.remove_entry().is_some());
        assert!(cursor.as_ref().is_none());
    }
}
