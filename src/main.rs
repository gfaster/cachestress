use std::{
    ptr,
    time::{Duration, Instant},
};

use clap::{Parser, ValueEnum};

const DEFAULT_SIZE: usize = 100_000_000;
const DEFAULT_SAMPLES: u64 = 100_000_000;
const DEFAULT_STRIDE: usize = 4096;

// const PAGE_SZ: usize = 4096;
const PAGE_SZ: usize = 1 << 21;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short = 'c', long, default_value_t = DEFAULT_SIZE)]
    size: usize,
    #[arg(short = 't', long, default_value_t = DEFAULT_SAMPLES)]
    samples: u64,
    #[arg(short, long, default_value_t = DEFAULT_STRIDE)]
    stride: usize,

    #[arg(short, long, value_enum, default_value_t = Advice::None)]
    advice: Advice,

    #[arg(short, long)]
    quiet: bool,
}

#[derive(Debug, ValueEnum, Clone, Copy, PartialEq, Eq)]
enum Advice {
    None,
    Huge,
}

fn main() {
    // t();
    let args = Args::parse();
    // println!("running: {args:?}");
    let elapsed = stress(&args);
    if !args.quiet {
        println!(
            "{} accesses done in {}",
            args.samples as usize,
            pretty_time(elapsed)
        );
    }
}

// fn t() {
//     let mut rlim: libc::rlimit = unsafe { std::mem::zeroed() };
//     let res = unsafe { libc::getrlimit(libc::RLIMIT_AS, &mut rlim) };
//     assert!(res == 0, "getrlimit: {:?}", std::io::Error::last_os_error());
//     // eprintln!("{rlim:#x?}");
//     let base = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS;
//     let pg = libc::MAP_HUGETLB | libc::MAP_HUGE_2MB;
//     let res = unsafe {
//         libc::mmap(ptr::null_mut(), 1 << 30, libc::PROT_READ | libc::PROT_WRITE, base | pg, -1, 0)
//     };
//     assert!(
//         res != usize::MAX as *mut _,
//         "mmap: {:}",
//         std::io::Error::last_os_error()
//     );
//     panic!("success");
// }

fn stress(args: &Args) -> Duration {
    let v = Mmap::new(args.size.next_multiple_of(PAGE_SZ), args.advice);
    let start = Instant::now();

    let stride = args.stride % args.size;

    let mut i = 0;
    for _ in 0..args.samples {
        let _ = unsafe { ptr::read_volatile(&v[i]) };
        i += stride;
        if i >= args.size {
            i -= args.size;
        }
    }

    start.elapsed()
}

fn pretty_time(d: Duration) -> String {
    if d.as_secs() >= 10 {
        format!("{:.1} s", d.as_secs_f64())
    } else if d.as_secs() >= 1 {
        format!("{:.2} s", d.as_secs_f64())
    } else if d.as_millis() >= 10 {
        format!("{:} ms", d.as_millis())
    } else if d.as_micros() >= 100 {
        format!("{:} μs", d.as_micros())
    } else {
        format!("{:} ns", d.as_nanos())
    }
}

struct Mmap {
    start: *mut u8,
    len: usize,
}

impl Mmap {
    fn new(len: usize, advice: Advice) -> Self {
        let map_len = len.next_multiple_of(PAGE_SZ);
        let flag = match advice {
            Advice::None => 0,
            Advice::Huge => libc::MAP_HUGETLB | libc::MAP_HUGE_2MB,
        };
        // eprintln!("{map_len:x}");
        let start = unsafe {
            libc::mmap(
                ptr::null_mut(),
                map_len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_POPULATE | flag,
                -1,
                0,
            )
        };
        assert!(
            start != usize::MAX as *mut _,
            "mmap failed: {:?}{diagnostic}",
            std::io::Error::last_os_error(),
            diagnostic = if advice == Advice::Huge { "\n(what's the value of /proc/sys/vm/nr_hugepages?)" } else {""}
        );
        match advice {
            Advice::None => {
                unsafe { madvise(start.cast(), map_len, libc::MADV_NOHUGEPAGE) }
            },
            Advice::Huge => {
                unsafe { madvise(start.cast(), map_len, libc::MADV_HUGEPAGE) }
                // unsafe { madvise(start.cast(), map_len, libc::MADV_COLLAPSE) }
            },
        };
        Mmap {
            start: start.cast(),
            len,
        }
    }
}

unsafe fn madvise(addr: *mut u8, len: usize, flag: libc::c_int) {
    let res = unsafe { libc::madvise(addr.cast(), len, flag) };
    assert!(
        res == 0,
        "madvise failed: {:?}",
        std::io::Error::last_os_error()
    );
}

impl Drop for Mmap {
    fn drop(&mut self) {
        let res = unsafe { libc::munmap(self.start.cast(), self.len.next_multiple_of(PAGE_SZ)) };
        assert!(
            res == 0,
            "munmap failed: {:?}",
            std::io::Error::last_os_error()
        );
    }
}

impl std::ops::Deref for Mmap {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { std::slice::from_raw_parts(self.start, self.len) }
    }
}
