//! Local patched copy of `typeid` without a build script.
//!
//! The upstream crate uses a build script only to set compiler cfg metadata.
//! This project patches it locally because Windows locks that build script in
//! the sandboxed build environment used during development.

#![no_std]
#![allow(clippy::doc_markdown, clippy::inline_always)]

extern crate self as typeid;

use core::any::TypeId;
use core::cmp::Ordering;
use core::fmt::{self, Debug};
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::mem;

/// Provides a const-constructible wrapper around `TypeId`.
#[derive(Copy, Clone)]
pub struct ConstTypeId {
    type_id_fn: fn() -> TypeId,
}

impl ConstTypeId {
    /// Creates a const wrapper that resolves to the `TypeId` of `T`.
    #[must_use]
    pub const fn of<T>() -> Self
    where
        T: ?Sized,
    {
        ConstTypeId {
            type_id_fn: typeid::of::<T>,
        }
    }

    /// Resolves the stored type-id function.
    #[inline]
    fn get(self) -> TypeId {
        (self.type_id_fn)()
    }
}

impl Debug for ConstTypeId {
    /// Formats the resolved `TypeId` for diagnostics.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(&self.get(), formatter)
    }
}

impl PartialEq for ConstTypeId {
    /// Compares two const type ids by their resolved runtime values.
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl PartialEq<TypeId> for ConstTypeId {
    /// Compares this wrapper with a standard-library `TypeId`.
    fn eq(&self, other: &TypeId) -> bool {
        self.get() == *other
    }
}

impl Eq for ConstTypeId {}

impl PartialOrd for ConstTypeId {
    /// Compares two const type ids using the standard `TypeId` ordering.
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for ConstTypeId {
    /// Orders two const type ids by their resolved runtime values.
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(&self.get(), &other.get())
    }
}

impl Hash for ConstTypeId {
    /// Hashes the resolved `TypeId`.
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.get().hash(state);
    }
}

/// Returns a `TypeId` for `T`, matching the upstream crate's non-static support.
#[must_use]
#[inline(always)]
pub fn of<T>() -> TypeId
where
    T: ?Sized,
{
    trait NonStaticAny {
        fn get_type_id(&self) -> TypeId
        where
            Self: 'static;
    }

    impl<T: ?Sized> NonStaticAny for PhantomData<T> {
        /// Returns the type id after lifetime erasure by the caller.
        #[inline(always)]
        fn get_type_id(&self) -> TypeId
        where
            Self: 'static,
        {
            TypeId::of::<T>()
        }
    }

    let phantom_data = PhantomData::<T>;
    NonStaticAny::get_type_id(unsafe {
        mem::transmute::<&dyn NonStaticAny, &(dyn NonStaticAny + 'static)>(&phantom_data)
    })
}
