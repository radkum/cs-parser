//! Hostile inputs: every case must return (Ok or Err) quickly, without
//! panicking, overflowing the stack, hanging or exhausting memory.

use std::{collections::BTreeSet, sync::mpsc, thread, time::Duration};

use cs_parser::{CSharpSession, Variables};

const TIMEOUT: Duration = Duration::from_secs(60);

struct Outcome {
    strings: BTreeSet<String>,
    errors: Vec<String>,
}

fn guarded<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(TIMEOUT) {
        Ok(v) => v,
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("hung for more than {TIMEOUT:?}"),
        Err(mpsc::RecvTimeoutError::Disconnected) => panic!("evaluation panicked"),
    }
}

fn run_program(src: String) -> Result<Outcome, String> {
    guarded(move || {
        CSharpSession::new()
            .parse_input(&src)
            .map(|r| Outcome {
                strings: r.tokens().string_set(),
                errors: r.errors().iter().map(|e| e.to_string()).collect(),
            })
            .map_err(|e| e.to_string())
    })
}

fn run(members: &str) -> Result<Outcome, String> {
    run_program(format!(
        "namespace N {{ public static class M {{\n{members}\n}} }}"
    ))
}

fn run_body(body: &str) -> Result<Outcome, String> {
    run(&format!(
        "static string F() {{ {body} }} static string R = M.F();"
    ))
}

fn run_statements(src: String) -> Result<String, String> {
    guarded(move || {
        CSharpSession::new()
            .safe_eval_statements(&src)
            .map(|v| v.to_string())
            .map_err(|e| e.to_string())
    })
}

fn nest(open: &str, inner: &str, close: &str, n: usize) -> String {
    format!("{}{inner}{}", open.repeat(n), close.repeat(n))
}

// ---------------------------------------------------------------- public API

#[test]
fn deobfuscate_script_on_fresh_session() {
    guarded(|| {
        let _ = CSharpSession::new().deobfuscate_script("namespace A { }");
        let _ = CSharpSession::new().deobfuscate_script("int a = 1;");
    });
}

#[test]
fn deobfuscate_script_after_safe_eval() {
    guarded(|| {
        let mut s = CSharpSession::new();
        let _ = s.safe_eval_statements("int a = 1;");
        let _ = s.deobfuscate_script("class C { }");
        let _ = s.deobfuscate_script("class C { }");
        let _ = s.parse_input("class C { }");
    });
}

#[test]
fn variables_from_ini() {
    guarded(|| {
        let vars = Variables::from_ini_string("[static]\na=1\nb=text").unwrap();
        let _ = CSharpSession::new()
            .with_variables(vars)
            .parse_input("class C { }");
    });
}

#[test]
fn run_is_not_a_panic() {
    guarded(|| {
        assert!(CSharpSession::new().run().is_err());
    });
}

#[test]
fn malformed_input() {
    for src in [
        "",
        "namespace",
        "namespace A {",
        "}}}}",
        "static int X = 1; /* unterminated",
        "\u{feff}\u{0}\u{1}garbage{{{",
        "class C { static string X = \"unterminated; }",
        "żółć ąę 日本語 🦀",
    ] {
        let _ = run_program(src.to_string());
        let _ = run_statements(src.to_string());
    }
}

// --------------------------------------------------------- syntax that
// panicked

#[test]
fn enum_with_body() {
    run_program("namespace N { enum E { A = 1, B = 2 } }".into()).unwrap();
    run_program("namespace N { class C { enum E { A = 1 } } }".into()).unwrap();
}

#[test]
fn array_creation() {
    for body in [
        "int[] a = new int[] { 1, 2 }; return \"x\";",
        "var a = new int[3]; return \"x\";",
        "int[5] a; return \"x\";",
        "[] a; return \"x\";",
        "var a = new [] { 1 }; return \"x\";",
        "var a = new object[] { }; return \"x\";",
        "var a = new object[0]; return \"x\";",
        "var a = new int(); return \"x\";",
        "var a = new string(\"a\"); return \"x\";",
        "var a = new string('a', 3); return \"x\";",
        "int[] a = new int[] { 1, 2 }; return \"\" + a[5];",
        "int[] a = new int[] { 1, 2 }; return \"\" + a[-1];",
    ] {
        let _ = run_body(body);
    }
}

