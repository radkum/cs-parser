//! Locks in the evaluation results the deobfuscator relies on today.

use std::collections::BTreeSet;

use cs_parser::CSharpSession;

fn strings(members: &str) -> BTreeSet<String> {
    let src = format!("namespace N {{ public static class M {{\n{members}\n}} }}");
    CSharpSession::new()
        .parse_input(&src)
        .unwrap()
        .tokens()
        .string_set()
}

fn assert_contains(members: &str, expected: &[&str]) {
    let set = strings(members);
    for e in expected {
        assert!(set.contains(*e), "missing {e:?} in {set:?}");
    }
}

#[test]
fn static_field_concat() {
    assert_contains(
        r#"static string a = "ab"; static string b = a + "cd";"#,
        &["abcd"],
    );
}

#[test]
fn static_fields_reference_each_other() {
    assert_contains(
        r#"static string P = "a" + "b"; static string Q = P + P; static string W = Q.toupper();"#,
        &["abab", "ABAB"],
    );
}

#[test]
fn const_field() {
    assert_contains(
        r#"const string K = "const"; static readonly string R = K + "ant";"#,
        &["constant"],
    );
}

#[test]
fn static_method_call() {
    assert_contains(
        r#"static string F(string x, string y) { return x + "_" + y; } static string R = M.F("p", "q");"#,
        &["p_q"],
    );
}

#[test]
fn method_declared_after_use() {
    assert_contains(
        r#"static string A = M.B("x"); static string B(string s) { return s + "y"; }"#,
        &["xy"],
    );
}

#[test]
fn nested_method_calls() {
    assert_contains(
        r#"static string A = M.F(M.F(M.F("a"))); static string F(string s) { return s + s; }"#,
        &["aa", "aaaa", "aaaaaaaa"],
    );
}

#[test]
fn string_builder_append() {
    assert_contains(
        r#"static string F(string s) { StringBuilder b = new StringBuilder(s); b.Append("x"); b.Append('y'); return b.ToString(); } static string R = M.F("Q");"#,
        &["Qxy"],
    );
}

#[test]
fn foreach_char_shift_decoder() {
    assert_contains(
        r#"static string F(string input) { StringBuilder b = new StringBuilder(input.Length + 1); foreach (char c in input) { char m = (char)((int)c - 1); b.Append(m); } return b.ToString(); } static string R = M.F("Tztufn");"#,
        &["System"],
    );
}

#[test]
fn foreach_reverse_string() {
    assert_contains(
        r#"static string F(string s) { string r = ""; foreach (char c in s) { r = c + r; } return r; } static string R = M.F("dlrow");"#,
        &["world"],
    );
}

#[test]
fn foreach_xor_decoder() {
    assert_contains(
        r#"static string F(string s) { string r = ""; foreach (char c in s) { r = r + (char)((int)c ^ 1); } return r; } static string R = M.F("`ab");"#,
        &["a`c"],
    );
}

#[test]
fn foreach_over_array() {
    assert_contains(
        r#"static string F() { object[] a = new object[] { "p", "q" }; string r = ""; foreach (object o in a) { r = r + o; } return r; } static string R = M.F();"#,
        &["pq"],
    );
}

#[test]
fn char_arithmetic() {
    assert_contains(
        r#"static string F(string s) { char m = (char)((int)s[0] + 1); return "" + m; } static string R = M.F("a");"#,
        &["b"],
    );
}

#[test]
fn char_literal_concat() {
    assert_contains(
        r#"static string F() { char c = 'z'; return "c" + c; } static string R = M.F();"#,
        &["cz"],
    );
}

#[test]
fn string_indexing() {
    assert_contains(
        r#"static string F() { string s = "abc"; return s[1] + "" + s[0]; } static string R = M.F();"#,
        &["ba"],
    );
}

#[test]
fn string_length() {
    assert_contains(
        r#"static string F(string s) { int n = s.Length; return "n" + n; } static string R = M.F("abcd");"#,
        &["n4"],
    );
}

#[test]
fn arithmetic_precedence() {
    assert_contains(
        r#"static string a = "n" + (1 + 2 * 3); static string b = "n" + ((1 + 2) * 3); static string c = "n" + (10 / 4); static string d = "n" + (10 % 3); static string e = "n" + (7 - 10);"#,
        &["n7", "n9", "n2.5", "n1", "n-3"],
    );
}

