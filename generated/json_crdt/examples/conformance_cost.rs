//! M-E7 of the model plane's validation plan: what the structural check
//! costs per operation, and what it would cost on the receive path.
//!
//! The check runs at the local intake, where the node already consults its
//! guard. It is safe on receive too, since every replica holding one digest
//! reaches one verdict, but it is not free there: a structural check written
//! the way the header guard is written serializes the operation to a
//! `serde_json::Value` and walks it against the schema, on every received
//! frame of every hosted model. This prices that before anyone makes it
//! unconditional; the check stays off the receive path by design, and the
//! "check on" arm below is an experiment arm and not a mode the node has.
//!
//! Per descriptor (`bt` and `uml`) and per N hosted models, the M-E2 points:
//!
//! - **check alone**: `check_structure` over the well-formed operations
//!   from the seeded generator the property test mp32 uses, each timed on
//!   its own with `Instant` so p50 and p99 are real quantiles; the
//!   serialization the receive path would pay is timed beside it.
//! - **receive**: the same operations, spread over the N logs round-robin,
//!   sent by writer replicas and delivered to the real `Node` by hand
//!   through `handle_transport_message`, timed as one wall-clock run with
//!   the check off, then again on a fresh node with the check on, which is
//!   the serialization plus the check before every delivery.
//!
//! The plan asked for 100,000 operations per arm, the count M-E1's workload
//! uses. The first run showed why that is not the number here: applying a
//! JSON CRDT operation costs time linear in the document it lands in (38 µs
//! per delivery at 2,000 operations in one log, 72 µs at 4,000, release),
//! and a writer replica's `send` pays that plus an `is_enabled` query that
//! is dearer still, so 100,000 operations into one log is a quadratic run
//! of hours that measures the CRDT and not the check. The default is 10,000
//! per descriptor, and the writers are rotated every 1,000 operations
//! (`w0`, `w1`, …, each a fresh replica the node learns as a member) so the
//! frame generation stays out of the way; the node under test still holds
//! every operation in one document per log.
//!
//! Thresholds, proposed in the plan and for decision after this run: p50 at
//! most 10 µs and p99 at most 50 µs per operation at the largest N on either
//! descriptor, and the receive run with the check on at most 1.10 × the run
//! with it off. A crossed threshold is printed and turns the exit status
//! non-zero after the CSV is written, so a run is never lost to its verdict.
//!
//! ```text
//! cargo run --release --example conformance_cost -- \
//!     --descriptors ../../examples --out results.csv --points 1,4,16,64 --ops 10000
//! ```

use std::hint::black_box;
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use json_crdt::package::JsonLog;
use moirai_network::HashMap;
use moirai_network::generic::{LogReplica, Node};
use moirai_network::transport::TransportMessage;
use moirai_protocol::log_id::LogId;
use moirai_protocol::replica::IsReplica;
use moirai_protocol::state::log::IsLog;
use serde_json::Value;

#[allow(dead_code)]
mod conformance;
#[allow(dead_code)]
#[path = "../tests/support/conformance_workload.rs"]
mod workload;

use conformance::{Schema, check_structure};
use workload::Generator;

type Op = <JsonLog as IsLog>::Op;

const DEFAULT_POINTS: [usize; 4] = [1, 4, 16, 64];
const DEFAULT_OPS: usize = 10_000;
/// Operations per writer replica before the next one takes over.
const WRITER_CHUNK: usize = 1_000;
const DEFAULT_SEED: u64 = 20_260_903;
const DESCRIPTORS: [(&str, &str); 2] = [("bt", "bt.metamodel.json"), ("uml", "uml.metamodel.json")];
const P50_CEILING: Duration = Duration::from_micros(10);
const P99_CEILING: Duration = Duration::from_micros(50);
const RATIO_CEILING: f64 = 1.10;

struct Args {
    descriptors: PathBuf,
    out: Option<String>,
    points: Vec<usize>,
    ops: usize,
    seed: u64,
    run: u32,
}

fn parse_args() -> Args {
    let mut args = Args {
        descriptors: PathBuf::from("../../examples"),
        out: None,
        points: DEFAULT_POINTS.to_vec(),
        ops: DEFAULT_OPS,
        seed: DEFAULT_SEED,
        run: 1,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let value = it.next();
        match (flag.as_str(), value) {
            ("--descriptors", Some(dir)) => args.descriptors = PathBuf::from(dir),
            ("--out", Some(path)) => args.out = Some(path),
            ("--points", Some(list)) => {
                args.points = list
                    .split(',')
                    .map(|n| n.trim().parse().expect("--points takes integers"))
                    .collect();
            }
            ("--ops", Some(n)) => args.ops = n.parse().expect("--ops takes an integer"),
            ("--seed", Some(n)) => args.seed = n.parse().expect("--seed takes an integer"),
            ("--run", Some(n)) => args.run = n.parse().expect("--run takes an integer"),
            (flag, _) => {
                eprintln!(
                    "usage: conformance_cost [--descriptors DIR] [--out FILE] [--points 1,4,16,64] \
                     [--ops N] [--seed N] [--run N]"
                );
                panic!("unknown or incomplete argument `{flag}`");
            }
        }
    }
    args
}

