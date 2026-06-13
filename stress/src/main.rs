mod client;
mod ratelimit;
mod runner;

use std::sync::Arc;
use std::time::Instant;

use clap::Parser;
use hdrhistogram::Histogram;
use tokio::sync::Semaphore;

use ratelimit::RateGate;
use runner::{FailReason, GameConfig, GameResult, run_game};

#[derive(Parser, Debug, Clone)]
#[command(about = "Staged WebSocket load + state-consistency stress test")]
struct Args {
    /// WebSocket endpoint (use wss://host/ws for the deployed cluster).
    #[arg(long, default_value = "ws://localhost:3000/ws")]
    url: String,

    /// Cap on plies played per game.
    #[arg(long, default_value_t = 30)]
    max_plies: u32,

    /// Per-move wait for a `state` frame (ms).
    #[arg(long, default_value_t = 5000)]
    move_timeout_ms: u64,

    /// Max concurrent connection setups (throttles connect storms).
    #[arg(long, default_value_t = 500)]
    connect_concurrency: usize,

    /// Cap new connections to this many per second (0 = unlimited). Use this to
    /// stay under a load balancer's TLS-handshake limit; unlike
    /// --connect-concurrency it bounds the rate regardless of client speed.
    #[arg(long, default_value_t = 0.0)]
    connect_rate: f64,

    /// A stage fails if the non-consistency failure rate exceeds this.
    #[arg(long, default_value_t = 0.1)]
    fail_threshold: f64,

    /// Safety ceiling for the +100 escalation loop.
    #[arg(long, default_value_t = 200_000)]
    max_games: usize,

    /// RNG seed base; per-game seed = base + game index.
    #[arg(long, default_value_t = 1)]
    seed: u64,

    /// Optional `session` cookie value so games count as signed-in.
    #[arg(long)]
    session_cookie: Option<String>,

    /// Override the stage list, e.g. "1,50,200".
    #[arg(long, value_delimiter = ',')]
    stages: Option<Vec<usize>>,
}

#[derive(Default)]
struct StageOutcome {
    attempted: usize,
    completed: usize,
    connect: usize,
    timeout: usize,
    server: usize,
    consistency: usize,
    desync: usize,
    total_plies: u64,
    first_detail: Option<String>,
    hist: Option<Histogram<u64>>,
    wall_secs: f64,
}

impl StageOutcome {
    fn failures(&self) -> usize {
        self.connect + self.timeout + self.server + self.consistency + self.desync
    }
}

/// Yields the stage sizes: fixed ramp, then +100 forever (bounded by caller).
fn stage_sizes(custom: &Option<Vec<usize>>) -> Vec<usize> {
    if let Some(s) = custom {
        return s.clone();
    }
    let mut v = vec![100, 200, 300, 1000, 5000, 10000];
    let mut n = 10100;
    // Generous upper bound; the driver stops earlier on failure or --max-games.
    while n <= 1_000_000 {
        v.push(n);
        n += 100;
    }
    v
}

