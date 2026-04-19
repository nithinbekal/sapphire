use std::collections::HashMap;

use jiff::civil;
use jiff::tz::{Disambiguation, TimeZone};
use jiff::{Span, Timestamp, Zoned};

use crate::gc::{GcHeap, GcRef};
use crate::vm::{HeapObject, VmError, VmValue};

// ── Public result type ────────────────────────────────────────────────────────

/// Returned by every dispatch function so the caller in `vm.rs` can allocate
/// the GcRef for new instances without needing access to private VM internals.
pub enum DtValue {
    Value(VmValue),
    NewInstance {
        class_name: String,
        fields: HashMap<String, VmValue>,
    },
}

impl From<VmValue> for DtValue {
    fn from(v: VmValue) -> Self {
        DtValue::Value(v)
    }
}

// ── Error helpers ─────────────────────────────────────────────────────────────

fn raise(msg: impl Into<String>) -> VmError {
    VmError::Raised(VmValue::Str(msg.into()))
}

fn jiff_err(e: impl std::fmt::Display) -> VmError {
    raise(e.to_string())
}

fn type_err(msg: impl Into<String>, line: u32) -> VmError {
    VmError::TypeError { message: msg.into(), line }
}

fn wrong_args(class: &str, method: &str, expected: usize, got: usize, line: u32) -> VmError {
    type_err(
        format!("{}.{} expects {} arg(s), got {}", class, method, expected, got),
        line,
    )
}

// ── Field map helpers ─────────────────────────────────────────────────────────

fn get_int(f: &HashMap<String, VmValue>, key: &str) -> i64 {
    match f.get(key) {
        Some(VmValue::Int(n)) => *n,
        _ => 0,
    }
}

fn get_str<'a>(f: &'a HashMap<String, VmValue>, key: &str) -> &'a str {
    match f.get(key) {
        Some(VmValue::Str(s)) => s.as_str(),
        _ => "",
    }
}

fn flds(heap: &GcHeap<HeapObject>, r: GcRef) -> &HashMap<String, VmValue> {
    heap.get_fields(r)
}

// ── Arg helpers ───────────────────────────────────────────────────────────────

fn int_arg(args: &[VmValue], idx: usize, ctx: &str) -> Result<i64, VmError> {
    match args.get(idx) {
        Some(VmValue::Int(n)) => Ok(*n),
        Some(_) => Err(raise(format!("{}: arg {} must be an integer", ctx, idx + 1))),
        None => Err(raise(format!("{}: missing arg {}", ctx, idx + 1))),
    }
}

fn str_arg<'a>(args: &'a [VmValue], idx: usize, ctx: &str) -> Result<&'a str, VmError> {
    match args.get(idx) {
        Some(VmValue::Str(s)) => Ok(s.as_str()),
        Some(_) => Err(raise(format!("{}: arg {} must be a string", ctx, idx + 1))),
        None => Err(raise(format!("{}: missing arg {}", ctx, idx + 1))),
    }
}

// ── Conversion: fields → jiff types ──────────────────────────────────────────

fn to_timestamp(heap: &GcHeap<HeapObject>, r: GcRef) -> Result<Timestamp, VmError> {
    let f = flds(heap, r);
    Timestamp::new(get_int(f, "_secs"), get_int(f, "_ns") as i32).map_err(jiff_err)
}

fn to_date(heap: &GcHeap<HeapObject>, r: GcRef) -> Result<civil::Date, VmError> {
    let f = flds(heap, r);
    civil::Date::new(get_int(f, "_y") as i16, get_int(f, "_mo") as i8, get_int(f, "_d") as i8)
        .map_err(jiff_err)
}

fn to_time(heap: &GcHeap<HeapObject>, r: GcRef) -> Result<civil::Time, VmError> {
    let f = flds(heap, r);
    civil::Time::new(
        get_int(f, "_h") as i8,
        get_int(f, "_mi") as i8,
        get_int(f, "_s") as i8,
        get_int(f, "_ns") as i32,
    )
    .map_err(jiff_err)
}

fn to_datetime(heap: &GcHeap<HeapObject>, r: GcRef) -> Result<civil::DateTime, VmError> {
    let f = flds(heap, r);
    civil::DateTime::new(
        get_int(f, "_y") as i16,
        get_int(f, "_mo") as i8,
        get_int(f, "_d") as i8,
        get_int(f, "_h") as i8,
        get_int(f, "_mi") as i8,
        get_int(f, "_s") as i8,
        get_int(f, "_ns") as i32,
    )
    .map_err(jiff_err)
}

fn to_zoned(heap: &GcHeap<HeapObject>, r: GcRef) -> Result<Zoned, VmError> {
    let f = flds(heap, r);
    let ts = Timestamp::new(get_int(f, "_secs"), get_int(f, "_ns") as i32).map_err(jiff_err)?;
    let tz = TimeZone::get(get_str(f, "_tz")).map_err(jiff_err)?;
    Ok(ts.to_zoned(tz))
}

fn to_span(heap: &GcHeap<HeapObject>, r: GcRef) -> Result<Span, VmError> {
    let f = flds(heap, r);
    Ok(Span::new()
        .years(get_int(f, "_years"))
        .months(get_int(f, "_months"))
        .days(get_int(f, "_days"))
        .hours(get_int(f, "_hours"))
        .minutes(get_int(f, "_minutes"))
        .seconds(get_int(f, "_secs"))
        .nanoseconds(get_int(f, "_ns")))
}

fn span_from_val(heap: &GcHeap<HeapObject>, val: &VmValue) -> Result<Span, VmError> {
    match val {
        VmValue::Instance { class_name, fields, .. } if class_name == "Duration" => {
            to_span(heap, *fields)
        }
        _ => Err(raise("expected a Duration instance")),
    }
}

// ── Conversion: jiff types → field maps ──────────────────────────────────────