/// A port nothing else is listening on right now.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind a loopback port")
        .local_addr()
        .expect("local addr")
        .port()
}

/// The `i`th hosted log's id: deterministic, so a run is reproducible.
fn hosted_id(i: usize) -> LogId {
    let mut bytes = [0u8; 16];
    bytes[0] = (i >> 8) as u8;
    bytes[1] = i as u8;
    LogId::from_bytes(bytes)
}

/// A node hosting `n` logs over the real transport on a free port with no
/// peers; the writers are learned from their frames, as a peer's are.
fn node_hosting(n: usize) -> Node<JsonLog> {
    let mut node = Node::<JsonLog>::new_with_log_id(
        "node".to_string(),
        &["node"],
        free_port(),
        HashMap::default(),
        hosted_id(0),
    );
    for i in 1..n {
        node.host_log(hosted_id(i)).expect("a fresh id");
    }
    node
}

#[derive(Debug, Clone, Copy)]
struct Quantiles {
    p50: Duration,
    p99: Duration,
    mean: Duration,
}

/// Time `count` calls of `step`, one by one.
fn timed<T, F: FnMut(usize) -> T>(count: usize, mut step: F) -> Quantiles {
    let mut samples: Vec<Duration> = Vec::with_capacity(count);
    for i in 0..count {
        let started = Instant::now();
        let out = step(i);
        let elapsed = started.elapsed();
        black_box(out);
        samples.push(elapsed);
    }
    samples.sort_unstable();
    let total: Duration = samples.iter().sum();
    Quantiles {
        p50: samples[count / 2],
        p99: samples[(count * 99) / 100],
        mean: total / count as u32,
    }
}

struct Point {
    descriptor: &'static str,
    n: usize,
    check: Quantiles,
    serialize: Quantiles,
    timer: Quantiles,
    receive_off: Duration,
    receive_on: Duration,
}

/// The frames writer replicas produce for `ops`, round-robin over `n` logs,
/// each writer applying the operation locally as a peer would before
/// broadcasting it. A fresh set of writers every [`WRITER_CHUNK`]
/// operations, named by chunk, so no writer's own document grows past it.
fn frames_for(n: usize, ops: &[Op]) -> Vec<TransportMessage<Op>> {
    let mut frames = Vec::with_capacity(ops.len());
    for (chunk, ops) in ops.chunks(WRITER_CHUNK).enumerate() {
        let id = format!("w{chunk}");
        let mut writers: Vec<LogReplica<JsonLog>> = (0..n)
            .map(|i| IsReplica::bootstrap_with_log_id(id.clone(), &[&id], hosted_id(i)))
            .collect();
        for (i, op) in ops.iter().enumerate() {
            let event = writers[i % n].send(op.clone()).unwrap_or_else(|| {
                panic!("the writer did not accept a well-formed operation: {op:?}")
            });
            frames.push(TransportMessage::Event { event });
        }
    }
    frames
}

fn delivered(node: &Node<JsonLog>, n: usize) -> usize {
    (0..n)
        .map(|i| {
            node.hosted(&hosted_id(i))
                .expect("hosted")
                .stability()
                .delivered
        })
        .sum()
}

