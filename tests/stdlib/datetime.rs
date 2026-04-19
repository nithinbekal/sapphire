use super::{VmValue, eval};

// ── helpers ──────────────────────────────────────────────────────────────────

fn eval_int(src: &str) -> i64 {
    match eval(src) {
        VmValue::Int(n) => n,
        other => panic!("expected Int, got {:?}", other),
    }
}

fn eval_bool(src: &str) -> bool {
    match eval(src) {
        VmValue::Bool(b) => b,
        other => panic!("expected Bool, got {:?}", other),
    }
}

fn eval_str(src: &str) -> String {
    match eval(src) {
        VmValue::Str(s) => s,
        other => panic!("expected Str, got {:?}", other),
    }
}

// ── Instant ───────────────────────────────────────────────────────────────────

#[test]
fn instant_of_epoch_seconds() {
    assert_eq!(eval_int("Instant.of_epoch_seconds(0).epoch_seconds()"), 0);
    assert_eq!(eval_int("Instant.of_epoch_seconds(3600).epoch_seconds()"), 3600);
}

#[test]
fn instant_of_epoch_millis() {
    assert_eq!(eval_int("Instant.of_epoch_millis(5000).epoch_millis()"), 5000);
}

#[test]
fn instant_parse_roundtrip() {
    let s = eval_str(r#"Instant.parse("1970-01-01T00:00:00Z").to_s()"#);
    assert!(s.contains("1970-01-01"), "got: {}", s);
}

#[test]
fn instant_add_sub_duration() {
    assert_eq!(
        eval_int(
            "d = Duration.of_hours(2)
             t = Instant.of_epoch_seconds(0)
             t.add(d).epoch_seconds()"
        ),
        7200
    );
    assert_eq!(
        eval_int(
            "d = Duration.of_hours(1)
             t = Instant.of_epoch_seconds(7200)
             t.sub(d).epoch_seconds()"
        ),
        3600
    );
}

#[test]
fn instant_sub_instant_gives_duration() {
    assert_eq!(
        eval_int(
            "a = Instant.of_epoch_seconds(7200)
             b = Instant.of_epoch_seconds(3600)
             a.sub(b).hours()"
        ),
        1
    );
}

#[test]
fn instant_before_after() {
    assert_eq!(
        eval_bool(
            "a = Instant.of_epoch_seconds(0)
             b = Instant.of_epoch_seconds(1)
             a.before?(b)"
        ),
        true
    );
    assert_eq!(
        eval_bool(
            "a = Instant.of_epoch_seconds(1)
             b = Instant.of_epoch_seconds(0)
             a.after?(b)"
        ),
        true
    );
}

#[test]
fn instant_in_timezone() {
    let s = eval_str(
        r#"Instant.of_epoch_seconds(0).in_timezone("UTC").to_s()"#,
    );
    assert!(s.contains("1970"), "got: {}", s);
}

// ── Duration ──────────────────────────────────────────────────────────────────

#[test]
fn duration_of_hours() {
    assert_eq!(eval_int("Duration.of_hours(3).hours()"), 3);
}

#[test]
fn duration_of_days() {
    assert_eq!(eval_int("Duration.of_days(7).days()"), 7);
}

#[test]
fn duration_negate() {
    assert_eq!(eval_int("Duration.of_hours(5).negate().hours()"), -5);
}

#[test]
fn duration_add() {
    assert_eq!(
        eval_int("Duration.of_hours(3).add(Duration.of_hours(2)).hours()"),
        5
    );
}

#[test]
fn duration_of_full() {
    assert_eq!(eval_int("Duration.of(1, 2, 3, 4, 5, 6, 0).years()"), 1);
    assert_eq!(eval_int("Duration.of(1, 2, 3, 4, 5, 6, 0).months()"), 2);
    assert_eq!(eval_int("Duration.of(1, 2, 3, 4, 5, 6, 0).days()"), 3);
    assert_eq!(eval_int("Duration.of(1, 2, 3, 4, 5, 6, 0).hours()"), 4);
}

// ── Date ──────────────────────────────────────────────────────────────────────

#[test]
fn date_of() {
    assert_eq!(eval_int("Date.of(2024, 3, 15).year()"), 2024);
    assert_eq!(eval_int("Date.of(2024, 3, 15).month()"), 3);
    assert_eq!(eval_int("Date.of(2024, 3, 15).day()"), 15);
}

#[test]
fn date_parse() {
    assert_eq!(eval_int(r#"Date.parse("2024-06-01").month()"#), 6);
}

#[test]
fn date_day_of_week() {
    // 2024-01-01 is a Monday → 1
    assert_eq!(eval_int("Date.of(2024, 1, 1).day_of_week()"), 1);
}

#[test]
fn date_add_duration() {
    assert_eq!(
        eval_int("Date.of(2024, 1, 1).add(Duration.of_days(10)).day()"),
        11
    );
}

#[test]
fn date_sub_duration() {
    assert_eq!(
        eval_int("Date.of(2024, 1, 11).sub(Duration.of_days(10)).day()"),
        1
    );
}

#[test]
fn date_sub_date_gives_duration() {
    // 10 day difference
    assert_eq!(
        eval_int(
            "a = Date.of(2024, 1, 11)
             b = Date.of(2024, 1, 1)
             a.sub(b).days()"
        ),
        10
    );
}

#[test]
fn date_before_after() {
    assert_eq!(
        eval_bool(
            "Date.of(2023, 1, 1).before?(Date.of(2024, 1, 1))"
        ),
        true
    );
    assert_eq!(
        eval_bool(
            "Date.of(2024, 1, 1).after?(Date.of(2023, 1, 1))"
        ),
        true
    );
}

#[test]
fn date_next_prev() {
    assert_eq!(eval_int("Date.of(2024, 1, 31).next_day().month()"), 2);
    assert_eq!(eval_int("Date.of(2024, 2, 1).prev_day().month()"), 1);
}

// ── Time ──────────────────────────────────────────────────────────────────────

#[test]
fn time_of() {
    assert_eq!(eval_int("Time.of(14, 30, 0).hour()"), 14);
    assert_eq!(eval_int("Time.of(14, 30, 0).minute()"), 30);
    assert_eq!(eval_int("Time.of(14, 30, 0).second()"), 0);
}

#[test]
fn time_before_after() {
    assert_eq!(
        eval_bool("Time.of(8, 0, 0).before?(Time.of(12, 0, 0))"),
        true
    );
    assert_eq!(
        eval_bool("Time.of(12, 0, 0).after?(Time.of(8, 0, 0))"),
        true
    );
}

// ── DateTime ──────────────────────────────────────────────────────────────────

#[test]
fn datetime_of() {
    assert_eq!(eval_int("DateTime.of(2024, 6, 15, 10, 30, 0).year()"), 2024);
    assert_eq!(eval_int("DateTime.of(2024, 6, 15, 10, 30, 0).month()"), 6);
    assert_eq!(eval_int("DateTime.of(2024, 6, 15, 10, 30, 0).hour()"), 10);
}

#[test]
fn datetime_date_time() {
    assert_eq!(
        eval_int("DateTime.of(2024, 6, 15, 10, 30, 0).date().day()"),
        15
    );
    assert_eq!(
        eval_int("DateTime.of(2024, 6, 15, 10, 30, 0).time().minute()"),
        30
    );
}

#[test]
fn datetime_add_sub() {
    assert_eq!(
        eval_int(
            "dt = DateTime.of(2024, 1, 1, 0, 0, 0)
             dt.add(Duration.of_days(1)).day()"
        ),
        2
    );
}

// ── ZonedDateTime ─────────────────────────────────────────────────────────────

#[test]
fn zoned_now_utc() {
    let s = eval_str(r#"ZonedDateTime.now("UTC").to_s()"#);
    assert!(s.contains("UTC") || s.contains("+00"), "got: {}", s);
}

#[test]
fn zoned_from_instant() {
    let s = eval_str(
        r#"i = Instant.of_epoch_seconds(0)
           ZonedDateTime.from_instant(i, "UTC").to_s()"#,
    );
    assert!(s.contains("1970"), "got: {}", s);
}

#[test]
fn zoned_to_instant_roundtrip() {
    assert_eq!(
        eval_int(
            r#"i = Instant.of_epoch_seconds(86400)
               i2 = ZonedDateTime.from_instant(i, "UTC").to_instant()
               i2.epoch_seconds()"#
        ),
        86400
    );
}

#[test]
fn zoned_add_duration_dst_aware() {
    // 2024-03-10 01:30 EST + 1 hour → 03:30 EDT (skips 2am gap)
    let h = eval_int(
        r#"z = ZonedDateTime.of_compatible(2024, 3, 10, 1, 30, 0, "America/New_York")
           z.add(Duration.of_hours(1)).hour()"#,
    );
    assert_eq!(h, 3);
}

#[test]
fn zoned_spring_forward_gap_raises() {
    use sapphire::compiler::compile;
    use sapphire::lexer::Lexer;
    use sapphire::parser::Parser;
    use sapphire::vm::{Vm, VmError};

    let src = r#"ZonedDateTime.of(2024, 3, 10, 2, 30, 0, "America/New_York")"#;
    let tokens = Lexer::new(src).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse");
    let func = compile(&stmts).expect("compile");
    let mut vm = Vm::new(func, std::path::PathBuf::new());
    vm.load_stdlib().expect("stdlib");
    assert!(
        matches!(vm.run(), Err(VmError::Raised(_))),
        "should raise on impossible time in spring-forward gap"
    );
}

#[test]
fn zoned_fall_back_before() {
    // 2024-11-03 01:30 in fold → of_before → EDT (-04)
    let s = eval_str(
        r#"ZonedDateTime.of_before(2024, 11, 3, 1, 30, 0, "America/New_York").to_s()"#,
    );
    assert!(s.contains("-04:00"), "expected EDT offset, got: {}", s);
}

#[test]
fn zoned_fall_back_after() {
    // 2024-11-03 01:30 in fold → of_after → EST (-05)
    let s = eval_str(
        r#"ZonedDateTime.of_after(2024, 11, 3, 1, 30, 0, "America/New_York").to_s()"#,
    );
    assert!(s.contains("-05:00"), "expected EST offset, got: {}", s);
}

#[test]
fn zoned_with_timezone() {
    let s = eval_str(
        r#"ZonedDateTime.now("UTC").with_timezone("America/Chicago").timezone()"#,
    );
    assert_eq!(s, "America/Chicago");
}
