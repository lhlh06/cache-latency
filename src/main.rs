#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#![warn(clippy::all)]

use bytesize::ByteSize;
use clap::Parser;

use crate::benchmark::{PaddedNode, run_benchmark};

mod benchmark;
mod benchmark_ptr;
mod topo;
mod util;

const DEFAULT_ITERATIONS: usize = 30_000;
const DEFAULT_SAMPLES: usize = 1000;

// const DEFAULT_SIZES: [ByteSize; 12] = [
//     ByteSize(1024 * 16),         // 16 KB  (L1)
//     ByteSize(1024 * 32),         // 32 KB  (L1 boundary)
//     ByteSize(1024 * 64),         // 64 KB  (L2)
//     ByteSize(1024 * 256),        // 256 KB (L2)
//     ByteSize(1024 * 1024),       // 1 MB   (L3)
//     ByteSize(1024 * 1024 * 8),   // 8 MB   (L3)
//     ByteSize(1024 * 1024 * 10),  // 10 MB  (L3)
//     ByteSize(1024 * 1024 * 12),  // 12 MB  (L3)
//     ByteSize(1024 * 1024 * 16),  // 16 MB  (DRAM)
//     ByteSize(1024 * 1024 * 32),  // 32 MB  (DRAM)
//     ByteSize(1024 * 1024 * 128), // 128 MB (DRAM - Deep)
//     ByteSize(1024 * 1024 * 512), // 512 MB (DRAM - Deep)
// ];

const DEFAULT_SIZES: &str = "8kiB,16kiB,32kiB,64kiB,128kiB,256kiB,512kiB,1miB,2miB,4miB,8miB,12miB,16miB,20miB,32miB,64miB,128miB,256miB,512miB,1giB,2giB";

#[derive(clap::Parser)]
pub struct CliArgs {
    /// The number of iterations per sample
    #[clap(default_value_t = DEFAULT_ITERATIONS, value_parser)]
    num_iterations: usize,

    #[clap(default_value_t = DEFAULT_SAMPLES, value_parser)]
    num_samples: usize,

    #[clap(long, value_parser)]
    csv: bool,

    /// Specify the buffer sizes that should be run in benchmark.
    #[clap(short, long, value_delimiter(','), default_value = DEFAULT_SIZES, value_parser)]
    sizes: Vec<ByteSize>,

    /// Specify the core by id that should be used. By default, available core 0 is used.
    #[clap(short, long, value_parser)]
    core: Option<usize>,

    /// Specify the program whether bind local numa if possible.
    #[clap(long, value_parser)]
    numa: bool,
}
pub fn get_cpuid() -> Option<raw_cpuid::CpuId<raw_cpuid::native_cpuid::CpuIdReaderNative>> {
    Some(raw_cpuid::CpuId::default())
}

fn main() {
    let args = CliArgs::parse();
    if let Some(brand) = get_cpuid()
        .and_then(|c| c.get_processor_brand_string())
        .map(|c| c.as_str().to_string())
    {
        println!("CPU: {}", brand);
    }

    println!("Size of PaddedNode: {} bytes", size_of::<PaddedNode>());
    println!("Number of iterations: {}", args.num_iterations);
    println!("Number of samples:    {}", args.num_samples);
    if args.numa {
        println!("With NUMA enabled");
    }
    let cores = core_affinity::get_core_ids().expect("unable to get cores");

    let core = if let Some(core) = args.core {
        *cores
            .iter()
            .find(|c| c.id == core)
            .unwrap_or_else(|| panic!("Core {} not found. Available: {:?}", core, &cores))
    } else {
        cores[0]
    };

    println!("Run on core: {}\n", core.id);
    {
        for size in &args.sizes {
            assert!(
                size.as_u64() <= usize::MAX as u64,
                "Buffer size exceeds max usize limit!"
            );
            run_benchmark(
                size.as_u64() as usize,
                core,
                args.num_iterations,
                args.num_samples,
                &args,
            );
        }
    }

    println!(
        "--------------------------------------------------------------------------------------"
    );
    {
        for size in &args.sizes {
            assert!(
                size.as_u64() <= usize::MAX as u64,
                "Buffer size exceeds max usize limit!"
            );
            benchmark_ptr::run_benchmark(
                size.as_u64() as usize,
                core,
                args.num_iterations,
                args.num_samples,
                &args,
            );
        }
    }
}
