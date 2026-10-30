//! How the pitch canvas shows a released result to a person (APP-092
//! polish): exact integers compact, in bytes where the column's name says
//! bytes (money is never inferred from a name: it is formatted from the
//! column's declared type, which the island receives as `column_types`;
//! pivot/fix-093), readable UTC times, and a static inline
//! SVG chart drawn from the released rows, so a page without JavaScript
//! shows the chart too. Nothing here computes a new number from the data:
//! each shown figure is one released value, rounded for reading, with the
//! exact value beside it (in the element's `title`, the table and the
//! insight's exact line). Mirrors `admin/src/canvas/display.ts` and
//! `chart.ts`, which draw the same thing for results the island receives.
use super::page;
use forma_ir::PageProps;
use serde::Serialize;

/// The unit an integer column's name implies (bytes only; never money).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unit {
    Bytes,
    Plain,
}

pub fn unit_of(column: &str) -> Unit {
    if column.to_ascii_lowercase().contains("bytes") {
        Unit::Bytes
    } else {
        Unit::Plain
    }
}

/// `total_bytes` as `Total bytes`.
pub fn label(id: &str) -> String {
    let spaced = id.replace('_', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// An exact integer's sign and magnitude, from its text with or without
/// digit grouping; `None` for anything else (an unknown cell stays text).
pub fn integer(text: &str) -> Option<(bool, u128)> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let digits: String = digits.chars().filter(|c| *c != ',').collect();
    if digits.is_empty() || digits.len() > 36 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u128>().ok().map(|n| (negative && n > 0, n))
}

pub fn group(n: u128) -> String {
    page::display(&n.to_string())
}

/// `q` scaled by `10^decimals`, with trailing zeros dropped.
fn decimal(q: u128, decimals: u32) -> String {
    if decimals == 0 {
        return q.to_string();
    }
    let scale = 10u128.pow(decimals);
    let fraction = format!("{:0width$}", q % scale, width = decimals as usize);
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        (q / scale).to_string()
    } else {
        format!("{}.{fraction}", q / scale)
    }
}

/// `n / den` to three significant digits (half up), as `(q, decimals)`.
fn three(n: u128, den: u128) -> (u128, u32) {
    let whole = n / den;
    let decimals = if whole >= 100 {
        0
    } else if whole >= 10 {
        1
    } else {
        2
    };
    let scaled = n * 10u128.pow(decimals);
    ((scaled * 2 + den) / (2 * den), decimals)
}

/// `n` in steps of 1000 from `start`, as text plus the step's suffix.
fn compact(n: u128, start: u128, suffixes: &[&str]) -> (String, usize) {
    let mut k = 0;
    while k + 1 < suffixes.len() && start * 1000u128.pow(k as u32 + 1) <= n {
        k += 1;
    }
    let (mut q, mut d) = three(n, start * 1000u128.pow(k as u32));
    if q >= 1000 * 10u128.pow(d) && k + 1 < suffixes.len() {
        k += 1;
        (q, d) = three(n, start * 1000u128.pow(k as u32));
    }
    (decimal(q, d), k)
}

