use std::arch::asm;

const fn valgrind_request_code(a: u8, b: u8, c: u16) -> usize {
    (a as usize) << 24 | (b as usize) << 16 | (c as usize)
}

#[inline(always)]
fn valgrind_request(args: &[usize; 6]) -> usize {
    let mut result = 0_usize;
    unsafe {
        asm!(
            "rol rdi, 3",
            "rol rdi, 13",
            "rol rdi, 61",
            "rol rdi, 51",
            "xchg rbx, rbx",
            in("rax") args,
            inout("rdx") result,
            options(nostack, readonly, preserves_flags)
        );
    }
    result
}

#[inline(always)]
pub(crate) fn running_on_valgrind() -> bool {
    const REQ: usize = 0x1001;
    valgrind_request(&[REQ, 0, 0, 0, 0, 0]) != 0
}

#[inline(always)]
pub(crate) fn start_instrumentation() {
    const REQ: usize = valgrind_request_code(b'C', b'G', 0);
    valgrind_request(&[REQ, 0, 0, 0, 0, 0]);
}

#[inline(always)]
pub(crate) fn stop_instrumentation() {
    const REQ: usize = valgrind_request_code(b'C', b'G', 1);
    valgrind_request(&[REQ, 0, 0, 0, 0, 0]);
}
