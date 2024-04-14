use std::ptr::NonNull;

use crate::core::*;
use crate::sys;

pub struct FilterView<'a, T>
where
    T: Iterable,
{
    world: WorldRef<'a>,
    filter_ptr: *const FilterT,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T> Clone for FilterView<'a, T>
where
    T: Iterable,
{
    fn clone(&self) -> Self {
        Self {
            world: self.world,
            filter_ptr: self.filter_ptr,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, T> FilterView<'a, T>
where
    T: Iterable,
{
    /// Create a new filter view
    ///
    /// # Arguments
    ///
    /// * `world`: the world to create the filter view from
    /// * `filter`: the filter to create the view from
    ///
    /// # See also
    ///
    /// * C++ API: `filter_view::filter_view`
    #[doc(alias = "filter_view::filter_view")]
    pub fn new(world: impl IntoWorld<'a>, filter: *const FilterT) -> Self {
        Self {
            world: world.world(),
            _phantom: std::marker::PhantomData,
            filter_ptr: filter as *const FilterT,
        }
    }
}

/// Filters are cheaper to create, but slower to iterate than queries.
pub struct Filter<'a, T>
where
    T: Iterable,
{
    world: WorldRef<'a>,
    _phantom: std::marker::PhantomData<T>,
    filter: FilterT,
}

impl<'a, T> Filter<'a, T>
where
    T: Iterable,
{
    /// Create a new filter
    ///
    /// # Arguments
    ///
    /// * `world`: the world to create the filter from
    ///
    /// # See also
    ///
    /// * C++ API: `filter::filter`
    #[doc(alias = "filter::filter")]
    pub fn new(world: impl IntoWorld<'a>) -> Self {
        let mut desc = sys::ecs_filter_desc_t::default();
        T::register_ids_descriptor(world.world_ptr_mut(), &mut desc);
        let mut filter: FilterT = Default::default();
        desc.storage = &mut filter;
        unsafe { sys::ecs_filter_init(world.world_ptr_mut(), &desc) };
        Filter {
            world: world.world(),
            _phantom: std::marker::PhantomData,
            filter,
        }
    }

    /// Wrap an existing raw filter
    ///
    /// # Safety
    /// Caller must ensure the validity of `filter`
    ///
    /// # Arguments
    ///
    /// * `world`: the world to create the filter from
    /// * `filter`: the filter to wrap
    ///
    /// # See also
    ///
    /// * C++ API: `filter::filter`
    #[doc(alias = "filter::filter")]
    pub unsafe fn new_ownership(world: impl IntoWorld<'a>, filter: NonNull<FilterT>) -> Self {
        let mut filter_obj = Filter {
            world: world.world(),
            _phantom: std::marker::PhantomData,
            filter: Default::default(),
        };

        unsafe { sys::ecs_filter_move(&mut filter_obj.filter, filter.as_ptr()) };

        filter_obj
    }

    //TODO: this needs testing -> desc.storage pointer becomes invalid after this call as it re-allocates after this new
    // determine if this is a problem
    /// Create a new filter from a filter descriptor
    ///
    /// # Arguments
    ///
    /// * `world`: the world to create the filter from
    /// * `desc`: the filter descriptor to create the filter from
    ///
    /// # See also
    ///
    /// * C++ API: `filter::filter`
    #[doc(alias = "filter::filter")]
    pub fn new_from_desc(world: impl IntoWorld<'a>, desc: &mut sys::ecs_filter_desc_t) -> Self {
        let mut filter_obj = Filter {
            world: world.world(),
            _phantom: std::marker::PhantomData,
            filter: Default::default(),
        };

        desc.storage = &mut filter_obj.filter;

        unsafe {
            if sys::ecs_filter_init(filter_obj.world.world_ptr_mut(), desc).is_null() {
                sys::ecs_abort_(
                    FlecsErrorCode::InvalidParameter.to_int(),
                    file!().as_ptr() as *const i8,
                    line!() as i32,
                    std::ptr::null(),
                );

                if let Some(abort_func) = sys::ecs_os_api.abort_ {
                    abort_func();
                }
            }

            if !desc.terms_buffer.is_null() {
                if let Some(free_func) = sys::ecs_os_api.free_ {
                    free_func(desc.terms_buffer as *mut _);
                }
            }
        }

        filter_obj
    }
}

impl<'a, T> Drop for Filter<'a, T>
where
    T: Iterable,
{
    fn drop(&mut self) {
        // this is a hack to prevent ecs_filter_fini from freeing the memory of our stack allocated filter
        // we do actually own this filter. ecs_filter_fini is called to free the memory of the terms
        //self.filter.owned = false;
        //TODO the above code, `.owned` got removed in upgrading flecs from 3.2.4 to 3.2.11,
        // so we need to find a new? way to prevent the memory from being freed if it's stack allocated
        unsafe { sys::ecs_filter_fini(&mut self.filter) }
    }
}

impl<'a, T> Clone for Filter<'a, T>
where
    T: Iterable,
{
    fn clone(&self) -> Self {
        let mut new_filter = Filter::<T> {
            world: self.world,
            _phantom: std::marker::PhantomData,
            filter: Default::default(),
        };

        unsafe { sys::ecs_filter_copy(&mut new_filter.filter, &self.filter) };
        new_filter
    }
}

impl<'a, T> IntoWorld<'a> for Filter<'a, T>
where
    T: Iterable,
{
    fn world(&self) -> WorldRef<'a> {
        self.world
    }
}

impl<'a, T> IterOperations for Filter<'a, T>
where
    T: Iterable,
{
    fn retrieve_iter(&self) -> super::IterT {
        unsafe { sys::ecs_filter_iter(self.world.world_ptr_mut(), &self.filter) }
    }

    fn iter_next(&self, iter: &mut super::IterT) -> bool {
        unsafe { sys::ecs_filter_next(iter) }
    }

    fn query_ptr(&self) -> *const QueryT {
        &self.filter
    }

    fn iter_next_func(&self) -> unsafe extern "C" fn(*mut super::IterT) -> bool {
        sys::ecs_filter_next
    }
}

impl<'a, T> IntoWorld<'a> for FilterView<'a, T>
where
    T: Iterable,
{
    fn world(&self) -> WorldRef<'a> {
        self.world
    }
}

impl<'a, T> IterAPI<'a, T> for FilterView<'a, T>
where
    T: Iterable,
{
    fn as_entity(&self) -> EntityView {
        EntityView::new_from(self.world, unsafe {
            sys::ecs_get_entity(self.filter_ptr as *const _)
        })
    }
}

impl<'a, T> IterOperations for FilterView<'a, T>
where
    T: Iterable,
{
    fn retrieve_iter(&self) -> super::IterT {
        unsafe { sys::ecs_filter_iter(self.world.world_ptr_mut(), self.filter_ptr) }
    }

    fn iter_next(&self, iter: &mut super::IterT) -> bool {
        unsafe { sys::ecs_filter_next(iter) }
    }

    fn query_ptr(&self) -> *const QueryT {
        self.filter_ptr
    }

    fn iter_next_func(&self) -> unsafe extern "C" fn(*mut super::IterT) -> bool {
        sys::ecs_filter_next
    }
}

impl<'a, T> IterAPI<'a, T> for Filter<'a, T>
where
    T: Iterable,
{
    fn as_entity(&self) -> EntityView<'a> {
        EntityView::new_from(self.world, unsafe {
            sys::ecs_get_entity(&self.filter as *const _ as *const _)
        })
    }
}
