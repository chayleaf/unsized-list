//! This was done purely for fun and learning
//! Tested with miri, so there aren't any surface-level bugs
use std::{marker::PhantomData, mem::MaybeUninit, ops::{Deref, DerefMut}, path::Path, ffi::{OsStr, CStr}, os::unix::prelude::OsStrExt};

pub struct ListSlice<T: ?Sized> {
    align: usize,
    chunks: *mut u8,
    chunks_len: usize,
    _ph: PhantomData<T>,
}

pub struct ListSliceTail<'a, T: ?Sized> {
    inner: ListSlice<T>,
    _ph: PhantomData<&'a T>,
}

pub struct ListSliceTailMut<'a, T: ?Sized> {
    inner: ListSlice<T>,
    _ph: PhantomData<&'a T>,
}

impl<T: ?Sized + CopyableUnsized> ListSlice<T> {
    pub fn head(&self) -> Option<&T> {
        let data = unsafe { std::slice::from_raw_parts(self.chunks, self.chunks_len) };
        let len = usize::from_ne_bytes(
            data.get(0..std::mem::size_of::<usize>())
                .and_then(|x| TryFrom::try_from(x).ok())?,
        );
        data.get(std::mem::size_of::<usize>()..std::mem::size_of::<usize>() + len)
            .map(|x| unsafe {
                T::construct_ptr(x.as_ptr(), len)
            })
    }
    pub fn head_tail_mut(&mut self) -> Option<(&mut T, Option<ListSliceTailMut<T>>)> {
        #[allow(clippy::cast_ref_to_mut)]
        let head = self.head().map(|x| unsafe { &mut *(x as *const T as *mut T) })?;
        Some((head, self.tail_mut()))
    }
    pub fn head_mut(&mut self) -> Option<&mut T> {
        #[allow(clippy::cast_ref_to_mut)]
        self.head().map(|x| unsafe { &mut *(x as *const T as *mut T) })
    }
}

impl<T: ?Sized> ListSlice<T> {
    /// n: used for constructing fat pointers. Probably unstable and might break in future
    /// Rust versions
    ///
    /// # Safety
    /// a fat pointer must be constructible from a pointer and a byte count divided by n
    pub unsafe fn head_unsafe(&self, n: usize) -> Option<&T> {
        let data = std::slice::from_raw_parts(self.chunks, self.chunks_len);
        let len = usize::from_ne_bytes(
            data.get(0..std::mem::size_of::<usize>())
                .and_then(|x| TryFrom::try_from(x).ok())?,
        );
        data.get(std::mem::size_of::<usize>()..std::mem::size_of::<usize>() + len)
            .map(|x| unsafe {
                *std::mem::transmute::<&(*const u8, usize), &&T>(&(x.as_ptr(), len / n))
            })
    }
    /// # Safety
    /// see head_unsafe
    pub unsafe fn head_mut_unsafe(&mut self, n: usize) -> Option<&mut T> {
        #[allow(clippy::cast_ref_to_mut)]
        self.head_unsafe(n).map(|x| unsafe { &mut *(x as *const T as *mut T) })
    }
    pub fn tail(&self) -> Option<ListSliceTail<T>> {
        let data = unsafe { std::slice::from_raw_parts(self.chunks, self.chunks_len) };
        let len = std::mem::size_of::<usize>()
            + usize::from_ne_bytes(
                data.get(0..std::mem::size_of::<usize>())
                    .and_then(|x| TryFrom::try_from(x).ok())?,
            );
        let aligned_len = if len % self.align == 0 {
            len
        } else {
            len + self.align - len % self.align
        };
        Some(ListSliceTail {
            inner: Self {
                chunks: unsafe { self.chunks.add(aligned_len) },
                chunks_len: self.chunks_len.checked_sub(aligned_len)?,
                align: self.align,
                _ph: self._ph,
            },
            _ph: PhantomData::default(),
        })
    }
    pub fn tail_mut(&mut self) -> Option<ListSliceTailMut<T>> {
        self.tail().map(|x| ListSliceTailMut { inner: x.inner, _ph: x._ph })
    }
    /// # Safety
    /// see head_unsafe
    pub unsafe fn head_tail_mut_unsafe(&mut self, n: usize) -> Option<(&mut T, Option<ListSliceTailMut<T>>)> {
        #[allow(clippy::cast_ref_to_mut)]
        let head = self.head_unsafe(n).map(|x| unsafe { &mut *(x as *const T as *mut T) })?;
        Some((head, self.tail_mut()))
    }
}