#[test]
fn method_call_without_arguments() {
    let _ = run_body("string a = \"x\"; a.ToString(); return a;");
    let _ = run_statements("string a = \"x\"; a.ToString();".into());
}

#[test]
fn number_literals() {
    for lit in [
        "0x1_0",
        "0x10000000000000000",
        "0xFFFFFFFFFFFFFFFF",
        "0b101",
        "0b1_0",
        "1_000",
        "99999999999999999999999999999",
        "9223372036854775807m",
        "9223372036854775807p",
        "1e400",
        "-1e400",
    ] {
        let _ = run(&format!(
            "static long X = {lit}; static string S = \"v\" + {lit};"
        ));
    }
}

#[test]
fn hash_literal_with_dictionary_key() {
    let _ = run_body("var h = @{ [\"a\"] = 1 }; return \"x\";");
}

#[test]
fn format_operator() {
    for fmt in [
        r#""é" -f 1"#,
        r#""żółć {0}" -f 1"#,
        r#""{0:N99999}" -f 1.5"#,
        r#""{0,4000000000}" -f 1"#,
        r#""{0,18446744073709551615}" -f 1"#,
        r#""{0:0}" -f (-9223372036854775807 - 1)"#,
        r#""{99999999999999999999}" -f 1"#,
        r#"string.Format("{0,2000000000}", "a")"#,
    ] {
        let _ = run(&format!("static string X = {fmt};"));
    }
}

// ------------------------------------------------------------------ arithmetic

#[test]
fn integer_overflow() {
    for body in [
        "long a = 9223372036854775807; long b = a + 1; return \"\" + b;",
        "long a = -9223372036854775807; long b = a - 10; return \"\" + b;",
        "long a = 9223372036854775807; long b = a * 2; return \"\" + b;",
        "long a = 9223372036854775807; ++a; return \"\" + a;",
        "long a = 9223372036854775807; a++; return \"\" + a;",
        "long a = -9223372036854775807; --a; --a; return \"\" + a;",
        "long a = 9223372036854775807; a += 1; a *= 2; return \"\" + a;",
        "long a = -9223372036854775807 - 1; return \"\" + (a / -1);",
        "long a = -9223372036854775807 - 1; return \"\" + (a % -1);",
        "long a = -9223372036854775807 - 1; return \"\" + Math.Abs(a);",
        "long a = -9223372036854775807 - 1; return \"\" + (-a);",
        "long a = -9223372036854775807 - 1; return \"\" + (~a);",
    ] {
        let _ = run_body(body);
    }
}

#[test]
fn division_by_zero() {
    for expr in [
        "1 / 0",
        "1 % 0",
        "5 / \"0.4\"",
        "5 % \"0.4\"",
        "5 / \"0x0\"",
        "5 % \"0x0\"",
        "1.5 / 0",
        "1.5 % 0",
    ] {
        let _ = run(&format!("static string X = \"v\" + ({expr});"));
    }
}

#[test]
fn shifts() {
    for expr in [
        "1 << 64", "1 << 200", "1 >> 64", "1 >> -1", "1 << -1", "-1 >> 70",
    ] {
        let _ = run(&format!("static string X = \"v\" + ({expr});"));
    }
}

#[test]
fn comparisons_of_unsupported_types() {
    for body in [
        "var a = \"abc\".split(\"b\"); bool b = a > 1; return \"x\";",
        "var a = \"abc\".split(\"b\"); bool b = a < 1; return \"x\";",
        "StringBuilder b = new StringBuilder(\"a\"); bool c = b > 1; return \"x\";",
        "StringBuilder b = new StringBuilder(\"a\"); bool c = b < 1; return \"x\";",
    ] {
        let _ = run_body(body);
    }
}

