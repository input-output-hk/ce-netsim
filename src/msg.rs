use crate::SimId;
use std::time::SystemTime;

pub trait HasBytesSize: Send + 'static {
    fn bytes_size(&self) -> u64;
}

pub struct Msg<T> {
    from: SimId,
    to: SimId,
    sent: SystemTime,
    content: T,
}

pub struct MsgWith<T> {
    pub msg: Msg<T>,
    pub reception_time: SystemTime,
}

pub struct OrderedByTime<T>(pub MsgWith<T>);

impl<T> OrderedByTime<T> {
    pub fn inner(&self) -> &MsgWith<T> {
        &self.0
    }

    pub fn into_inner(self) -> MsgWith<T> {
        self.0
    }
}

impl<T> PartialEq for OrderedByTime<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.reception_time == other.0.reception_time
    }
}

impl<T> Eq for OrderedByTime<T> {}

impl<T> PartialOrd for OrderedByTime<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.reception_time.partial_cmp(&other.0.reception_time)
    }
}
impl<T> Ord for OrderedByTime<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.reception_time.cmp(&other.0.reception_time)
    }
}

impl<T> Msg<T> {
    pub fn new(from: SimId, to: SimId, content: T) -> Self {
        Self {
            from,
            to,
            sent: SystemTime::now(),
            content,
        }
    }

    pub fn from(&self) -> &SimId {
        &self.from
    }

    pub fn to(&self) -> &SimId {
        &self.to
    }

    pub fn sent(&self) -> SystemTime {
        self.sent
    }

    pub fn content(&self) -> &T {
        &self.content
    }

    pub fn into_content(self) -> T {
        self.content
    }
}

impl HasBytesSize for [u8] {
    fn bytes_size(&self) -> u64 {
        self.len() as u64
    }
}
impl HasBytesSize for &'static str {
    fn bytes_size(&self) -> u64 {
        self.as_bytes().bytes_size()
    }
}
impl HasBytesSize for str {
    fn bytes_size(&self) -> u64 {
        self.as_bytes().bytes_size()
    }
}
impl HasBytesSize for Vec<u8> {
    fn bytes_size(&self) -> u64 {
        (self.capacity() + std::mem::size_of_val(self)) as u64
    }
}
impl HasBytesSize for String {
    fn bytes_size(&self) -> u64 {
        (self.capacity() + std::mem::size_of_val(self)) as u64
    }
}
