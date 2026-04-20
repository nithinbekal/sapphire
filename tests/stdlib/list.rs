use super::{VmValue, eval};

#[test]
fn size() {
    assert_eq!(eval("a = [1,2,3]\na.size()"), VmValue::Int(3));
    assert_eq!(eval("[].size()"), VmValue::Int(0));
}

#[test]
fn first_last() {
    assert_eq!(eval("a = [1,2,3]\na.first()"), VmValue::Int(1));
    assert_eq!(eval("a = [1,2,3]\na.last()"), VmValue::Int(3));
}

#[test]
fn empty() {
    assert_eq!(eval("[].empty?()"), VmValue::Bool(true));
    assert_eq!(eval("[1,2].empty?()"), VmValue::Bool(false));
}

#[test]
fn include() {
    assert_eq!(eval("a = [1,2,3]\na.include?(2)"), VmValue::Bool(true));
    assert_eq!(eval("a = [1,2,3]\na.include?(5)"), VmValue::Bool(false));
}

#[test]
fn sort() {
    assert_eq!(eval("a = [3,1,2]\na.sort().first()"), VmValue::Int(1));
    assert_eq!(
        eval("result = [3, 1, 4, 1, 5, 9, 2].sort()\nresult[0]"),
        VmValue::Int(1)
    );
    assert_eq!(
        eval("result = [3, 1, 4, 1, 5, 9, 2].sort()\nresult[6]"),
        VmValue::Int(9)
    );
}

#[test]
fn sort_strings() {
    assert_eq!(
        eval(r#"result = ["banana", "apple", "cherry"].sort(); result[0]"#),
        VmValue::Str("apple".into())
    );
    assert_eq!(
        eval(r#"result = ["banana", "apple", "cherry"].sort(); result[2]"#),
        VmValue::Str("cherry".into())
    );
}

#[test]
fn join() {
    assert_eq!(eval(r#"[1,2,3].join(",")"#), VmValue::Str("1,2,3".into()));
}

#[test]
fn push_pop() {
    assert_eq!(eval("a = [1,2]\na.append(3)\na.size()"), VmValue::Int(3));
    assert_eq!(eval("a = [1,2,3]\na.pop()"), VmValue::Int(3));
    assert_eq!(eval("a = [1,2,3]\na.pop()\na.size()"), VmValue::Int(2));
}

#[test]
fn each() {
    let src = "a = [1,2,3]\nsum = 0\na.each() { |x| sum = sum + x }\nsum";
    assert_eq!(eval(src), VmValue::Int(6));
}

#[test]
fn reduce_with_initial() {
    let src = "[1, 2, 3, 4, 5].reduce(0) { |acc, n| acc + n }";
    assert_eq!(eval(src), VmValue::Int(15));
}

#[test]
fn reduce_without_initial() {
    let src = "[1, 2, 3, 4, 5].reduce() { |acc, n| acc * n }";
    assert_eq!(eval(src), VmValue::Int(120));
}

#[test]
fn flatten() {
    assert_eq!(
        eval("result = [[1, 2], [3, [4, 5]]].flatten()\nresult.size()"),
        VmValue::Int(5)
    );
    assert_eq!(
        eval("result = [[1, 2], [3, [4, 5]]].flatten()\nresult[3]"),
        VmValue::Int(4)
    );
}

#[test]
fn uniq() {
    let src = "result = [1, 2, 2, 3, 1].uniq()\nresult.size()";
    assert_eq!(eval(src), VmValue::Int(3));
}

