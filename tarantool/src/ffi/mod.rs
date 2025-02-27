#[doc(hidden)]
pub mod helper;
#[doc(hidden)]
pub use ::tlua::ffi as lua;
#[doc(hidden)]
pub mod datetime;
#[doc(hidden)]
pub mod decimal;
#[doc(hidden)]
pub mod sql;
#[doc(hidden)]
pub mod tarantool;
#[doc(hidden)]
pub mod uuid;

/// Check whether the current tarantool executable supports decimal api.
/// If this function returns `false` using any of the functions in
/// [`tarantool::decimal`] will result in a **panic**.
///
/// [`tarantool::decimal`]: mod@crate::decimal
pub fn has_decimal() -> bool {
    true
}

/// Check whether the current tarantool executable supports fiber::channel api.
/// If this function returns `false` using any of the functions in
/// [`tarantool::fiber::channel`] will result in a **panic**.
///
/// [`tarantool::fiber::channel`]: crate::fiber::channel
pub fn has_fiber_channel() -> bool {
    unsafe {
        let name = crate::c_str!("fiber_channel_new");
        helper::tnt_internal_symbol::<*const ()>(name).is_some() || helper::has_dyn_symbol(name)
    }
}

/// Check whether the current tarantool executable supports getting tuple fields
/// by json pattern.
/// If this function returns `false` then
/// - passing a string to [`Tuple::try_get`] will always result in an `Error`,
/// - passing a string to [`Tuple::get`] will always result in a **panic**.
///
/// [`Tuple::try_get`]: crate::tuple::Tuple::try_get
/// [`Tuple::get`]: crate::tuple::Tuple::get
pub fn has_tuple_field_by_path() -> bool {
    let c_str = std::ffi::CStr::from_bytes_with_nul_unchecked;
    unsafe {
        helper::has_dyn_symbol(c_str(tarantool::TUPLE_FIELD_BY_PATH_NEW_API.as_bytes()))
            | helper::has_dyn_symbol(c_str(tarantool::TUPLE_FIELD_BY_PATH_OLD_API.as_bytes()))
    }
}

/// Check whether the current tarantool executable supports datetime api.
/// If this function returns `false` using functions in
/// [`tarantool::datetime`] may result in a **panic**.
///
/// [`tarantool::datetime`]: mod@crate::datetime
pub fn has_datetime() -> bool {
    unsafe { helper::has_dyn_symbol(crate::c_str!("tnt_mp_encode_datetime")) }
}