impl<T: ?Sized> Deref for ListSliceTail<'_, T> {
    type Target = ListSlice<T>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T: ?Sized> Deref for ListSliceTailMut<'_, T> {
    type Target = ListSlice<T>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T: ?Sized> DerefMut for ListSliceTailMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub struct List<T: ?Sized> {
    inner: ListSlice<T>,
    chunks_cap: usize,
}

impl<T: ?Sized> std::fmt::Debug for ListSlice<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ListSlice(&[ ")?;
        for i in 0..self.chunks_len {
            write!(f, "{} ", unsafe { *self.chunks.add(i) })?;
        }
        f.write_str("])")
    }
}

impl<T: ?Sized> std::fmt::Debug for List<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("List(&[ ")?;
        for i in 0..self.inner.chunks_len {
            write!(f, "{} ", unsafe { *self.inner.chunks.add(i) })?;
        }
        f.write_str("])")
    }
}

/// # Safety
/// type must be copyable byte-by-byte
pub unsafe trait CopyableUnsized {
    /// data: pointer
    /// len: byte count
    ///
    /// # Safety
    /// valid pointer and stuff
    unsafe fn construct_ptr<'a>(data: *const u8, len: usize) -> &'a Self;
}
unsafe impl<T: Copy> CopyableUnsized for [T] {
    unsafe fn construct_ptr<'a>(data: *const u8, len: usize) -> &'a Self {
        debug_assert_eq!(len % std::mem::size_of::<T>(), 0);
        std::slice::from_raw_parts(data as *const T, len / std::mem::size_of::<T>())
    }
}
unsafe impl CopyableUnsized for str {
    unsafe fn construct_ptr<'a>(data: *const u8, len: usize) -> &'a Self {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(data, len))
    }
}

#[cfg(unix)]
unsafe impl CopyableUnsized for OsStr {
    unsafe fn construct_ptr<'a>(data: *const u8, len: usize) -> &'a Self {
        OsStr::from_bytes(std::slice::from_raw_parts(data, len))
    }
}

#[cfg(unix)]
unsafe impl CopyableUnsized for Path {
    unsafe fn construct_ptr<'a>(data: *const u8, len: usize) -> &'a Self {
        Path::new(OsStr::from_bytes(std::slice::from_raw_parts(data, len)))
    }
}

unsafe impl CopyableUnsized for CStr {
    unsafe fn construct_ptr<'a>(data: *const u8, len: usize) -> &'a Self {
        CStr::from_bytes_with_nul_unchecked(std::slice::from_raw_parts(data, len))
    }
}

unsafe impl<T: Copy> CopyableUnsized for T {
    unsafe fn construct_ptr<'a>(data: *const u8, len: usize) -> &'a Self {
        debug_assert_eq!(len, std::mem::size_of::<T>());
        &*(data as *const Self)
    }
}

