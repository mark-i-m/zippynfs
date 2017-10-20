
extern crate rand;

use std::fs::OpenOptions;
use std::io::Write;
use std::io::{Seek, SeekFrom};
use std::time::Instant;

use rand::{Rng, thread_rng};

fn main() {
    // rand bytes
    let mut buf = [0u8; 1 << 20]; // 1MB
    thread_rng().fill_bytes(&mut buf);

    // open a random file
    let name: usize = thread_rng().gen();
    let name = name.to_string();

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(name)
        .unwrap();

    let mut sum_writes = 0.0;
    let mut sum_time = 0.0;

    loop {
        file.seek(SeekFrom::Start(0)).unwrap();

        let t0 = Instant::now();
        file.write_all(&buf).unwrap();
        file.sync_all().unwrap();
        let d = t0.elapsed();

        sum_writes += (1 << 30) as f64;
        sum_time += (d.as_secs() as f64 * 1E9) + (d.subsec_nanos() as f64);

        println!("Write B/W: {:.*} MB/s", 1, sum_writes / sum_time);
    }
}
