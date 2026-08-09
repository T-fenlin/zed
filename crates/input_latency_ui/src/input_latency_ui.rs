use collections::HashMap;
use gpui::{App, Global, InputLatencySnapshot, Window, WindowId, actions};
use hdrhistogram::Histogram;
use std::time::Instant;

actions!(
    dev,
    [
        /// Opens a buffer showing the input-to-frame latency histogram for the current window.
        DumpInputLatencyHistogram,
    ]
);

/// The data needed to format an input-latency report, captured synchronously
/// by [`snapshot_input_latency_report`]. Formatting via
/// [`format_input_latency_report`] is pure computation and can happen on a
/// background thread.
pub struct InputLatencyReportData {
    snapshot: InputLatencySnapshot,
    previous_snapshot: Option<InputLatencySnapshot>,
    previous_timestamp: Option<chrono::DateTime<chrono::Local>>,
    timestamp: chrono::DateTime<chrono::Local>,
    /// The collab username of the user whose machine produced the data.
    /// Set when the report will be placed in a buffer visible to other
    /// collaborators, so it's clear whose latency was measured.
    reported_by: Option<String>,
}

/// Captures the input-to-frame latency histogram for the given window, along
/// with the previous report's baseline (tracked via a global on the `App`) for
/// delta computation. Advances the baseline to this snapshot.
pub fn snapshot_input_latency_report(
    window: &Window,
    reported_by: Option<String>,
    cx: &mut App,
) -> InputLatencyReportData {
    let snapshot = window.input_latency_snapshot();
    let timestamp = chrono::Local::now();
    let state = cx.default_global::<ReporterState>();

    let data = InputLatencyReportData {
        snapshot: snapshot.clone(),
        previous_snapshot: state.previous_snapshot.take(),
        previous_timestamp: state.previous_timestamp.take(),
        timestamp,
        reported_by,
    };

    state.previous_snapshot = Some(snapshot);
    state.previous_timestamp = Some(timestamp);

    data
}

/// Generates a formatted text report from a previously captured
/// [`InputLatencyReportData`]. If the capture included a previous snapshot,
/// includes a delta section showing changes since that report.
pub fn format_input_latency_report(data: &InputLatencyReportData) -> String {
    format_report(data)
}

#[derive(Default)]
struct ReporterState {
    previous_snapshot: Option<InputLatencySnapshot>,
    previous_timestamp: Option<chrono::DateTime<chrono::Local>>,
}

impl Global for ReporterState {}

/// Per-window state used for telemetry delta computation. Kept separate from
/// `ReporterState` so the user-facing dump and the background telemetry flush
/// maintain independent baselines.
#[derive(Default)]
struct TelemetryReporterState {
    /// Keyed by window id. Each entry holds the cumulative snapshot at the time
    /// of the last telemetry flush, plus the wall-clock time of that flush.
    previous: HashMap<WindowId, (Instant, InputLatencySnapshot)>,
}

impl Global for TelemetryReporterState {}

/// Nanosecond boundaries for the time-range buckets used in telemetry.
/// These match the display distribution in format_report so the two stay in sync.
const MS4_NS: u64 = 4_000_000;
const MS8_NS: u64 = 8_000_000;
const MS16_NS: u64 = 16_000_000;
const MS33_NS: u64 = 33_000_000;
const MS100_NS: u64 = 100_000_000;

/// Minimum number of frames that must be present in the delta window for the
/// telemetry report to be sent. Avoids sending noise for windows that are
/// mostly idle.
const MIN_FRAMES_TO_REPORT: u64 = 5_000;

