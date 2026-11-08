#[inline]
pub fn get_errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

#[inline]
pub fn is_pow_of_two(val: u32) -> bool {
    if val == 0 {
        return false;
    }
    (val & (val - 1)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_powers_of_two() {
        assert!(!is_pow_of_two(0));
        assert!(is_pow_of_two(1));
        assert!(is_pow_of_two(2));
        assert!(!is_pow_of_two(13));
    }
}