// --------------------------------------------------------------------- strings

#[test]
fn string_methods_with_bad_indices() {
    for call in [
        "\"ąbc\".substring(1)",
        "\"ąbc\".substring(1, 1)",
        "\"abc\".substring(5)",
        "\"abc\".substring(1, 10)",
        "\"abc\".substring(-1)",
        "\"abc\".substring(1, -1)",
        "\"abc\".substring(-1, 2)",
        "\"ąbc\".remove(1)",
        "\"abc\".remove(10)",
        "\"abc\".remove(-1)",
        "\"ąbc\".insert(1, \"x\")",
        "\"abc\".insert(10, \"x\")",
        "\"abc\".insert(-1, \"x\")",
        "\"żółć\".indexof(\"ć\", 10)",
        "\"żółć\".trimstart('ż')",
        "\"abc\".tochararray()[9]",
        "\"abc\".padleft(-1)",
        "\"abc\".padright(-5, 'x')",
        "\"abc\".replace(\"\", \"x\")",
        "\"abc\".split(\"\")",
        "\"abc\".split(\"b\", -1)",
        "\"ą\".normalize()",
    ] {
        let _ = run(&format!("static string X = {call};"));
    }
}

#[test]
fn split_with_empty_separator_and_huge_count() {
    let _ = run_body("string s = \"abc\"; var a = s.split(\"\", 100000000000); return \"x\";");
}

#[test]
fn string_builder_bad_capacity() {
    for cap in ["-1", "4000000000000000000", "9223372036854775807"] {
        let _ = run_body(&format!(
            "StringBuilder b = new StringBuilder({cap}); b.Append('a'); return b.ToString();"
        ));
    }
}

// ----------------------------------------------------------------- recursion