fn from_timestamp(ts: Timestamp) -> HashMap<String, VmValue> {
    let mut m = HashMap::new();
    m.insert("_secs".into(), VmValue::Int(ts.as_second()));
    m.insert("_ns".into(), VmValue::Int(ts.subsec_nanosecond() as i64));
    m
}

fn from_date(d: civil::Date) -> HashMap<String, VmValue> {
    let mut m = HashMap::new();
    m.insert("_y".into(), VmValue::Int(d.year() as i64));
    m.insert("_mo".into(), VmValue::Int(d.month() as i64));
    m.insert("_d".into(), VmValue::Int(d.day() as i64));
    m
}

fn from_time(t: civil::Time) -> HashMap<String, VmValue> {
    let mut m = HashMap::new();
    m.insert("_h".into(), VmValue::Int(t.hour() as i64));
    m.insert("_mi".into(), VmValue::Int(t.minute() as i64));
    m.insert("_s".into(), VmValue::Int(t.second() as i64));
    m.insert("_ns".into(), VmValue::Int(t.subsec_nanosecond() as i64));
    m
}

fn from_datetime(dt: civil::DateTime) -> HashMap<String, VmValue> {
    let mut m = HashMap::new();
    m.insert("_y".into(), VmValue::Int(dt.year() as i64));
    m.insert("_mo".into(), VmValue::Int(dt.month() as i64));
    m.insert("_d".into(), VmValue::Int(dt.day() as i64));
    m.insert("_h".into(), VmValue::Int(dt.hour() as i64));
    m.insert("_mi".into(), VmValue::Int(dt.minute() as i64));
    m.insert("_s".into(), VmValue::Int(dt.second() as i64));
    m.insert("_ns".into(), VmValue::Int(dt.subsec_nanosecond() as i64));
    m
}

fn from_zoned(z: &Zoned) -> HashMap<String, VmValue> {
    let ts = z.timestamp();
    let tz_name = z.time_zone().iana_name().unwrap_or("UTC").to_owned();
    let mut m = HashMap::new();
    m.insert("_secs".into(), VmValue::Int(ts.as_second()));
    m.insert("_ns".into(), VmValue::Int(ts.subsec_nanosecond() as i64));
    m.insert("_tz".into(), VmValue::Str(tz_name));
    m
}

fn from_span(s: Span) -> HashMap<String, VmValue> {
    let mut m = HashMap::new();
    m.insert("_years".into(), VmValue::Int(s.get_years() as i64));
    m.insert("_months".into(), VmValue::Int(s.get_months() as i64));
    m.insert("_days".into(), VmValue::Int(s.get_days() as i64));
    m.insert("_hours".into(), VmValue::Int(s.get_hours() as i64));
    m.insert("_minutes".into(), VmValue::Int(s.get_minutes()));
    m.insert("_secs".into(), VmValue::Int(s.get_seconds()));
    m.insert("_ns".into(), VmValue::Int(s.get_nanoseconds()));
    m
}

// ── Instance constructors ─────────────────────────────────────────────────────

fn mk_instant(ts: Timestamp) -> DtValue {
    DtValue::NewInstance { class_name: "Instant".into(), fields: from_timestamp(ts) }
}
fn mk_date(d: civil::Date) -> DtValue {
    DtValue::NewInstance { class_name: "Date".into(), fields: from_date(d) }
}
fn mk_time(t: civil::Time) -> DtValue {
    DtValue::NewInstance { class_name: "Time".into(), fields: from_time(t) }
}
fn mk_datetime(dt: civil::DateTime) -> DtValue {
    DtValue::NewInstance { class_name: "DateTime".into(), fields: from_datetime(dt) }
}
fn mk_zoned(z: &Zoned) -> DtValue {
    DtValue::NewInstance { class_name: "ZonedDateTime".into(), fields: from_zoned(z) }
}
fn mk_duration(s: Span) -> DtValue {
    DtValue::NewInstance { class_name: "Duration".into(), fields: from_span(s) }
}

// ── Build Span from DateTime components ───────────────────────────────────────

fn span_from_components(y: i64, mo: i64, d: i64, h: i64, mi: i64, s: i64, ns: i64) -> Span {
    Span::new()
        .years(y).months(mo).days(d)
        .hours(h).minutes(mi).seconds(s).nanoseconds(ns)
}

/// Normalize a time-only span (no calendar units) from a timestamp difference
/// into canonical h/m/s/ns form so `duration.hours()` etc. work naturally.
fn normalize_time_span(span: Span) -> Span {
    let total_ns: i64 = span.get_seconds() * 1_000_000_000 + span.get_nanoseconds();
    let ns = total_ns % 1_000_000_000;
    let total_s = total_ns / 1_000_000_000;
    let secs = total_s % 60;
    let total_m = total_s / 60;
    let mins = total_m % 60;
    let hours = total_m / 60;
    Span::new().hours(hours).minutes(mins).seconds(secs).nanoseconds(ns)
}

// ── Public entry points ───────────────────────────────────────────────────────