/// Computes and sends a `input_latency_report` telemetry event for the given
/// window if enough frames have been recorded since the last report.
///
/// Call this periodically (e.g. every five minutes) from a spawned task. A
/// separate baseline snapshot is kept per window so user-facing histogram dumps
/// and telemetry never share state.
pub fn report_input_latency_telemetry(window: &Window, cx: &mut App) {
    let current = window.input_latency_snapshot();
    let window_id = window.window_handle().window_id();

    let state = cx.default_global::<TelemetryReporterState>();
    let now = Instant::now();

    let (delta_latency, delta_coalesce, report_window_seconds) =
        if let Some((prev_instant, prev_snapshot)) = state.previous.get(&window_id) {
            let mut delta_latency = current.latency_histogram.clone();
            delta_latency
                .subtract(&prev_snapshot.latency_histogram)
                .ok();
            let mut delta_coalesce = current.events_per_frame_histogram.clone();
            delta_coalesce
                .subtract(&prev_snapshot.events_per_frame_histogram)
                .ok();
            let elapsed = now.duration_since(*prev_instant).as_secs();
            (delta_latency, delta_coalesce, elapsed)
        } else {
            // First report for this window: the full cumulative histogram is the
            // delta from the empty starting state. We don't know how long the
            // window has been open, so record 0 to signal that this is the
            // initial accumulation period rather than a fixed-width window.
            (
                current.latency_histogram.clone(),
                current.events_per_frame_histogram.clone(),
                0u64,
            )
        };

    let total_frames = delta_latency.len();
    if total_frames < MIN_FRAMES_TO_REPORT {
        return;
    }

    state.previous.insert(window_id, (now, current));

    let frames_sub4 = count_frames_in_range(&delta_latency, 0, MS4_NS);
    let frames_4to8 = count_frames_in_range(&delta_latency, MS4_NS, MS8_NS);
    let frames_8to16 = count_frames_in_range(&delta_latency, MS8_NS, MS16_NS);
    let frames_16to33 = count_frames_in_range(&delta_latency, MS16_NS, MS33_NS);
    let frames_33to100 = count_frames_in_range(&delta_latency, MS33_NS, MS100_NS);
    // frames > 100 ms are implicitly total_frames - (sub4 + 4to8 + 8to16 + 16to33 + 33to100)

    let frames_with_1_event = count_frames_in_range(&delta_coalesce, 1, 2);
    let frames_with_2_events = count_frames_in_range(&delta_coalesce, 2, 3);
    let frames_with_3_events = count_frames_in_range(&delta_coalesce, 3, 4);
    // frames with 4+ events are implicitly total_frames - (1 + 2 + 3)

    telemetry::event!(
        "延迟报告",
        frames_sub4 = frames_sub4,
        frames_4to8 = frames_4to8,
        frames_8to16 = frames_8to16,
        frames_16to33 = frames_16to33,
        frames_33to100 = frames_33to100,
        total_frames = total_frames,
        frames_with_1_event = frames_with_1_event,
        frames_with_2_events = frames_with_2_events,
        frames_with_3_events = frames_with_3_events,
        report_window_seconds = report_window_seconds,
    );
}

fn count_frames_in_range(histogram: &Histogram<u64>, low_ns: u64, high_ns: u64) -> u64 {
    histogram
        .iter_recorded()
        .filter(|v| v.value_iterated_to() >= low_ns && v.value_iterated_to() < high_ns)
        .map(|v| v.count_at_value())
        .sum()
}