impl<T: CopyableUnsized + ?Sized> Default for List<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: CopyableUnsized + ?Sized> List<T> {
    pub fn new() -> Self {
        unsafe { Self::new_unsafe() }
    }
}
impl<T: ?Sized> List<T> {
    /// # Safety
    /// type must be copyable
    pub unsafe fn new_unsafe() -> Self {
        Self {
            inner: ListSlice {
                align: std::mem::align_of::<usize>(),
                chunks: std::ptr::null_mut(),
                chunks_len: 0,
                _ph: PhantomData::default(),
            },
            chunks_cap: 0,
        }
    }
    pub fn push(&mut self, data: &T) {
        let len = std::mem::size_of_val(data);
        let align = std::mem::align_of_val(data);
        let old_align = self.inner.align;
        self.inner.align = self.inner.align.max(align);
        let usize_len = std::mem::size_of::<usize>().max(align);
        let total_len = usize_len + len;
        let aligned_len = if self.inner.chunks_len % self.inner.align == 0 {
            self.inner.chunks_len
        } else {
            self.inner.chunks_len + self.inner.align - self.inner.chunks_len % self.inner.align
        };
        if old_align != self.inner.align || total_len + aligned_len > self.chunks_cap {
            if self.inner.chunks.is_null() {
                self.inner.chunks = unsafe {
                    std::alloc::alloc(
                        std::alloc::Layout::from_size_align(total_len * 2, self.inner.align).unwrap(),
                    )
                };
                self.chunks_cap = total_len * 2;
            } else if old_align == self.inner.align {
                self.inner.chunks = unsafe {
                    std::alloc::realloc(
                        self.inner.chunks,
                        std::alloc::Layout::from_size_align(self.chunks_cap, self.inner.align).unwrap(),
                        (aligned_len + total_len) * 2,
                    )
                };
                self.chunks_cap = (aligned_len + total_len) * 2;
            } else {
                unsafe {
                    let old_chunks = self.inner.chunks;
                    self.inner.chunks = std::alloc::alloc(
                        std::alloc::Layout::from_size_align(
                            (aligned_len + total_len) * 2,
                            self.inner.align,
                        )
                        .unwrap(),
                    );
                    std::slice::from_raw_parts_mut(
                        self.inner.chunks as *mut MaybeUninit<u8>,
                        self.inner.chunks_len,
                    )
                    .copy_from_slice(std::slice::from_raw_parts(
                        old_chunks as *const MaybeUninit<u8>,
                        self.inner.chunks_len,
                    ));
                    std::alloc::dealloc(
                        old_chunks,
                        std::alloc::Layout::from_size_align(self.chunks_cap, old_align).unwrap(),
                    );
                    self.chunks_cap = (aligned_len + total_len) * 2;
                }
            }
        }
        unsafe {
            (self.inner.chunks.add(aligned_len) as *mut [u8; 8]).write(len.to_ne_bytes());
            std::slice::from_raw_parts_mut(
                self.inner.chunks.add(aligned_len + usize_len) as *mut MaybeUninit<u8>,
                len,
            )
            .copy_from_slice(std::slice::from_raw_parts(
                data as *const T as *const MaybeUninit<u8>,
                len,
            ));
        }
        self.inner.chunks_len = aligned_len + total_len;
    }
    pub fn as_slice(&self) -> &ListSlice<T> {
        &self.inner
    }
}

impl<T: ?Sized> Deref for List<T> {
    type Target = ListSlice<T>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: ?Sized> Drop for List<T> {
    fn drop(&mut self) {
        if !self.inner.chunks.is_null() {
            unsafe { std::alloc::dealloc(self.inner.chunks, std::alloc::Layout::from_size_align(self.chunks_cap, self.inner.align).unwrap()) };
        }
    }
}

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut list = List::<[u16]>::new();
        list.push(&[0, 1]);
        list.push(&[2, 3]);
        assert_eq!(list.head().unwrap(), &[0, 1]);
        assert_eq!(list.tail().unwrap().head().unwrap(), &[2, 3]);
        let mut list = List::<str>::new();
        list.push("testing");
        list.push("testing 2");
        assert_eq!(list.head().unwrap(), "testing");
        assert_eq!(list.tail().unwrap().head().unwrap(), "testing 2");
    }
}