#[test]
fn bitwise_operators() {
    assert_contains(
        r#"static string a = "b" + (6 & 3); static string b = "b" + (6 | 3); static string c = "b" + (6 ^ 3); static string d = "b" + (1 << 4); static string e = "b" + (256 >> 2);"#,
        &["b2", "b7", "b5", "b16", "b64"],
    );
}

#[test]
fn comparison_and_logical() {
    assert_contains(
        r#"static string a = "c" + (1 < 2); static string b = "c" + (2 == 2); static string c = "c" + (1 != 1); static string d = "c" + (true && false); static string e = "c" + !true;"#,
        &["cTrue", "cFalse"],
    );
}

#[test]
fn mixed_type_concat() {
    assert_contains(r#"static string s = "A" + 'b' + 1 + 2.5;"#, &["Ab12.5"]);
}

#[test]
fn if_else() {
    assert_contains(
        r#"static string F(int x) { if (x > 1) { return "big"; } else { return "small"; } } static string A = M.F(5); static string B = M.F(0);"#,
        &["big", "small"],
    );
}

#[test]
fn if_else_if_updates_variable() {
    assert_contains(
        r#"static string F() { string t = "ab"; if (t == "ab") { t = t + "!"; } else if (t == "x") { t = "no"; } return t; } static string R = M.F();"#,
        &["ab!"],
    );
}

#[test]
fn compound_assignment() {
    assert_contains(
        r#"static string F(string s) { var x = s + "1"; x += "2"; return x; } static string R = M.F("v");"#,
        &["v12"],
    );
}

#[test]
fn string_methods() {
    assert_contains(
        r#"static string s = "Hello World"; static string a = s.toupper(); static string b = s.tolower(); static string d = s.substring(0, 5); static string e = s.replace("World", "There");"#,
        &["HELLO WORLD", "hello world", "Hello", "Hello There"],
    );
}

#[test]
fn string_trim_pad_insert_split() {
    assert_contains(
        r#"static string a = "  pad  ".trim(); static string d = "x".padleft(4); static string i = "abc".insert(1, "X"); static string p = "a,b,c".split(",")[1];"#,
        &["pad", "   x", "aXbc", "b"],
    );
}

#[test]
fn hashtable_lookup() {
    assert_contains(
        r#"static string F() { var h = @{ k = "hv" }; return h["k"]; } static string R = M.F();"#,
        &["hv"],
    );
}

#[test]
fn interpolated_and_verbatim_strings() {
    assert_contains(
        r#"static string X = $"interp {1 + 1}"; static string Y = @"verbatim\path";"#,
        &["interp 2", r"verbatim\path"],
    );
}

#[test]
fn unknown_call_without_arguments() {
    assert_contains(
        r#"static string F() { var p = GetCurrentProcess(); return "x"; } static string R = M.F();"#,
        &["GetCurrentProcess"],
    );
}

#[test]
fn unknown_nested_calls_keep_script_text() {
    assert_contains(
        r#"static string F() { return Unknown.Api(Other.Call("in" + "ner"), 5); } static string R = M.F();"#,
        &[
            "inner",
            "Other.Call(\"inner\")",
            "Unknown.Api(\"Other.Call(\"inner\")\", 5)",
        ],
    );
}

#[test]
fn nested_if_collects_all_branches() {
    assert_contains(
        r#"static string F(string s) { string t = "a"; if (s == "x") { t = t + "1"; if (t == "a1") { t = t + "2"; } else { t = t + "3"; } } else { t = t + "4"; } return t; } static string R = M.F("x"); static string Q = M.F("y");"#,
        &["a12", "a4", "a123", "a124"],
    );
}

#[test]
fn nested_classes_in_namespace() {
    let res = CSharpSession::new()
        .parse_input(r#"namespace N { class C { static string A = "x"; class D { static string B = "y" + "z"; } } }"#)
        .unwrap();
    assert!(res.tokens().string_set().contains("yz"));
}

#[test]
fn session_is_reusable() {
    let mut p = CSharpSession::new();
    for _ in 0..3 {
        let res = p
            .parse_input(r#"namespace N { class M { static string a = "x" + "y"; } }"#)
            .unwrap();
        assert!(res.tokens().string_set().contains("xy"));
    }
}

#[test]
fn safe_eval_statements() {
    let mut p = CSharpSession::new();
    let res = p.safe_eval_statements(r#"string a = "q"; a + "w";"#);
    assert!(res.is_ok());
}