#[test]
fn direct_recursion() {
    let out =
        run(r#"static string F(string s) { return M.F(s + "a"); } static string R = M.F("Q");"#);
    let _ = out;
}

#[test]
fn mutual_recursion() {
    let _ = run(r#"
        static string A(string s) { return M.B(s); }
        static string B(string s) { return M.A(s); }
        static string R = M.A("x");"#);
}

#[test]
fn recursion_at_definition_time() {
    let _ = run_program(
        "class C { static int f(int x) { return C.f(x); } } class D { static void g() { int r = \
         C.f(1); } }"
            .into(),
    );
}

#[test]
fn recursion_in_statements() {
    let _ =
        run_statements("class C { static int f(int x) { return C.f(x); } } int r = C.f(1);".into());
    let _ = run_statements(
        "int f(int x) { string g = \"f\"; return g(x); } string g = \"f\"; int r = g(1);".into(),
    );
}

#[test]
fn recursion_with_fan_out() {
    let _ =
        run(r#"static string F(string s) { return M.F(s) + M.F(s); } static string R = M.F("x");"#);
}

#[test]
fn deep_call_chain() {
    let mut members = String::new();
    for i in 0..2000 {
        members.push_str(&format!(
            "static string F{i}(string s) {{ return M.F{}(s); }} ",
            i + 1
        ));
    }
    members.push_str("static string F2000(string s) { return s; } static string R = M.F0(\"x\");");
    let _ = run(&members);
}

#[test]
fn exponential_call_tree() {
    let mut members = String::new();
    for i in 1..=40 {
        members.push_str(&format!(
            "static string F{i}(string s) {{ return M.F{n}(s) + M.F{n}(s); }} ",
            n = i + 1
        ));
    }
    members.push_str("static string F41(string s) { return s; } static string R = M.F1(\"a\");");
    let _ = run(&members);
}

// ------------------------------------------------------------- deep nesting

#[test]
fn deeply_nested_parentheses() {
    for n in [30, 100, 1_000, 100_000] {
        let _ = run(&format!(
            "static string X = {};",
            nest("(", "\"a\"", ")", n)
        ));
    }
}

#[test]
fn deeply_nested_calls() {
    let f = "static string F(string s) { return s; }";
    for n in [30, 100, 1_000, 50_000] {
        let _ = run(&format!(
            "{f} static string X = {};",
            nest("M.F(", "\"a\"", ")", n)
        ));
        let _ = run(&format!(
            "static string X = {};",
            nest("M.G(", "\"a\"", ")", n)
        ));
    }
}

#[test]
fn mixed_nesting_past_the_limit() {
    // combinations that made the parser backtrack exponentially
    let kinds: &[&[(&str, &str)]] = &[
        &[("(int)(", ")")],
        &[("(int)((int)", ")")],
        &[("a[", "]"), ("(a)(", ")"), ("new object[] {", "}")],
        &[
            ("(char)((int)", " - 1)"),
            ("f(a, ", ")"),
            ("new object[] {", "}"),
            ("(", " == 1)"),
        ],
        &[("new object[] {", "}"), ("!(", ")")],
        &[("$\"{", "}\""), ("(char)((int)", " - 1)")],
        &[("new object[] {", "}"), ("f(a, ", ")"), ("(", ").x")],
        &[("@{ a = ", " }"), ("a(", ")"), ("~", ""), ("(int)", "")],
        &[("!(", ")"), ("M.F(", ")"), ("(", ").x")],
        &[("F((", "))"), ("a[", "]")],
        &[("-(", ")"), ("a.b.c(", ").d")],
        &[("@{ a = ", " }"), ("++", "")],
    ];
    for kind in kinds {
        for n in [60, 120, 250] {
            let seq: Vec<_> = (0..n).map(|i| kind[i % kind.len()]).collect();
            let open: String = seq.iter().map(|(o, _)| *o).collect();
            let close: String = seq.iter().rev().map(|(_, c)| *c).collect();
            let _ = run(&format!("static string X = {open}1{close};"));
        }
    }
}

#[test]
fn deeply_nested_failing_method_calls() {
    for n in [30, 1_000] {
        let _ = run_body(&format!(
            "string q = \"x\"; var r = {}; return \"x\";",
            nest("q.Foo(", "1", ")", n)
        ));
    }
}

#[test]
fn long_unary_chains() {
    for n in [100, 100_000] {
        let _ = run(&format!("static bool X = {}true;", "!".repeat(n)));
        let _ = run(&format!("static long X = {}1;", "~".repeat(n)));
        let _ = run(&format!("static long X = {}1;", "(int)".repeat(n)));
        let _ = run(&format!("static long X = {}1;", "(int)!~".repeat(n)));
    }
}

#[test]
fn deeply_nested_statements() {
    for n in [30, 1_000, 50_000] {
        let _ = run_body(&format!(
            "{} return \"x\";",
            nest("if (true) { ", "int z = 1;", " }", n)
        ));
        let _ = run_body(&format!("{} return \"x\";", "if (true) ".repeat(n) + "{ }"));
        let _ = run_body(&format!("{} return \"x\";", nest("try { ", "", " } ", n)));
        let _ = run_body(&format!("{} return \"x\";", nest("{ ", "", " } ", n)));
        let _ = run_body(&format!(
            "{} return \"x\";",
            nest("foreach (char c in \"ab\") { ", "", " } ", n)
        ));
        let _ = run_body(&format!(
            "{} return \"x\";",
            nest("while (true) { ", "", " } ", n)
        ));
    }
}

#[test]
fn deeply_nested_declarations() {
    for n in [100, 50_000] {
        let _ = run_program(nest("namespace A { ", "", "} ", n));
        let _ = run_program(nest("class A { ", "", "} ", n));
        let _ = run(&format!("static {} X;", nest("List<", "int", ">", n)));
    }
}

#[test]
fn deeply_nested_collections() {
    for n in [100, 50_000] {
        let _ = run(&format!(
            "static object X = new object[] {};",
            nest("{", "1", "}", n)
        ));
        let _ = run_body(&format!(
            "var h = {}; return \"x\";",
            nest("@{ a = ", "1", " }", n)
        ));
        let _ = run(&format!(
            "static string X = {};",
            nest("$\"{", "1", "}\"", n)
        ));
    }
}

#[test]
fn nested_array_values() {
    let _ = run_body(
        r#"string s = "a" * 100000; object[] a = new object[] { 1 }; foreach (char ch in s) { a = new object[] { a }; } return "x";"#,
    );
}

// ------------------------------------------------------------------- loops

#[test]
fn infinite_loops_terminate() {
    for body in [
        "while (true) { } return \"x\";",
        "while (true) { continue; } return \"x\";",
        "do { } while (true); return \"x\";",
        "for (int i = 0; ; i++) { } return \"x\";",
        "for (int i = 0; i < 2000000000; i++) { } return \"x\";",
    ] {
        let _ = run_body(body);
    }
}

#[test]
fn nested_foreach_over_long_strings() {
    let long = "a".repeat(5_000);
    let _ = run_body(&format!(
        "string s = \"{long}\"; int k = 0; foreach (char a in s) {{ foreach (char b in s) {{ \
         foreach (char c in s) {{ k = k + 1; }} }} }} return \"\" + k;"
    ));
}

#[test]
fn huge_ranges() {
    for body in [
        "var r = 0..9223372036854775807; return \"x\";",
        "var r = (-9223372036854775807 - 1)..9223372036854775807; return \"x\";",
        "foreach (var i in 0..100000000) { } return \"x\";",
    ] {
        let _ = run_body(body);
    }
}

// ------------------------------------------------------------------- memory

#[test]
fn huge_allocations() {
    for body in [
        "string s = \"a\" * 99999999999999; return s;",
        "string s = \"abc\" * 9223372036854775807; return s;",
        "string s = \"abc\"; return s.padleft(999999999999999);",
        "string s = \"abc\"; return s.padright(2000000000);",
        "var a = \"a,b\".split(\",\") * 4000000000000000000; return \"x\";",
        "var a = \"abc\".split(\"b\", 0) * 4000000000000000000; return \"x\";",
        "string s = \"x\".replace(\"x\", \"y\" * 10000000); return s.replace(\"y\", s);",
    ] {
        let _ = run_body(body);
    }
}

#[test]
fn exponential_string_growth() {
    let _ = run(&format!(
        "static string F(string s) {{ return s + s; }} static string X = {};",
        nest("M.F(", "\"a\"", ")", 60)
    ));
    let _ = run_body(
        &(String::from("string s = \"a\"; ") + &"s = s + s; ".repeat(64) + "return \"x\";"),
    );
    let _ = run_body(
        r#"string s = "a"; foreach (char c in "0123456789012345678901234567890123456789") { s = s + s; } return "x";"#,
    );
    let _ = run_body(
        r#"object[] a = new object[] { "aaaaaaaaaaaaaaaa" }; foreach (char c in "0123456789012345678901234567890123456789") { a = a + a; } return "x";"#,
    );
}

// --------------------------------------------- results stay useful under
// limits

#[test]
fn work_before_limit_is_kept() {
    let out = run(r#"
        static string A = "keep" + "me";
        static string F(string s) { return M.F(s); }
        static string R = M.F("x");
        static string B = "after" + "wards";"#)
    .unwrap();
    assert!(out.strings.contains("keepme"), "{:?}", out.strings);
    assert!(out.strings.contains("afterwards"), "{:?}", out.strings);
    assert!(!out.errors.is_empty());
}

#[test]
fn session_usable_after_limit() {
    guarded(|| {
        let mut p = CSharpSession::new();
        let _ = p.parse_input(
            "namespace N { class M { static string F(string s) { return M.F(s); } static string R \
             = M.F(\"x\"); } }",
        );
        let res = p
            .parse_input(r#"namespace N { class M { static string a = "x" + "y"; } }"#)
            .unwrap();
        assert!(res.tokens().string_set().contains("xy"));
    });
}