pub fn dispatch_class_method(
    heap: &GcHeap<HeapObject>,
    class_name: &str,
    method_name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<DtValue, VmError> {
    match class_name {
        "Instant" => instant_class(heap, method_name, args, line),
        "Date" => date_class(method_name, args, line),
        "Time" => time_class(method_name, args, line),
        "DateTime" => datetime_class(method_name, args, line),
        "ZonedDateTime" => zoned_class(heap, method_name, args, line),
        "Duration" => duration_class(method_name, args, line),
        _ => Err(type_err(
            format!("{} has no native class method '{}'", class_name, method_name),
            line,
        )),
    }
}

pub fn dispatch_instance_method(
    heap: &GcHeap<HeapObject>,
    class_name: &str,
    fields: GcRef,
    method_name: &str,
    args: &[VmValue],
    line: u32,
) -> Result<DtValue, VmError> {
    match class_name {
        "Instant" => instant_instance(heap, fields, method_name, args, line),
        "Date" => date_instance(heap, fields, method_name, args, line),
        "Time" => time_instance(heap, fields, method_name, args, line),
        "DateTime" => datetime_instance(heap, fields, method_name, args, line),
        "ZonedDateTime" => zoned_instance(heap, fields, method_name, args, line),
        "Duration" => duration_instance(heap, fields, method_name, args, line),
        _ => Err(type_err(
            format!("{} has no native method '{}'", class_name, method_name),
            line,
        )),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Instant
// ════════════════════════════════════════════════════════════════════════════

fn instant_class(
    _heap: &GcHeap<HeapObject>,
    method: &str,
    args: &[VmValue],
    line: u32,
) -> Result<DtValue, VmError> {
    match method {
        "now" => {
            if !args.is_empty() {
                return Err(wrong_args("Instant", "now", 0, args.len(), line));
            }
            Ok(mk_instant(Timestamp::now()))
        }
        "of_epoch_seconds" => {
            if args.len() != 1 {
                return Err(wrong_args("Instant", "of_epoch_seconds", 1, args.len(), line));
            }
            let s = int_arg(args, 0, "Instant.of_epoch_seconds")?;
            Ok(mk_instant(Timestamp::from_second(s).map_err(jiff_err)?))
        }
        "of_epoch_millis" => {
            if args.len() != 1 {
                return Err(wrong_args("Instant", "of_epoch_millis", 1, args.len(), line));
            }
            let ms = int_arg(args, 0, "Instant.of_epoch_millis")?;
            Ok(mk_instant(Timestamp::from_millisecond(ms).map_err(jiff_err)?))
        }
        "parse" => {
            if args.len() != 1 {
                return Err(wrong_args("Instant", "parse", 1, args.len(), line));
            }
            let s = str_arg(args, 0, "Instant.parse")?;
            Ok(mk_instant(s.parse::<Timestamp>().map_err(jiff_err)?))
        }
        _ => Err(type_err(format!("Instant has no class method '{}'", method), line)),
    }
}

fn instant_instance(
    heap: &GcHeap<HeapObject>,
    fields: GcRef,
    method: &str,
    args: &[VmValue],
    line: u32,
) -> Result<DtValue, VmError> {
    let ts = || to_timestamp(heap, fields);
    match method {
        "to_s" | "inspect" => Ok(VmValue::Str(ts()?.to_string()).into()),
        "epoch_seconds" => Ok(VmValue::Int(get_int(flds(heap, fields), "_secs")).into()),
        "epoch_millis" => Ok(VmValue::Int(ts()?.as_millisecond()).into()),
        "add" => {
            if args.len() != 1 {
                return Err(wrong_args("Instant", "add", 1, args.len(), line));
            }
            let span = span_from_val(heap, &args[0])?;
            Ok(mk_instant(ts()?.checked_add(span).map_err(jiff_err)?))
        }
        "sub" => {
            if args.len() != 1 {
                return Err(wrong_args("Instant", "sub", 1, args.len(), line));
            }
            match &args[0] {
                VmValue::Instance { class_name, fields: of, .. } if class_name == "Instant" => {
                    let other = to_timestamp(heap, *of)?;
                    let raw = ts()?.since(other).map_err(jiff_err)?;
                    Ok(mk_duration(normalize_time_span(raw)))
                }
                VmValue::Instance { class_name, fields: of, .. } if class_name == "Duration" => {
                    let span = to_span(heap, *of)?;
                    Ok(mk_instant(ts()?.checked_sub(span).map_err(jiff_err)?))
                }
                _ => Err(raise("Instant.sub: argument must be an Instant or Duration")),
            }
        }
        "before?" => {
            if args.len() != 1 {
                return Err(wrong_args("Instant", "before?", 1, args.len(), line));
            }
            let other = instant_from_arg(heap, &args[0], "Instant.before?")?;
            Ok(VmValue::Bool(ts()? < other).into())
        }
        "after?" => {
            if args.len() != 1 {
                return Err(wrong_args("Instant", "after?", 1, args.len(), line));
            }
            let other = instant_from_arg(heap, &args[0], "Instant.after?")?;
            Ok(VmValue::Bool(ts()? > other).into())
        }
        "equal?" => {
            if args.len() != 1 {
                return Err(wrong_args("Instant", "equal?", 1, args.len(), line));
            }
            let other = instant_from_arg(heap, &args[0], "Instant.equal?")?;
            Ok(VmValue::Bool(ts()? == other).into())
        }
        "format" => {
            if args.len() != 1 {
                return Err(wrong_args("Instant", "format", 1, args.len(), line));
            }
            let pat = str_arg(args, 0, "Instant.format")?;
            Ok(VmValue::Str(ts()?.strftime(pat).to_string()).into())
        }
        "in_timezone" => {
            if args.len() != 1 {
                return Err(wrong_args("Instant", "in_timezone", 1, args.len(), line));
            }
            let tz_name = str_arg(args, 0, "Instant.in_timezone")?;
            let tz = TimeZone::get(tz_name).map_err(jiff_err)?;
            Ok(mk_zoned(&ts()?.to_zoned(tz)))
        }
        _ => Err(type_err(format!("Instant has no method '{}'", method), line)),
    }
}

fn instant_from_arg(
    heap: &GcHeap<HeapObject>,
    val: &VmValue,
    ctx: &str,
) -> Result<Timestamp, VmError> {
    match val {
        VmValue::Instance { class_name, fields, .. } if class_name == "Instant" => {
            to_timestamp(heap, *fields)
        }
        _ => Err(raise(format!("{}: argument must be an Instant", ctx))),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Date
// ════════════════════════════════════════════════════════════════════════════

fn date_class(method: &str, args: &[VmValue], line: u32) -> Result<DtValue, VmError> {
    match method {
        "today" => {
            if !args.is_empty() {
                return Err(wrong_args("Date", "today", 0, args.len(), line));
            }
            Ok(mk_date(Zoned::now().date()))
        }
        "of" => {
            if args.len() != 3 {
                return Err(wrong_args("Date", "of", 3, args.len(), line));
            }
            let y = int_arg(args, 0, "Date.of")? as i16;
            let mo = int_arg(args, 1, "Date.of")? as i8;
            let d = int_arg(args, 2, "Date.of")? as i8;
            Ok(mk_date(civil::Date::new(y, mo, d).map_err(jiff_err)?))
        }
        "parse" => {
            if args.len() != 1 {
                return Err(wrong_args("Date", "parse", 1, args.len(), line));
            }
            let s = str_arg(args, 0, "Date.parse")?;
            Ok(mk_date(s.parse::<civil::Date>().map_err(jiff_err)?))
        }
        _ => Err(type_err(format!("Date has no class method '{}'", method), line)),
    }
}

fn date_instance(
    heap: &GcHeap<HeapObject>,
    fields: GcRef,
    method: &str,
    args: &[VmValue],
    line: u32,
) -> Result<DtValue, VmError> {
    let date = || to_date(heap, fields);
    match method {
        "to_s" | "inspect" => Ok(VmValue::Str(date()?.to_string()).into()),
        "year" => Ok(VmValue::Int(get_int(flds(heap, fields), "_y")).into()),
        "month" => Ok(VmValue::Int(get_int(flds(heap, fields), "_mo")).into()),
        "day" => Ok(VmValue::Int(get_int(flds(heap, fields), "_d")).into()),
        "day_of_week" => {
            // 1 = Monday … 7 = Sunday (ISO 8601)
            Ok(VmValue::Int(date()?.weekday().to_monday_one_offset() as i64).into())
        }
        "day_of_year" => Ok(VmValue::Int(date()?.day_of_year() as i64).into()),
        "days_in_month" => Ok(VmValue::Int(date()?.days_in_month() as i64).into()),
        "add" => {
            if args.len() != 1 {
                return Err(wrong_args("Date", "add", 1, args.len(), line));
            }
            let span = span_from_val(heap, &args[0])?;
            Ok(mk_date(date()?.checked_add(span).map_err(jiff_err)?))
        }
        "sub" => {
            if args.len() != 1 {
                return Err(wrong_args("Date", "sub", 1, args.len(), line));
            }
            match &args[0] {
                VmValue::Instance { class_name, fields: of, .. } if class_name == "Date" => {
                    let other = to_date(heap, *of)?;
                    Ok(mk_duration(date()?.since(other).map_err(jiff_err)?))
                }
                VmValue::Instance { class_name, fields: of, .. } if class_name == "Duration" => {
                    let span = to_span(heap, *of)?;
                    Ok(mk_date(date()?.checked_sub(span).map_err(jiff_err)?))
                }
                _ => Err(raise("Date.sub: argument must be a Date or Duration")),
            }
        }
        "before?" => {
            if args.len() != 1 {
                return Err(wrong_args("Date", "before?", 1, args.len(), line));
            }
            let other = date_from_arg(heap, &args[0], "Date.before?")?;
            Ok(VmValue::Bool(date()? < other).into())
        }
        "after?" => {
            if args.len() != 1 {
                return Err(wrong_args("Date", "after?", 1, args.len(), line));
            }
            let other = date_from_arg(heap, &args[0], "Date.after?")?;
            Ok(VmValue::Bool(date()? > other).into())
        }
        "equal?" => {
            if args.len() != 1 {
                return Err(wrong_args("Date", "equal?", 1, args.len(), line));
            }
            let other = date_from_arg(heap, &args[0], "Date.equal?")?;
            Ok(VmValue::Bool(date()? == other).into())
        }
        "format" => {
            if args.len() != 1 {
                return Err(wrong_args("Date", "format", 1, args.len(), line));
            }
            let pat = str_arg(args, 0, "Date.format")?;
            Ok(VmValue::Str(date()?.strftime(pat).to_string()).into())
        }
        "next_day" => Ok(mk_date(date()?.checked_add(Span::new().days(1)).map_err(jiff_err)?)),
        "prev_day" => Ok(mk_date(date()?.checked_sub(Span::new().days(1)).map_err(jiff_err)?)),
        _ => Err(type_err(format!("Date has no method '{}'", method), line)),
    }
}

fn date_from_arg(
    heap: &GcHeap<HeapObject>,
    val: &VmValue,
    ctx: &str,
) -> Result<civil::Date, VmError> {
    match val {
        VmValue::Instance { class_name, fields, .. } if class_name == "Date" => {
            to_date(heap, *fields)
        }
        _ => Err(raise(format!("{}: argument must be a Date", ctx))),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Time
// ════════════════════════════════════════════════════════════════════════════

fn time_class(method: &str, args: &[VmValue], line: u32) -> Result<DtValue, VmError> {
    match method {
        "now" => {
            if !args.is_empty() {
                return Err(wrong_args("Time", "now", 0, args.len(), line));
            }
            Ok(mk_time(Zoned::now().time()))
        }
        "of" => {
            if args.len() != 3 {
                return Err(wrong_args("Time", "of", 3, args.len(), line));
            }
            let h = int_arg(args, 0, "Time.of")? as i8;
            let mi = int_arg(args, 1, "Time.of")? as i8;
            let s = int_arg(args, 2, "Time.of")? as i8;
            Ok(mk_time(civil::Time::new(h, mi, s, 0).map_err(jiff_err)?))
        }
        "of_nanos" => {
            if args.len() != 4 {
                return Err(wrong_args("Time", "of_nanos", 4, args.len(), line));
            }
            let h = int_arg(args, 0, "Time.of_nanos")? as i8;
            let mi = int_arg(args, 1, "Time.of_nanos")? as i8;
            let s = int_arg(args, 2, "Time.of_nanos")? as i8;
            let ns = int_arg(args, 3, "Time.of_nanos")? as i32;
            Ok(mk_time(civil::Time::new(h, mi, s, ns).map_err(jiff_err)?))
        }
        "parse" => {
            if args.len() != 1 {
                return Err(wrong_args("Time", "parse", 1, args.len(), line));
            }
            let s = str_arg(args, 0, "Time.parse")?;
            Ok(mk_time(s.parse::<civil::Time>().map_err(jiff_err)?))
        }
        _ => Err(type_err(format!("Time has no class method '{}'", method), line)),
    }
}

fn time_instance(
    heap: &GcHeap<HeapObject>,
    fields: GcRef,
    method: &str,
    args: &[VmValue],
    line: u32,
) -> Result<DtValue, VmError> {
    let f = flds(heap, fields);
    let time = || to_time(heap, fields);
    match method {
        "to_s" | "inspect" => Ok(VmValue::Str(time()?.to_string()).into()),
        "hour" => Ok(VmValue::Int(get_int(f, "_h")).into()),
        "minute" => Ok(VmValue::Int(get_int(f, "_mi")).into()),
        "second" => Ok(VmValue::Int(get_int(f, "_s")).into()),
        "nano" => Ok(VmValue::Int(get_int(f, "_ns")).into()),
        "before?" => {
            if args.len() != 1 {
                return Err(wrong_args("Time", "before?", 1, args.len(), line));
            }
            let other = time_from_arg(heap, &args[0], "Time.before?")?;
            Ok(VmValue::Bool(time()? < other).into())
        }
        "after?" => {
            if args.len() != 1 {
                return Err(wrong_args("Time", "after?", 1, args.len(), line));
            }
            let other = time_from_arg(heap, &args[0], "Time.after?")?;
            Ok(VmValue::Bool(time()? > other).into())
        }
        "equal?" => {
            if args.len() != 1 {
                return Err(wrong_args("Time", "equal?", 1, args.len(), line));
            }
            let other = time_from_arg(heap, &args[0], "Time.equal?")?;
            Ok(VmValue::Bool(time()? == other).into())
        }
        "format" => {
            if args.len() != 1 {
                return Err(wrong_args("Time", "format", 1, args.len(), line));
            }
            let pat = str_arg(args, 0, "Time.format")?;
            Ok(VmValue::Str(time()?.strftime(pat).to_string()).into())
        }
        _ => Err(type_err(format!("Time has no method '{}'", method), line)),
    }
}

fn time_from_arg(
    heap: &GcHeap<HeapObject>,
    val: &VmValue,
    ctx: &str,
) -> Result<civil::Time, VmError> {
    match val {
        VmValue::Instance { class_name, fields, .. } if class_name == "Time" => {
            to_time(heap, *fields)
        }
        _ => Err(raise(format!("{}: argument must be a Time", ctx))),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// DateTime
// ════════════════════════════════════════════════════════════════════════════

fn datetime_class(method: &str, args: &[VmValue], line: u32) -> Result<DtValue, VmError> {
    match method {
        "now" => {
            if !args.is_empty() {
                return Err(wrong_args("DateTime", "now", 0, args.len(), line));
            }
            Ok(mk_datetime(Zoned::now().datetime()))
        }
        "of" => {
            if args.len() != 6 {
                return Err(wrong_args("DateTime", "of", 6, args.len(), line));
            }
            let dt = civil::DateTime::new(
                int_arg(args, 0, "DateTime.of")? as i16,
                int_arg(args, 1, "DateTime.of")? as i8,
                int_arg(args, 2, "DateTime.of")? as i8,
                int_arg(args, 3, "DateTime.of")? as i8,
                int_arg(args, 4, "DateTime.of")? as i8,
                int_arg(args, 5, "DateTime.of")? as i8,
                0,
            )
            .map_err(jiff_err)?;
            Ok(mk_datetime(dt))
        }
        "of_nanos" => {
            if args.len() != 7 {
                return Err(wrong_args("DateTime", "of_nanos", 7, args.len(), line));
            }
            let dt = civil::DateTime::new(
                int_arg(args, 0, "DateTime.of_nanos")? as i16,
                int_arg(args, 1, "DateTime.of_nanos")? as i8,
                int_arg(args, 2, "DateTime.of_nanos")? as i8,
                int_arg(args, 3, "DateTime.of_nanos")? as i8,
                int_arg(args, 4, "DateTime.of_nanos")? as i8,
                int_arg(args, 5, "DateTime.of_nanos")? as i8,
                int_arg(args, 6, "DateTime.of_nanos")? as i32,
            )
            .map_err(jiff_err)?;
            Ok(mk_datetime(dt))
        }
        "parse" => {
            if args.len() != 1 {
                return Err(wrong_args("DateTime", "parse", 1, args.len(), line));
            }
            let s = str_arg(args, 0, "DateTime.parse")?;
            Ok(mk_datetime(s.parse::<civil::DateTime>().map_err(jiff_err)?))
        }
        _ => Err(type_err(format!("DateTime has no class method '{}'", method), line)),
    }
}

fn datetime_instance(
    heap: &GcHeap<HeapObject>,
    fields: GcRef,
    method: &str,
    args: &[VmValue],
    line: u32,
) -> Result<DtValue, VmError> {
    let f = flds(heap, fields);
    let dt = || to_datetime(heap, fields);
    match method {
        "to_s" | "inspect" => Ok(VmValue::Str(dt()?.to_string()).into()),
        "year" => Ok(VmValue::Int(get_int(f, "_y")).into()),
        "month" => Ok(VmValue::Int(get_int(f, "_mo")).into()),
        "day" => Ok(VmValue::Int(get_int(f, "_d")).into()),
        "hour" => Ok(VmValue::Int(get_int(f, "_h")).into()),
        "minute" => Ok(VmValue::Int(get_int(f, "_mi")).into()),
        "second" => Ok(VmValue::Int(get_int(f, "_s")).into()),
        "nano" => Ok(VmValue::Int(get_int(f, "_ns")).into()),
        "date" => Ok(mk_date(dt()?.date())),
        "time" => Ok(mk_time(dt()?.time())),
        "add" => {
            if args.len() != 1 {
                return Err(wrong_args("DateTime", "add", 1, args.len(), line));
            }
            let span = span_from_val(heap, &args[0])?;
            Ok(mk_datetime(dt()?.checked_add(span).map_err(jiff_err)?))
        }
        "sub" => {
            if args.len() != 1 {
                return Err(wrong_args("DateTime", "sub", 1, args.len(), line));
            }
            match &args[0] {
                VmValue::Instance { class_name, fields: of, .. } if class_name == "DateTime" => {
                    let other = to_datetime(heap, *of)?;
                    Ok(mk_duration(dt()?.since(other).map_err(jiff_err)?))
                }
                VmValue::Instance { class_name, fields: of, .. } if class_name == "Duration" => {
                    let span = to_span(heap, *of)?;
                    Ok(mk_datetime(dt()?.checked_sub(span).map_err(jiff_err)?))
                }
                _ => Err(raise("DateTime.sub: argument must be a DateTime or Duration")),
            }
        }
        "before?" => {
            if args.len() != 1 {
                return Err(wrong_args("DateTime", "before?", 1, args.len(), line));
            }
            let other = datetime_from_arg(heap, &args[0], "DateTime.before?")?;
            Ok(VmValue::Bool(dt()? < other).into())
        }
        "after?" => {
            if args.len() != 1 {
                return Err(wrong_args("DateTime", "after?", 1, args.len(), line));
            }
            let other = datetime_from_arg(heap, &args[0], "DateTime.after?")?;
            Ok(VmValue::Bool(dt()? > other).into())
        }
        "equal?" => {
            if args.len() != 1 {
                return Err(wrong_args("DateTime", "equal?", 1, args.len(), line));
            }
            let other = datetime_from_arg(heap, &args[0], "DateTime.equal?")?;
            Ok(VmValue::Bool(dt()? == other).into())
        }
        "format" => {
            if args.len() != 1 {
                return Err(wrong_args("DateTime", "format", 1, args.len(), line));
            }
            let pat = str_arg(args, 0, "DateTime.format")?;
            Ok(VmValue::Str(dt()?.strftime(pat).to_string()).into())
        }
        "to_instant" => {
            // Treat as UTC civil datetime → instant
            let tz = TimeZone::UTC;
            let zoned = dt()?.to_zoned(tz).map_err(jiff_err)?;
            Ok(mk_instant(zoned.timestamp()))
        }
        "in_timezone" => {
            if args.len() != 1 {
                return Err(wrong_args("DateTime", "in_timezone", 1, args.len(), line));
            }
            let tz_name = str_arg(args, 0, "DateTime.in_timezone")?;
            let tz = TimeZone::get(tz_name).map_err(jiff_err)?;
            let zoned = tz.to_ambiguous_zoned(dt()?).compatible().map_err(jiff_err)?;
            Ok(mk_zoned(&zoned))
        }
        _ => Err(type_err(format!("DateTime has no method '{}'", method), line)),
    }
}

fn datetime_from_arg(
    heap: &GcHeap<HeapObject>,
    val: &VmValue,
    ctx: &str,
) -> Result<civil::DateTime, VmError> {
    match val {
        VmValue::Instance { class_name, fields, .. } if class_name == "DateTime" => {
            to_datetime(heap, *fields)
        }
        _ => Err(raise(format!("{}: argument must be a DateTime", ctx))),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ZonedDateTime
// ════════════════════════════════════════════════════════════════════════════

fn build_dt_args(args: &[VmValue], ctx: &str) -> Result<civil::DateTime, VmError> {
    civil::DateTime::new(
        int_arg(args, 0, ctx)? as i16,
        int_arg(args, 1, ctx)? as i8,
        int_arg(args, 2, ctx)? as i8,
        int_arg(args, 3, ctx)? as i8,
        int_arg(args, 4, ctx)? as i8,
        int_arg(args, 5, ctx)? as i8,
        0,
    )
    .map_err(jiff_err)
}

fn zoned_class(
    heap: &GcHeap<HeapObject>,
    method: &str,
    args: &[VmValue],
    line: u32,
) -> Result<DtValue, VmError> {
    match method {
        "now" => {
            if args.len() != 1 {
                return Err(wrong_args("ZonedDateTime", "now", 1, args.len(), line));
            }
            let tz_name = str_arg(args, 0, "ZonedDateTime.now")?;
            let tz = TimeZone::get(tz_name).map_err(jiff_err)?;
            Ok(mk_zoned(&Timestamp::now().to_zoned(tz)))
        }
        "of" => {
            // Raises on gaps AND folds.
            if args.len() != 7 {
                return Err(wrong_args("ZonedDateTime", "of", 7, args.len(), line));
            }
            let tz_name = str_arg(args, 6, "ZonedDateTime.of")?;
            let tz = TimeZone::get(tz_name).map_err(jiff_err)?;
            let dt = build_dt_args(args, "ZonedDateTime.of")?;
            let zoned = tz
                .to_ambiguous_zoned(dt)
                .disambiguate(Disambiguation::Reject)
                .map_err(jiff_err)?;
            Ok(mk_zoned(&zoned))
        }
        "of_compatible" => {
            // For unambiguous times or when you want compatible resolution.
            if args.len() != 7 {
                return Err(wrong_args("ZonedDateTime", "of_compatible", 7, args.len(), line));
            }
            let tz_name = str_arg(args, 6, "ZonedDateTime.of_compatible")?;
            let tz = TimeZone::get(tz_name).map_err(jiff_err)?;
            let dt = build_dt_args(args, "ZonedDateTime.of_compatible")?;
            let zoned = tz.to_ambiguous_zoned(dt).compatible().map_err(jiff_err)?;
            Ok(mk_zoned(&zoned))
        }
        "of_before" => {
            // Fold → first occurrence (before the clock falls back). Gap → error.
            if args.len() != 7 {
                return Err(wrong_args("ZonedDateTime", "of_before", 7, args.len(), line));
            }
            let tz_name = str_arg(args, 6, "ZonedDateTime.of_before")?;
            let tz = TimeZone::get(tz_name).map_err(jiff_err)?;
            let dt = build_dt_args(args, "ZonedDateTime.of_before")?;
            let zoned = tz.to_ambiguous_zoned(dt).earlier().map_err(jiff_err)?;
            Ok(mk_zoned(&zoned))
        }
        "of_after" => {
            // Fold → second occurrence (after the clock falls back). Gap → error.
            if args.len() != 7 {
                return Err(wrong_args("ZonedDateTime", "of_after", 7, args.len(), line));
            }
            let tz_name = str_arg(args, 6, "ZonedDateTime.of_after")?;
            let tz = TimeZone::get(tz_name).map_err(jiff_err)?;
            let dt = build_dt_args(args, "ZonedDateTime.of_after")?;
            let zoned = tz.to_ambiguous_zoned(dt).later().map_err(jiff_err)?;
            Ok(mk_zoned(&zoned))
        }
        "from_instant" => {
            if args.len() != 2 {
                return Err(wrong_args("ZonedDateTime", "from_instant", 2, args.len(), line));
            }
            let ts = match &args[0] {
                VmValue::Instance { class_name, fields, .. } if class_name == "Instant" => {
                    to_timestamp(heap, *fields)?
                }
                _ => return Err(raise("ZonedDateTime.from_instant: first arg must be Instant")),
            };
            let tz_name = str_arg(args, 1, "ZonedDateTime.from_instant")?;
            let tz = TimeZone::get(tz_name).map_err(jiff_err)?;
            Ok(mk_zoned(&ts.to_zoned(tz)))
        }
        "parse" => {
            if args.len() != 1 {
                return Err(wrong_args("ZonedDateTime", "parse", 1, args.len(), line));
            }
            let s = str_arg(args, 0, "ZonedDateTime.parse")?;
            Ok(mk_zoned(&s.parse::<Zoned>().map_err(jiff_err)?))
        }
        _ => Err(type_err(format!("ZonedDateTime has no class method '{}'", method), line)),
    }
}

fn zoned_instance(
    heap: &GcHeap<HeapObject>,
    fields: GcRef,
    method: &str,
    args: &[VmValue],
    line: u32,
) -> Result<DtValue, VmError> {
    let f = flds(heap, fields);
    let zdt = || to_zoned(heap, fields);
    match method {
        "to_s" | "inspect" => Ok(VmValue::Str(zdt()?.to_string()).into()),
        "year" => Ok(VmValue::Int(zdt()?.year() as i64).into()),
        "month" => Ok(VmValue::Int(zdt()?.month() as i64).into()),
        "day" => Ok(VmValue::Int(zdt()?.day() as i64).into()),
        "hour" => Ok(VmValue::Int(zdt()?.hour() as i64).into()),
        "minute" => Ok(VmValue::Int(zdt()?.minute() as i64).into()),
        "second" => Ok(VmValue::Int(zdt()?.second() as i64).into()),
        "nano" => Ok(VmValue::Int(zdt()?.subsec_nanosecond() as i64).into()),
        "timezone" => Ok(VmValue::Str(get_str(f, "_tz").to_owned()).into()),
        "epoch_seconds" => Ok(VmValue::Int(get_int(f, "_secs")).into()),
        "epoch_millis" => Ok(VmValue::Int(zdt()?.timestamp().as_millisecond()).into()),
        "date" => Ok(mk_date(zdt()?.date())),
        "time" => Ok(mk_time(zdt()?.time())),
        "datetime" => Ok(mk_datetime(zdt()?.datetime())),
        "to_instant" => Ok(mk_instant(zdt()?.timestamp())),
        "to_utc" => {
            let ts = zdt()?.timestamp();
            let tz = TimeZone::UTC;
            Ok(mk_zoned(&ts.to_zoned(tz)))
        }
        "with_timezone" => {
            if args.len() != 1 {
                return Err(wrong_args("ZonedDateTime", "with_timezone", 1, args.len(), line));
            }
            let tz_name = str_arg(args, 0, "ZonedDateTime.with_timezone")?;
            let tz = TimeZone::get(tz_name).map_err(jiff_err)?;
            let ts = zdt()?.timestamp();
            Ok(mk_zoned(&ts.to_zoned(tz)))
        }
        "add" => {
            if args.len() != 1 {
                return Err(wrong_args("ZonedDateTime", "add", 1, args.len(), line));
            }
            let span = span_from_val(heap, &args[0])?;
            Ok(mk_zoned(&zdt()?.checked_add(span).map_err(jiff_err)?))
        }
        "sub" => {
            if args.len() != 1 {
                return Err(wrong_args("ZonedDateTime", "sub", 1, args.len(), line));
            }
            let z = zdt()?;
            match &args[0] {
                VmValue::Instance { class_name, fields: of, .. }
                    if class_name == "ZonedDateTime" =>
                {
                    let other = to_zoned(heap, *of)?;
                    Ok(mk_duration(z.since(&other).map_err(jiff_err)?))
                }
                VmValue::Instance { class_name, fields: of, .. } if class_name == "Duration" => {
                    let span = to_span(heap, *of)?;
                    Ok(mk_zoned(&z.checked_sub(span).map_err(jiff_err)?))
                }
                _ => Err(raise("ZonedDateTime.sub: argument must be ZonedDateTime or Duration")),
            }
        }
        "before?" => {
            if args.len() != 1 {
                return Err(wrong_args("ZonedDateTime", "before?", 1, args.len(), line));
            }
            let other = zoned_from_arg(heap, &args[0], "ZonedDateTime.before?")?;
            Ok(VmValue::Bool(zdt()?.timestamp() < other.timestamp()).into())
        }
        "after?" => {
            if args.len() != 1 {
                return Err(wrong_args("ZonedDateTime", "after?", 1, args.len(), line));
            }
            let other = zoned_from_arg(heap, &args[0], "ZonedDateTime.after?")?;
            Ok(VmValue::Bool(zdt()?.timestamp() > other.timestamp()).into())
        }
        "equal?" => {
            if args.len() != 1 {
                return Err(wrong_args("ZonedDateTime", "equal?", 1, args.len(), line));
            }
            let other = zoned_from_arg(heap, &args[0], "ZonedDateTime.equal?")?;
            Ok(VmValue::Bool(zdt()?.timestamp() == other.timestamp()).into())
        }
        "format" => {
            if args.len() != 1 {
                return Err(wrong_args("ZonedDateTime", "format", 1, args.len(), line));
            }
            let pat = str_arg(args, 0, "ZonedDateTime.format")?;
            Ok(VmValue::Str(zdt()?.strftime(pat).to_string()).into())
        }
        _ => Err(type_err(format!("ZonedDateTime has no method '{}'", method), line)),
    }
}

fn zoned_from_arg(
    heap: &GcHeap<HeapObject>,
    val: &VmValue,
    ctx: &str,
) -> Result<Zoned, VmError> {
    match val {
        VmValue::Instance { class_name, fields, .. } if class_name == "ZonedDateTime" => {
            to_zoned(heap, *fields)
        }
        _ => Err(raise(format!("{}: argument must be a ZonedDateTime", ctx))),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Duration (maps to jiff::Span)
// ════════════════════════════════════════════════════════════════════════════

fn duration_class(method: &str, args: &[VmValue], line: u32) -> Result<DtValue, VmError> {
    match method {
        "of_years" => {
            if args.len() != 1 {
                return Err(wrong_args("Duration", "of_years", 1, args.len(), line));
            }
            Ok(mk_duration(Span::new().years(int_arg(args, 0, "Duration.of_years")?)))
        }
        "of_months" => {
            if args.len() != 1 {
                return Err(wrong_args("Duration", "of_months", 1, args.len(), line));
            }
            Ok(mk_duration(Span::new().months(int_arg(args, 0, "Duration.of_months")?)))
        }
        "of_days" => {
            if args.len() != 1 {
                return Err(wrong_args("Duration", "of_days", 1, args.len(), line));
            }
            Ok(mk_duration(Span::new().days(int_arg(args, 0, "Duration.of_days")?)))
        }
        "of_hours" => {
            if args.len() != 1 {
                return Err(wrong_args("Duration", "of_hours", 1, args.len(), line));
            }
            Ok(mk_duration(Span::new().hours(int_arg(args, 0, "Duration.of_hours")?)))
        }
        "of_minutes" => {
            if args.len() != 1 {
                return Err(wrong_args("Duration", "of_minutes", 1, args.len(), line));
            }
            Ok(mk_duration(Span::new().minutes(int_arg(args, 0, "Duration.of_minutes")?)))
        }
        "of_seconds" => {
            if args.len() != 1 {
                return Err(wrong_args("Duration", "of_seconds", 1, args.len(), line));
            }
            Ok(mk_duration(Span::new().seconds(int_arg(args, 0, "Duration.of_seconds")?)))
        }
        "of_nanos" => {
            if args.len() != 1 {
                return Err(wrong_args("Duration", "of_nanos", 1, args.len(), line));
            }
            Ok(mk_duration(Span::new().nanoseconds(int_arg(args, 0, "Duration.of_nanos")?)))
        }
        "of" => {
            if args.len() != 7 {
                return Err(wrong_args("Duration", "of", 7, args.len(), line));
            }
            Ok(mk_duration(span_from_components(
                int_arg(args, 0, "Duration.of")?,
                int_arg(args, 1, "Duration.of")?,
                int_arg(args, 2, "Duration.of")?,
                int_arg(args, 3, "Duration.of")?,
                int_arg(args, 4, "Duration.of")?,
                int_arg(args, 5, "Duration.of")?,
                int_arg(args, 6, "Duration.of")?,
            )))
        }
        _ => Err(type_err(format!("Duration has no class method '{}'", method), line)),
    }
}

fn duration_instance(
    heap: &GcHeap<HeapObject>,
    fields: GcRef,
    method: &str,
    args: &[VmValue],
    line: u32,
) -> Result<DtValue, VmError> {
    let f = flds(heap, fields);
    let sp = || to_span(heap, fields);
    match method {
        "to_s" | "inspect" => Ok(VmValue::Str(sp()?.to_string()).into()),
        "years" => Ok(VmValue::Int(get_int(f, "_years")).into()),
        "months" => Ok(VmValue::Int(get_int(f, "_months")).into()),
        "days" => Ok(VmValue::Int(get_int(f, "_days")).into()),
        "hours" => Ok(VmValue::Int(get_int(f, "_hours")).into()),
        "minutes" => Ok(VmValue::Int(get_int(f, "_minutes")).into()),
        "seconds" => Ok(VmValue::Int(get_int(f, "_secs")).into()),
        "nanos" => Ok(VmValue::Int(get_int(f, "_ns")).into()),
        "negate" => Ok(mk_duration(-sp()?)),
        "abs" => {
            let s = sp()?;
            let neg = s.get_years() < 0
                || s.get_months() < 0
                || s.get_days() < 0
                || s.get_hours() < 0
                || s.get_minutes() < 0
                || s.get_seconds() < 0
                || s.get_nanoseconds() < 0;
            Ok(mk_duration(if neg { -s } else { s }))
        }
        "add" => {
            if args.len() != 1 {
                return Err(wrong_args("Duration", "add", 1, args.len(), line));
            }
            let other = span_from_val(heap, &args[0])?;
            // Field-wise addition: combine each component
            let a = sp()?;
            let combined = Span::new()
                .years(a.get_years() as i64 + other.get_years() as i64)
                .months(a.get_months() as i64 + other.get_months() as i64)
                .days(a.get_days() as i64 + other.get_days() as i64)
                .hours(a.get_hours() as i64 + other.get_hours() as i64)
                .minutes(a.get_minutes() + other.get_minutes())
                .seconds(a.get_seconds() + other.get_seconds())
                .nanoseconds(a.get_nanoseconds() + other.get_nanoseconds());
            Ok(mk_duration(combined))
        }
        _ => Err(type_err(format!("Duration has no method '{}'", method), line)),
    }
}