/// An integer of `unit` for reading, and its exact form.
pub fn amount(unit: Unit, negative: bool, n: u128) -> (String, String) {
    let sign = if negative { "-" } else { "" };
    match unit {
        Unit::Bytes => {
            const UNITS: [&str; 7] = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
            let short = if n < 1000 {
                format!("{n} B")
            } else {
                let (text, k) = compact(n, 1, &UNITS);
                format!("{text} {}", UNITS[k])
            };
            (
                format!("{sign}{short}"),
                format!(
                    "{sign}{} {}",
                    group(n),
                    if n == 1 { "byte" } else { "bytes" }
                ),
            )
        }
        Unit::Plain => {
            const SUFFIX: [&str; 5] = ["", "K", "M", "B", "T"];
            let short = if n < 10_000 {
                group(n)
            } else {
                let (text, k) = compact(n, 1, &SUFFIX);
                format!("{text}{}", SUFFIX[k])
            };
            (format!("{sign}{short}"), format!("{sign}{}", group(n)))
        }
    }
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// A UTC timestamp's parts: `YYYY-MM-DDTHH:MM[:SS[.frac]]Z`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stamp {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}
impl Stamp {
    pub fn parse(text: &str) -> Option<Self> {
        let b = text.as_bytes();
        let digits = |range: std::ops::Range<usize>| -> Option<u32> {
            let part = text.get(range)?;
            if part.bytes().all(|c| c.is_ascii_digit()) {
                part.parse().ok()
            } else {
                None
            }
        };
        if b.len() < 17
            || b[4] != b'-'
            || b[7] != b'-'
            || b[10] != b'T'
            || b[13] != b':'
            || b[b.len() - 1] != b'Z'
        {
            return None;
        }
        let second = match b.len() {
            17 => 0,
            20.. if b[16] == b':' && (b.len() == 20 || b[19] == b'.') => {
                if b.len() > 20 && !text[20..b.len() - 1].bytes().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                digits(17..19)?
            }
            _ => return None,
        };
        let stamp = Self {
            year: i64::from(digits(0..4)?),
            month: digits(5..7)?,
            day: digits(8..10)?,
            hour: digits(11..13)?,
            minute: digits(14..16)?,
            second,
        };
        ((1..=12).contains(&stamp.month)
            && (1..=31).contains(&stamp.day)
            && stamp.hour < 24
            && stamp.minute < 60
            && stamp.second < 60)
            .then_some(stamp)
    }
    /// Seconds since 1970 (proleptic Gregorian, UTC).
    pub fn seconds(&self) -> i64 {
        let (y, m) = if self.month <= 2 {
            (self.year - 1, i64::from(self.month) + 9)
        } else {
            (self.year, i64::from(self.month) - 3)
        };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * m + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        days * 86_400 + i64::from(self.hour * 3600 + self.minute * 60 + self.second)
    }
    pub fn date(&self) -> String {
        format!("{} {}", MONTHS[self.month as usize - 1], self.day)
    }
    pub fn clock(&self) -> String {
        if self.second == 0 {
            format!("{:02}:{:02}", self.hour, self.minute)
        } else {
            format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
        }
    }
    /// `Oct 22, 10:03`.
    pub fn human(&self) -> String {
        format!("{}, {}", self.date(), self.clock())
    }
}

/// A cell for reading and its exact text. Integer columns are compacted
/// with their unit; timestamps read as `Oct 22, 10:03`; dates as
/// `Oct 22, 2026`; anything else is shown as released.
pub fn cell(column: &str, integer_column: bool, text: &str) -> (String, String) {
    if integer_column {
        if let Some((negative, n)) = integer(text) {
            return amount(unit_of(column), negative, n);
        }
    }
    if let Some(stamp) = Stamp::parse(text) {
        return (stamp.human(), text.to_owned());
    }
    if let Some(stamp) = Stamp::parse(&format!("{text}T00:00Z")) {
        if text.len() == 10 {
            return (format!("{}, {}", stamp.date(), stamp.year), text.to_owned());
        }
    }
    (text.to_owned(), text.to_owned())
}

/// `10:03 UTC` for a whole-second UTC timestamp (`events::timestamp`).
pub fn released_at(timestamp: &str) -> String {
    Stamp::parse(timestamp)
        .map(|stamp| format!("{:02}:{:02} UTC", stamp.hour, stamp.minute))
        .unwrap_or_default()
}

// --- The chart ---

/// One bar: geometry in the chart's view box, and its exact reading.
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct BarProps {
    pub id: String,
    pub x: String,
    pub y: String,
    pub width: String,
    pub height: String,
    /// `cv-bar`, or `cv-bar is-peak` for the largest released value.
    pub class: String,
    pub tip: String,
}
/// One horizontal gridline and its value label.
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct GridProps {
    pub id: String,
    pub y: String,
    pub label_y: String,
    pub text: String,
}
/// One x-axis label.
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct TickProps {
    pub id: String,
    pub x: String,
    pub anchor: String,
    pub text: String,
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct ChartProps {
    /// The chart's accessible name: what it plots, how many points, the peak.
    pub label: String,
    pub caption: String,
    pub view_box: String,
    pub has_bars: bool,
    pub bars: Vec<BarProps>,
    /// A time series of many points is drawn as an area under a line.
    pub has_line: bool,
    pub line: String,
    pub area: String,
    pub grid: Vec<GridProps>,
    pub ticks: Vec<TickProps>,
}

pub const WIDTH: f64 = 560.0;
pub const HEIGHT: f64 = 188.0;
const LEFT: f64 = 52.0;
const RIGHT: f64 = 8.0;
const TOP: f64 = 10.0;
const BOTTOM: f64 = 26.0;
/// From this many time points on, a series is an area and a line.
const LINE_FROM: usize = 24;
const MAX_POINTS: usize = 120;

/// A coordinate to one decimal (`Math.round(v * 10) / 10` in `chart.ts`).
pub fn coordinate(value: f64) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if rounded == 0.0 {
        "0".to_owned()
    } else {
        format!("{rounded}")
    }
}

/// The gridline step: 1, 2, 2.5 or 5 times a power of ten (2.5 only from
/// 25 up), at most four steps to the top.
fn step(max: u128) -> u128 {
    let target = max.div_ceil(4).max(1);
    let mut magnitude = 1u128;
    while magnitude * 10 <= target {
        magnitude *= 10;
    }
    [10, 20, 25, 50, 100]
        .iter()
        .filter(|f| magnitude >= 10 || **f != 25)
        .map(|f| f * magnitude / 10)
        .find(|s| *s >= target)
        .unwrap_or(10 * magnitude)
}

/// The chart of a released table, when it has exactly the shape a chart
/// reads honestly: one value column of non-negative exact integers (the
/// first integer column), 2 to 120 rows, and a time or text column for the
/// other axis. `rows` are the released cell texts (integers with or without
/// grouping).
pub fn chart(
    columns: &[String],
    rows: &[Vec<String>],
    integer_columns: &[String],
) -> Option<ChartProps> {
    let is_integer = |index: usize| integer_columns.contains(&columns[index]);
    let value = (0..columns.len()).find(|index| is_integer(*index))?;
    let n = rows.len();
    if !(2..=MAX_POINTS).contains(&n) || rows.iter().any(|row| row.len() != columns.len()) {
        return None;
    }
    let values = rows
        .iter()
        .map(|row| match integer(&row[value]) {
            Some((false, v)) => Some(v),
            _ => None,
        })
        .collect::<Option<Vec<u128>>>()?;
    let max = *values.iter().max()?;
    if max == 0 {
        return None;
    }
    let time_column = (0..columns.len())
        .filter(|index| *index != value)
        .find(|index| rows.iter().all(|row| Stamp::parse(&row[*index]).is_some()));
    let axis = time_column.or_else(|| (0..columns.len()).find(|index| !is_integer(*index)))?;
    let stamps: Vec<Stamp> = match time_column {
        Some(index) => rows
            .iter()
            .filter_map(|row| Stamp::parse(&row[index]))
            .collect(),
        None => Vec::new(),
    };
    let unit = unit_of(&columns[value]);
    let reading = |v: u128| amount(unit, false, v);

    let plot_width = WIDTH - LEFT - RIGHT;
    let plot_height = HEIGHT - TOP - BOTTOM;
    let base = TOP + plot_height;
    let step = step(max);
    let top = max.div_ceil(step) * step;
    let y_of = |v: u128| base - (v as f64) / (top as f64) * plot_height;
    let grid = (0..=top / step)
        .map(|k| {
            let v = k * step;
            GridProps {
                id: format!("g{k}"),
                y: coordinate(y_of(v)),
                label_y: coordinate(y_of(v) + 3.5),
                text: if v == 0 { "0".to_owned() } else { reading(v).0 },
            }
        })
        .collect();

    // Positions along x (bar centres or points), in row order.
    let seconds: Vec<i64> = stamps.iter().map(Stamp::seconds).collect();
    let timed = !stamps.is_empty() && {
        let mut sorted = seconds.clone();
        sorted.sort_unstable();
        sorted.windows(2).all(|pair| pair[0] < pair[1])
    };
    let same_day = stamps
        .windows(2)
        .all(|pair| pair[0].date() == pair[1].date() && pair[0].year == pair[1].year);
    let axis_text = |index: usize| -> (String, String) {
        match stamps.get(index) {
            Some(stamp) => (
                if same_day {
                    stamp.clock()
                } else {
                    stamp.date()
                },
                stamp.human(),
            ),
            None => {
                let text = &rows[index][axis];
                let short: String = if text.chars().count() > 12 {
                    format!("{}…", text.chars().take(11).collect::<String>())
                } else {
                    text.clone()
                };
                (short, text.clone())
            }
        }
    };
    let line_mode = timed && n >= LINE_FROM;
    let (centres, width): (Vec<f64>, f64) = if timed {
        let (first, last) = (
            *seconds.iter().min().unwrap_or(&0),
            *seconds.iter().max().unwrap_or(&0),
        );
        if line_mode {
            let span = (last - first).max(1) as f64;
            (
                seconds
                    .iter()
                    .map(|s| LEFT + (*s - first) as f64 / span * plot_width)
                    .collect(),
                0.0,
            )
        } else {
            let mut sorted = seconds.clone();
            sorted.sort_unstable();
            let gap = sorted
                .windows(2)
                .map(|pair| pair[1] - pair[0])
                .min()
                .unwrap_or(1)
                .max(1) as f64;
            let start = first as f64 - gap / 2.0;
            let scale = plot_width / ((last - first) as f64 + gap);
            (
                seconds
                    .iter()
                    .map(|s| LEFT + (*s as f64 - start) * scale)
                    .collect(),
                (gap * scale * 0.64).max(3.0),
            )
        }
    } else {
        let slot = plot_width / n as f64;
        (
            (0..n).map(|i| LEFT + slot * (i as f64 + 0.5)).collect(),
            (slot * 0.64).max(2.0),
        )
    };

    let peak = (0..n).fold(0, |best, i| if values[i] > values[best] { i } else { best });
    let tip = |i: usize| {
        let mut parts = vec![axis_text(i).1];
        for (index, text) in rows[i].iter().enumerate() {
            if index != axis && index != value {
                parts.push(cell(&columns[index], is_integer(index), text).0);
            }
        }
        parts.push(reading(values[i]).1);
        parts.join(" · ")
    };
    let bars: Vec<BarProps> = if line_mode {
        Vec::new()
    } else {
        (0..n)
            .map(|i| {
                let y = y_of(values[i]);
                BarProps {
                    id: format!("b{i}"),
                    x: coordinate(centres[i] - width / 2.0),
                    y: coordinate(y),
                    width: coordinate(width),
                    height: coordinate(base - y),
                    class: if i == peak {
                        "cv-bar is-peak"
                    } else {
                        "cv-bar"
                    }
                    .to_owned(),
                    tip: tip(i),
                }
            })
            .collect()
    };
    let (line, area) = if line_mode {
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|i| seconds[*i]);
        let points: Vec<String> = order
            .iter()
            .map(|i| {
                format!(
                    "{} {}",
                    coordinate(centres[*i]),
                    coordinate(y_of(values[*i]))
                )
            })
            .collect();
        let line = format!("M{}", points.join(" L"));
        let first = coordinate(centres[order[0]]);
        let last = coordinate(centres[order[n - 1]]);
        let base_text = coordinate(base);
        (
            line.clone(),
            format!("{line} L{last} {base_text} L{first} {base_text} Z"),
        )
    } else {
        (String::new(), String::new())
    };

    // Up to six labels, left to right, at least 44 units apart.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|a, b| centres[*a].total_cmp(&centres[*b]));
    let wanted: Vec<usize> = if n <= 6 {
        order.clone()
    } else {
        let mut picked: Vec<usize> = (0..5).map(|k| order[(k * (n - 1) + 2) / 4]).collect();
        picked.dedup();
        picked
    };
    let mut placed: Vec<usize> = Vec::new();
    for index in wanted {
        if let Some(previous) = placed.last() {
            if centres[index] - centres[*previous] < 44.0 {
                if Some(&index) == order.last() {
                    placed.pop();
                } else {
                    continue;
                }
            }
        }
        placed.push(index);
    }
    let count = placed.len();
    let ticks = placed
        .iter()
        .enumerate()
        .map(|(k, index)| TickProps {
            id: format!("t{k}"),
            x: coordinate(centres[*index]),
            anchor: if line_mode && k == 0 {
                "start"
            } else if line_mode && k + 1 == count {
                "end"
            } else {
                "middle"
            }
            .to_owned(),
            text: axis_text(*index).0,
        })
        .collect();

    let peak_reading = reading(values[peak]).0;
    let caption = if timed {
        let first = order[0];
        let last = order[n - 1];
        let range = if same_day {
            format!(
                "{}, {}–{} UTC",
                stamps[first].date(),
                stamps[first].clock(),
                stamps[last].clock()
            )
        } else {
            format!("{} – {} UTC", stamps[first].human(), stamps[last].human())
        };
        let at = if same_day {
            stamps[peak].clock()
        } else {
            stamps[peak].human()
        };
        format!("{range}. Peak {peak_reading} at {at}.")
    } else {
        format!("Peak {peak_reading}: {}.", axis_text(peak).1)
    };
    Some(ChartProps {
        label: format!(
            "{} by {}, {n} {}. {caption}",
            label(&columns[value]),
            label(&columns[axis]).to_lowercase(),
            if line_mode { "points" } else { "bars" }
        ),
        caption,
        view_box: format!("0 0 {} {}", coordinate(WIDTH), coordinate(HEIGHT)),
        has_bars: !line_mode,
        bars,
        has_line: line_mode,
        line,
        area,
        grid,
        ticks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_read_with_their_unit_and_keep_the_exact_value() {
        let read = |column: &str, text: &str| cell(column, true, text);
        assert_eq!(
            read("total_bytes", "2040000"),
            ("2.04 MB".into(), "2,040,000 bytes".into())
        );
        assert_eq!(
            read("bytes", "60,000"),
            ("60 KB".into(), "60,000 bytes".into())
        );
        assert_eq!(read("bytes", "900000").0, "900 KB");
        assert_eq!(read("bytes", "999999").0, "1 MB");
        assert_eq!(read("bytes", "999").0, "999 B");
        assert_eq!(read("bytes", "1").1, "1 byte");
        // Money is never inferred from a name: a `*cents` column reads as a count.
        assert_eq!(
            read("amount_cents", "123456"),
            ("123K".into(), "123,456".into())
        );
        assert_eq!(read("count", "9999"), ("9,999".into(), "9,999".into()));
        assert_eq!(read("count", "12345").0, "12.3K");
        assert_eq!(read("count", "-12345").0, "-12.3K");
        assert_eq!(read("count", "1005"), ("1,005".into(), "1,005".into()));
        // Not an integer: shown as released.
        assert_eq!(
            read("count", "unknown"),
            ("unknown".into(), "unknown".into())
        );
        assert_eq!(cell("bytes", false, "60000").0, "60000");
    }

    #[test]
    fn times_read_as_dates_and_clocks() {
        assert_eq!(
            cell("minute", false, "2026-10-22T10:03:00Z").0,
            "Oct 22, 10:03"
        );
        assert_eq!(
            cell("minute", false, "2026-10-22T10:03:15Z").0,
            "Oct 22, 10:03:15"
        );
        assert_eq!(
            cell("minute", false, "2026-10-22T10:03Z").0,
            "Oct 22, 10:03"
        );
        assert_eq!(cell("day", false, "2026-10-22").0, "Oct 22, 2026");
        assert_eq!(cell("ip", false, "203.0.113.10").0, "203.0.113.10");
        assert_eq!(released_at("2026-10-30T15:04:59Z"), "15:04 UTC");
        let stamp = Stamp::parse("2026-10-22T10:00:00Z").unwrap();
        assert_eq!(
            crate::events::timestamp(stamp.seconds() as u64),
            "2026-10-22T10:00:00Z"
        );
        assert!(Stamp::parse("2026-13-22T10:00:00Z").is_none());
        assert!(Stamp::parse("2026-10-22 10:00:00").is_none());
    }

    #[test]
    fn a_timeline_is_a_time_scaled_bar_chart_of_the_released_values() {
        let columns = vec![
            "minute".to_owned(),
            "destination".to_owned(),
            "bytes".to_owned(),
        ];
        let rows: Vec<Vec<String>> = [
            ("10:00", "203.0.113.10", "60,000"),
            ("10:01", "203.0.113.20", "120,000"),
            ("10:02", "198.51.100.7", "300,000"),
            ("10:03", "198.51.100.7", "600,000"),
            ("10:05", "203.0.113.10", "60,000"),
            ("10:10", "198.51.100.99", "900,000"),
        ]
        .iter()
        .map(|(m, d, b)| {
            vec![
                format!("2026-10-22T{m}:00Z"),
                (*d).to_owned(),
                (*b).to_owned(),
            ]
        })
        .collect();
        let chart = chart(&columns, &rows, &["bytes".to_owned()]).unwrap();
        assert!(chart.has_bars && !chart.has_line);
        assert_eq!(chart.bars.len(), 6);
        assert_eq!(chart.bars[5].class, "cv-bar is-peak");
        assert_eq!(
            chart.bars[5].tip,
            "Oct 22, 10:10 · 198.51.100.99 · 900,000 bytes"
        );
        assert_eq!(
            chart.caption,
            "Oct 22, 10:00–10:10 UTC. Peak 900 KB at 10:10."
        );
        assert_eq!(
            chart
                .grid
                .iter()
                .map(|g| g.text.as_str())
                .collect::<Vec<_>>(),
            ["0", "250 KB", "500 KB", "750 KB", "1 MB"]
        );
        // The tallest bar reaches 900/1000 of the plot; bars sit on the base.
        let base = TOP + (HEIGHT - TOP - BOTTOM);
        for bar in &chart.bars {
            let (y, h): (f64, f64) = (bar.y.parse().unwrap(), bar.height.parse().unwrap());
            assert!((y + h - base).abs() < 0.11, "{bar:?}");
        }
        assert!(chart.ticks.iter().any(|t| t.text == "10:10"));
        assert!(chart.label.starts_with("Bytes by minute, 6 bars."));
        // No chart for a single row, a negative or unknown value, or no axis.
        assert!(super::chart(&columns, &rows[..1], &["bytes".to_owned()]).is_none());
        let mut unknown = rows.clone();
        unknown[2][2] = "unknown".into();
        assert!(super::chart(&columns, &unknown, &["bytes".to_owned()]).is_none());
        assert!(super::chart(&columns, &rows, &[]).is_none());
    }

    #[test]
    fn a_long_time_series_is_an_area_and_a_line() {
        let columns = vec!["minute".to_owned(), "events".to_owned()];
        let rows: Vec<Vec<String>> = (0..30)
            .map(|m| vec![format!("2026-10-22T10:{m:02}:00Z"), (m * 3 + 1).to_string()])
            .collect();
        let chart = chart(&columns, &rows, &["events".to_owned()]).unwrap();
        assert!(chart.has_line && !chart.has_bars && chart.bars.is_empty());
        assert!(chart.line.starts_with("M52 ") && chart.area.ends_with(" Z"));
        assert_eq!(chart.ticks.first().unwrap().anchor, "start");
        assert_eq!(chart.ticks.last().unwrap().anchor, "end");
    }

    #[test]
    fn labels_and_coordinates() {
        assert_eq!(label("total_bytes"), "Total bytes");
        assert_eq!(label("timeline"), "Timeline");
        assert_eq!(coordinate(12.0), "12");
        assert_eq!(coordinate(12.34), "12.3");
        assert_eq!(coordinate(0.04), "0");
        assert_eq!(step(900_000), 250_000);
        assert_eq!(step(7), 2);
        assert_eq!(step(1), 1);
    }
}