fn measure(
    descriptor: &'static str,
    schema: &Schema,
    n: usize,
    values: &[Value],
    ops: &[Op],
) -> Point {
    let count = values.len();
    // Warm once so the first sample is not a page fault.
    for value in values.iter().take(64) {
        black_box(check_structure(schema, value).is_ok());
    }
    let check = timed(count, |i| {
        check_structure(schema, &values[i]).unwrap_or_else(|why| {
            panic!("a well-formed operation was refused: {why}: {}", values[i])
        })
    });
    let serialize = timed(count, |i| {
        serde_json::to_value(&ops[i]).expect("serializes")
    });
    let timer = timed(count, |_| ());

    let generating = Instant::now();
    let frames = frames_for(n, ops);
    eprintln!(
        "{descriptor} N={n}: {} frames generated in {:?}",
        frames.len(),
        generating.elapsed()
    );
    let mut node = node_hosting(n);
    let started = Instant::now();
    for frame in frames {
        node.handle_transport_message("writer".to_string(), frame);
    }
    let receive_off = started.elapsed();
    assert_eq!(
        delivered(&node, n),
        count,
        "the check-off run did not deliver every frame"
    );
    drop(node);

    let frames = frames_for(n, ops);
    let mut node = node_hosting(n);
    let started = Instant::now();
    for frame in frames {
        if let TransportMessage::Event { event } = &frame {
            let value = serde_json::to_value(event.event().op()).expect("serializes");
            if let Err(why) = check_structure(schema, &value) {
                panic!("the receive-side check refused a well-formed operation: {why}");
            }
        }
        node.handle_transport_message("writer".to_string(), frame);
    }
    let receive_on = started.elapsed();
    assert_eq!(
        delivered(&node, n),
        count,
        "the check-on run did not deliver every frame"
    );

    Point {
        descriptor,
        n,
        check,
        serialize,
        timer,
        receive_off,
        receive_on,
    }
}

fn main() {
    let args = parse_args();
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let mut rows = vec![
        "run,n_logs,descriptor,ops,check_p50_ns,check_p99_ns,receive_ms_check_on,receive_ms_check_off,\
         profile,check_mean_ns,serialize_p50_ns,serialize_p99_ns,timer_p50_ns,ratio"
            .to_string(),
    ];
    let mut crossed = 0;
    let mut verdict = |met: bool, what: &str, detail: String| {
        eprintln!("{}: {what}: {detail}", if met { "met" } else { "CROSSED" });
        if !met {
            crossed += 1;
        }
    };

    for (name, file) in DESCRIPTORS {
        let text = std::fs::read_to_string(args.descriptors.join(file))
            .unwrap_or_else(|e| panic!("read {file} in {}: {e}", args.descriptors.display()));
        let schema = Schema::parse(&text).expect("a schema");
        let mut generator = Generator::new(&schema, args.seed, true);
        let values: Vec<Value> = (0..args.ops).map(|_| generator.conforming()).collect();
        let ops: Vec<Op> = values
            .iter()
            .map(|value| {
                serde_json::from_value(value.clone())
                    .unwrap_or_else(|e| panic!("{value} is not an operation: {e}"))
            })
            .collect();
        eprintln!(
            "{name}: {} operations generated from seed {}",
            ops.len(),
            args.seed
        );

        let largest = args.points.iter().copied().max().unwrap_or(1);
        for &n in &args.points {
            let point = measure(name, &schema, n, &values, &ops);
            let ratio = point.receive_on.as_secs_f64() / point.receive_off.as_secs_f64();
            eprintln!(
                "{name} N={n}: check p50 {:?} p99 {:?} mean {:?}; serialize p50 {:?}; receive off {:?} on {:?} (x{ratio:.3})",
                point.check.p50,
                point.check.p99,
                point.check.mean,
                point.serialize.p50,
                point.receive_off,
                point.receive_on
            );
            rows.push(format!(
                "{},{},{},{},{},{},{:.3},{:.3},{profile},{},{},{},{},{ratio:.4}",
                args.run,
                n,
                point.descriptor,
                args.ops,
                point.check.p50.as_nanos(),
                point.check.p99.as_nanos(),
                point.receive_on.as_secs_f64() * 1000.0,
                point.receive_off.as_secs_f64() * 1000.0,
                point.check.mean.as_nanos(),
                point.serialize.p50.as_nanos(),
                point.serialize.p99.as_nanos(),
                point.timer.p50.as_nanos(),
            ));
            if n == largest {
                verdict(
                    point.check.p50 <= P50_CEILING,
                    &format!("{name} check p50 <= 10 us at N={n}"),
                    format!("{:?}", point.check.p50),
                );
                verdict(
                    point.check.p99 <= P99_CEILING,
                    &format!("{name} check p99 <= 50 us at N={n}"),
                    format!("{:?}", point.check.p99),
                );
            }
            verdict(
                ratio <= RATIO_CEILING,
                &format!("{name} receive with the check on <= 1.10 x off at N={n}"),
                format!(
                    "{ratio:.3}x ({:?} vs {:?})",
                    point.receive_on, point.receive_off
                ),
            );
        }
    }

    let csv = rows.join("\n") + "\n";
    match &args.out {
        Some(path) => std::fs::write(path, csv).unwrap_or_else(|e| panic!("write {path}: {e}")),
        None => print!("{csv}"),
    }
    if crossed > 0 {
        eprintln!("{crossed} threshold(s) crossed");
        std::process::exit(1);
    }
}