fn format_report(data: &InputLatencyReportData) -> String {
    let snapshot = &data.snapshot;
    let histogram = &snapshot.latency_histogram;
    let total = histogram.len();

    if total == 0 {
        return "尚未记录输入延迟样本。\n\n请先在缓冲区中键入或点击。".to_string();
    }

    let percentiles: &[(&str, f64)] = &[
        ("min  ", 0.0),
        ("p50  ", 0.50),
        ("p75  ", 0.75),
        ("p90  ", 0.90),
        ("p95  ", 0.95),
        ("p99  ", 0.99),
        ("p99.9", 0.999),
        ("max  ", 1.0),
    ];

    let now = data.timestamp;

    let mut report = String::new();
    report.push_str("输入延迟直方图\n");
    report.push_str("=======================\n");

    let timestamp = now.format("%Y-%m-%d %H:%M:%S %Z");
    report.push_str(&format!("时间戳: {timestamp}\n"));
    if let Some(reported_by) = &data.reported_by {
        report.push_str(&format!("报告来自 {reported_by} 的机器\n"));
    }
    report.push_str(&format!("样本数: {total}\n"));
    if snapshot.mid_draw_events_dropped > 0 {
        report.push_str(&format!(
            "排除的绘制中事件: {}\n",
            snapshot.mid_draw_events_dropped
        ));
    }

    write_latency_percentiles(&mut report, "百分位数", histogram, percentiles);
    write_latency_distribution(&mut report, "分布", histogram);

    let coalesce = &snapshot.events_per_frame_histogram;
    let coalesce_total = coalesce.len();
    if coalesce_total > 0 {
        report.push('\n');
        report.push_str("每帧合并的事件数:\n");
        for (label, quantile) in percentiles {
            let value = if *quantile == 0.0 {
                coalesce.min()
            } else if *quantile == 1.0 {
                coalesce.max()
            } else {
                coalesce.value_at_quantile(*quantile)
            };
            report.push_str(&format!("  {label}: {value:>6} 个事件\n"));
        }

        report.push('\n');
        report.push_str("分布:\n");
        let bar_width = 30usize;
        let max_count = coalesce.max();
        for n in 1..=max_count {
            let count = coalesce
                .iter_recorded()
                .filter(|value| value.value_iterated_to() == n)
                .map(|value| value.count_at_value())
                .sum::<u64>();
            if count == 0 {
                continue;
            }
            let fraction = count as f64 / coalesce_total as f64;
            let bar_len = (fraction * bar_width as f64) as usize;
            let bar = "\u{2588}".repeat(bar_len);
            report.push_str(&format!(
                "  {n:>6} 个事件: {count:>6} ({:>5.1}%) {bar}\n",
                fraction * 100.0,
            ));
        }
    }

    // Delta section: compare against the previous report's snapshot.
    if let (Some(prev_snapshot), Some(prev_timestamp)) =
        (&data.previous_snapshot, &data.previous_timestamp)
    {
        let prev_latency = &prev_snapshot.latency_histogram;
        let prev_total = prev_latency.len();
        let delta_total = total - prev_total;

        report.push('\n');
        report.push_str("自上次报告以来的变化\n");
        report.push_str("-----------------------\n");
        let prev_ts = prev_timestamp.format("%Y-%m-%d %H:%M:%S %Z");
        let elapsed_secs = (now - *prev_timestamp).num_seconds().max(0);
        report.push_str(&format!(
            "上次报告: {prev_ts} ({elapsed_secs}秒前)\n"
        ));
        report.push_str(&format!("新样本数: {delta_total}\n"));

        if delta_total > 0 {
            let mut delta_histogram = histogram.clone();
            delta_histogram.subtract(prev_latency).ok();

            write_latency_percentiles(
                &mut report,
                "百分位数(仅新样本)",
                &delta_histogram,
                percentiles,
            );
            write_latency_distribution(
                &mut report,
                "分布(仅新样本)",
                &delta_histogram,
            );
        }
    }

    report
}

fn write_latency_percentiles(
    report: &mut String,
    heading: &str,
    histogram: &Histogram<u64>,
    percentiles: &[(&str, f64)],
) {
    let ns_to_ms = |ns: u64| ns as f64 / 1_000_000.0;

    report.push('\n');
    report.push_str(heading);
    report.push_str(":\n");
    for (label, quantile) in percentiles {
        let value_ns = if *quantile == 0.0 {
            histogram.min()
        } else if *quantile == 1.0 {
            histogram.max()
        } else {
            histogram.value_at_quantile(*quantile)
        };
        let hz = if value_ns > 0 {
            1_000_000_000.0 / value_ns as f64
        } else {
            f64::INFINITY
        };
        report.push_str(&format!(
            "  {label}: {:>8.2}毫秒  ({:>7.1} 赫兹)\n",
            ns_to_ms(value_ns),
            hz
        ));
    }
}

fn write_latency_distribution(report: &mut String, heading: &str, histogram: &Histogram<u64>) {
    const BUCKETS: &[(u64, u64, &str, &str)] = &[
        (0, 4_000_000, "0\u{2013}4毫秒", "(优秀)"),
        (4_000_000, 8_000_000, "4\u{2013}8毫秒", "(120帧/秒)"),
        (8_000_000, 16_000_000, "8\u{2013}16毫秒", "(60帧/秒)"),
        (16_000_000, 33_000_000, "16\u{2013}33毫秒", "(30帧/秒)"),
        (33_000_000, 100_000_000, "33\u{2013}100毫秒", ""),
        (100_000_000, u64::MAX, "100毫秒以上", "(卡顿)"),
    ];
    let bar_width = 30usize;
    let total = histogram.len() as f64;

    report.push('\n');
    report.push_str(heading);
    report.push_str(":\n");
    for (low, high, range, note) in BUCKETS {
        let count: u64 = histogram
            .iter_recorded()
            .filter(|value| value.value_iterated_to() >= *low && value.value_iterated_to() < *high)
            .map(|value| value.count_at_value())
            .sum();
        let fraction = if total > 0.0 {
            count as f64 / total
        } else {
            0.0
        };
        let bar_len = (fraction * bar_width as f64) as usize;
        let bar = "\u{2588}".repeat(bar_len);
        report.push_str(&format!(
            "  {range:>8}  {note:<11}: {count:>6} ({:>5.1}%) {bar}\n",
            fraction * 100.0,
        ));
    }
}
