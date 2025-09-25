use iai::black_box;

fn fibonacci(n: u64) -> u64 {
    match n {
        0 | 1 => 1,
        n => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn bench_empty() {
    return;
}

fn bench_fibonacci() -> u64 {
    fibonacci(black_box(10))
}

fn bench_fibonacci_long() -> u64 {
    fibonacci(black_box(30))
}

fn bench_binary_search() -> usize {
    const LEN: usize = 10 * 1024;
    static LARGE_ARRAY: [u64; LEN] = const {
        let mut array = [0; LEN];
        let mut i = 0;
        while i < LEN {
            array[i] = i as u64;
            i += 1;
        }
        array
    };

    black_box(&LARGE_ARRAY).binary_search(&black_box(123)).expect("number not found")
}

fn bench_binary_search_with_allocation() -> usize {
    const LEN: usize = 10 * 1024;
    let mut vec = Vec::with_capacity(LEN);

    for i in 0..LEN {
        black_box(&mut vec).push(black_box(i));
    }

    black_box(&vec).binary_search(&black_box(123)).expect("number not found")
}

iai::main!(bench_empty, bench_fibonacci, bench_fibonacci_long, bench_binary_search, bench_binary_search_with_allocation);