async fn run_stage(args: &Args, size: usize, index_base: u64) -> StageOutcome {
    let sem = Arc::new(Semaphore::new(args.connect_concurrency));
    let gate = (args.connect_rate > 0.0).then(|| Arc::new(RateGate::new(args.connect_rate)));
    let mut handles = Vec::with_capacity(size);
    let start = Instant::now();

    for i in 0..size {
        let sem = sem.clone();
        let cfg = GameConfig {
            url: args.url.clone(),
            cookie: args.session_cookie.clone(),
            max_plies: args.max_plies,
            move_timeout_ms: args.move_timeout_ms,
            seed: index_base + i as u64,
            gate: gate.clone(),
        };
        handles.push(tokio::spawn(async move {
            // Hold a permit only across connection setup.
            let permit = sem.acquire_owned().await.unwrap();
            let mut slot = Some(permit);
            run_game(&cfg, move || {
                slot.take();
            })
            .await
        }));
    }

    let mut out = StageOutcome {
        attempted: size,
        ..Default::default()
    };
    let mut hist = Histogram::<u64>::new(3).unwrap();
    for h in handles {
        let r: GameResult = match h.await {
            Ok(r) => r,
            Err(_) => {
                out.desync += 1;
                continue;
            }
        };
        out.total_plies += r.plies as u64;
        for us in &r.move_latencies_us {
            let _ = hist.record(*us);
        }
        if r.ok {
            out.completed += 1;
        } else {
            if out.first_detail.is_none() {
                out.first_detail = r.detail.clone();
            }
            match r.reason {
                Some(FailReason::Connect) => out.connect += 1,
                Some(FailReason::Timeout) => out.timeout += 1,
                Some(FailReason::ServerError) => out.server += 1,
                Some(FailReason::Consistency) => out.consistency += 1,
                Some(FailReason::Desync) => out.desync += 1,
                None => {}
            }
        }
    }
    out.wall_secs = start.elapsed().as_secs_f64();
    out.hist = Some(hist);
    out
}

fn print_stage(size: usize, o: &StageOutcome) {
    let (p50, p95, p99, max) = match &o.hist {
        Some(h) if h.len() > 0 => (
            h.value_at_quantile(0.50),
            h.value_at_quantile(0.95),
            h.value_at_quantile(0.99),
            h.max(),
        ),
        _ => (0, 0, 0, 0),
    };
    let mps = if o.wall_secs > 0.0 {
        o.total_plies as f64 / o.wall_secs
    } else {
        0.0
    };
    println!(
        "stage {size:>7} | ok {ok:>7}/{att:<7} | fail c{c} t{t} s{s} X{x} d{d} | \
         moves {mv:>8} {mps:>8.0}/s | lat us p50 {p50} p95 {p95} p99 {p99} max {max} | {wall:.1}s",
        ok = o.completed,
        att = o.attempted,
        c = o.connect,
        t = o.timeout,
        s = o.server,
        x = o.consistency,
        d = o.desync,
        mv = o.total_plies,
        wall = o.wall_secs,
    );
    if let Some(d) = &o.first_detail {
        println!("            first failure: {d}");
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let rate_label = if args.connect_rate > 0.0 {
        format!("{:.0}/s", args.connect_rate)
    } else {
        "unlimited".to_string()
    };
    println!(
        "target {} | max_plies {} | connect_concurrency {} | connect_rate {} | fail_threshold {}",
        args.url, args.max_plies, args.connect_concurrency, rate_label, args.fail_threshold
    );
    println!("(X = consistency failures — always fatal)\n");

    let sizes = stage_sizes(&args.stages);
    let mut last_ok: Option<usize> = None;
    let mut index_base: u64 = args.seed;

    for size in sizes {
        if size > args.max_games {
            println!("\nreached --max-games {}; stopping.", args.max_games);
            break;
        }
        let o = run_stage(&args, size, index_base).await;
        index_base += size as u64;
        print_stage(size, &o);

        let fail_rate = o.failures() as f64 / o.attempted.max(1) as f64;
        let consistency_broken = o.consistency > 0;
        if consistency_broken || fail_rate > args.fail_threshold {
            println!("\n>>> STAGE {size} FAILED");
            if consistency_broken {
                println!(
                    "    {} consistency violation(s) — state divergence detected.",
                    o.consistency
                );
            }
            if fail_rate > args.fail_threshold {
                println!(
                    "    failure rate {:.2}% > threshold {:.2}%.",
                    fail_rate * 100.0,
                    args.fail_threshold * 100.0
                );
            }
            match last_ok {
                Some(n) => println!(">>> CEILING: last fully-healthy stage = {n} parallel games."),
                None => println!(">>> CEILING: failed at the very first stage."),
            }
            return;
        }
        last_ok = Some(size);
    }

    if let Some(n) = last_ok {
        println!("\nAll stages passed up to {n} parallel games.");
    }
}
