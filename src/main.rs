#![cfg(target_arch = "x86_64")]
#![warn(clippy::all)]

use crate::pointer_chasing::PaddedNode;
use bytesize::ByteSize;
use clap::Parser;

mod pointer_chasing;
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
    /// The number of iterations per sample.
    #[clap(default_value_t = DEFAULT_ITERATIONS, value_parser = parse_positive_usize)]
    num_iterations: usize,

    /// The number of samples to run.
    #[clap(default_value_t = DEFAULT_SAMPLES, value_parser = parse_positive_usize)]
    num_samples: usize,

    /// Output as csv format in stdout.
    #[clap(long, value_parser)]
    csv: bool,

    /// Specify the buffer sizes that should be run in benchmark.
    #[clap(
        short,
        long,
        value_delimiter(','),
        default_value = DEFAULT_SIZES,
        value_parser = parse_buffer_size
    )]
    sizes: Vec<ByteSize>,

    /// Specify the core by id that should be used. By default, available core 0 is used.
    #[clap(short, long, value_parser)]
    core: Option<usize>,

    /// Subtract the TSC timing overhead from each sample.
    #[clap(long, value_parser)]
    subtract_overhead: bool,
}

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let value = value
        .parse::<usize>()
        .map_err(|_| "value must be a positive integer".to_string())?;

    if value == 0 {
        Err("value must be greater than zero".to_string())
    } else {
        Ok(value)
    }
}

fn parse_buffer_size(value: &str) -> Result<ByteSize, String> {
    let size = value
        .parse::<ByteSize>()
        .map_err(|err| format!("invalid buffer size: {err}"))?;
    let min_size = (2 * size_of::<PaddedNode>()) as u64;

    if size.as_u64() < min_size {
        Err(format!(
            "buffer size must be at least {}",
            ByteSize(min_size)
        ))
    } else if size.as_u64() > usize::MAX as u64 {
        Err("buffer size exceeds usize::MAX".to_string())
    } else {
        Ok(size)
    }
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
        eprintln!("CPU: {}", brand);
    }

    eprintln!("Size of PaddedNode: {} bytes", size_of::<PaddedNode>());
    eprintln!("Number of iterations: {}", args.num_iterations);
    eprintln!("Number of samples:    {}", args.num_samples);
    let cores = core_affinity::get_core_ids().expect("unable to get cores");

    let core = if let Some(core) = args.core {
        *cores
            .iter()
            .find(|c| c.id == core)
            .unwrap_or_else(|| panic!("Core {} not found. Available: {:?}", core, &cores))
    } else {
        cores[0]
    };

    // run pointer-chasing benchmark
    {
        pointer_chasing::run_benchmark(core, &args);
    }
}
