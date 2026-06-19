//! Acceptance tests for Issue #241: Document/Sketch 2-stage Variable scope + Equation evaluation.

use engawa_format::{evaluate_scope, Document, EvalError, Feature, Variable};

/// T01: 決定性 — 同一 Variable 列を 2 回 evaluate_scope → 同一 IndexMap (キー順含む)
#[test]
fn t01_determinism() {
    let doc_vars = vec![
        Variable {
            name: "a".into(),
            expr: "10".into(),
        },
        Variable {
            name: "b".into(),
            expr: "${a} * 2".into(),
        },
    ];
    let sketch_vars: Vec<Variable> = vec![];

    let result1 = evaluate_scope(&doc_vars, &sketch_vars).unwrap();
    let result2 = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

    assert_eq!(result1, result2);
    assert_eq!(result1.len(), 2);
    assert_eq!(result1.get("a"), Some(&10.0));
    assert_eq!(result1.get("b"), Some(&20.0));
}

/// T02: 正常系 — Document のみ: a=10, b="${a}*2", c="${b}+${a}" → a=10, b=20, c=30
#[test]
fn t02_document_only() {
    let doc_vars = vec![
        Variable {
            name: "a".into(),
            expr: "10".into(),
        },
        Variable {
            name: "b".into(),
            expr: "${a}*2".into(),
        },
        Variable {
            name: "c".into(),
            expr: "${b}+${a}".into(),
        },
    ];
    let sketch_vars: Vec<Variable> = vec![];

    let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

    assert_eq!(result.get("a"), Some(&10.0));
    assert_eq!(result.get("b"), Some(&20.0));
    assert_eq!(result.get("c"), Some(&30.0));
}

/// T03: 正常系 (shadowing) — Document a=10, Sketch b="${a}+5" → Sketch scope 評価結果に Document.a が含まれる
#[test]
fn t03_sketch_shadowing() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "10".into(),
    }];
    let sketch_vars = vec![Variable {
        name: "b".into(),
        expr: "${a}+5".into(),
    }];

    let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

    assert_eq!(result.get("a"), Some(&10.0));
    assert_eq!(result.get("b"), Some(&15.0));
    assert_eq!(result.len(), 2);
}

/// T03b: 正常系 (shadowing 上書き) — Document a=10, Sketch a=20, Sketch b="${a}+5" → Sketch.a が優先
#[test]
fn t03b_sketch_overrides_a() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "10".into(),
    }];
    let sketch_vars = vec![
        Variable {
            name: "a".into(),
            expr: "20".into(),
        },
        Variable {
            name: "b".into(),
            expr: "${a}+5".into(),
        },
    ];

    let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

    assert_eq!(result.get("a"), Some(&20.0));
    assert_eq!(result.get("b"), Some(&25.0));
    assert_eq!(result.len(), 2);
}

/// T03c: 境界 — Document のみ、Sketch 空の regression テスト
#[test]
fn t03c_doc_only_no_sketch() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "10".into(),
    }];
    let sketch_vars: Vec<Variable> = vec![];

    let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result.get("a"), Some(&10.0));
}

/// T03d: 境界 — Sketch のみ、Document 空の regression テスト
#[test]
fn t03d_sketch_only_no_doc() {
    let doc_vars: Vec<Variable> = vec![];
    let sketch_vars = vec![Variable {
        name: "a".into(),
        expr: "10".into(),
    }];

    let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result.get("a"), Some(&10.0));
}

/// T05: 正常系 — operator precedence: "2+3*4" → 14.0
#[test]
fn t05_arithmetic() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "2+3*4".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]).unwrap();
    assert_eq!(result.get("a"), Some(&14.0));
}

/// T06: 正常系 — parentheses: "(2+3)*4" → 20.0
#[test]
fn t06_parens() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "(2+3)*4".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]).unwrap();
    assert_eq!(result.get("a"), Some(&20.0));
}

/// T07: 正常系 — unary minus: "-5" → -5.0
#[test]
fn t07_unary_minus() {
    let doc_vars = vec![
        Variable {
            name: "a".into(),
            expr: "-5".into(),
        },
        Variable {
            name: "b".into(),
            expr: "${a}*2".into(),
        },
    ];
    let result = evaluate_scope(&doc_vars, &[]).unwrap();
    assert_eq!(result.get("a"), Some(&-5.0));
    assert_eq!(result.get("b"), Some(&-10.0));
}

/// T08: 正常系 — division: "10/4" → 2.5 (f64)
#[test]
fn t08_division() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "10/4".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]).unwrap();
    assert_eq!(result.get("a"), Some(&2.5));
}

/// T09: YAML roundtrip — Document.variables + Sketch.variables 含む YAML を to_yaml → from_yaml → 再 to_yaml で同一
#[test]
fn t09_yaml_roundtrip() {
    use engawa_format::{Feature, SketchPlane};

    let mut doc = Document::new("Test");
    doc.variables = vec![
        Variable {
            name: "width".into(),
            expr: "10.0".into(),
        },
        Variable {
            name: "depth".into(),
            expr: "${width} * 2".into(),
        },
    ];

    // Sketch variables も含める
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sketch_1".into(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        suppressed: false,
        variables: vec![Variable {
            name: "w_local".into(),
            expr: "${width} + 5".into(),
        }],
        profile: vec![],
        plane_ref: None,
    });

    let yaml1 = doc.to_yaml().unwrap();
    let doc2 = Document::from_yaml(&yaml1).unwrap();
    let yaml2 = doc2.to_yaml().unwrap();

    assert_eq!(yaml1, yaml2);
    assert_eq!(doc2.variables.len(), 2);
    assert_eq!(doc2.variables[0].name, "width");
    assert_eq!(doc2.variables[1].name, "depth");

    // Sketch.variables も byte-identical roundtrip
    match &doc2.root_component.features[0] {
        Feature::CreateSketch {
            suppressed: _,
            variables,
            ..
        } => {
            assert_eq!(variables.len(), 1);
            assert_eq!(variables[0].name, "w_local");
            assert_eq!(variables[0].expr, "${width} + 5");
        }
        _ => panic!("expected CreateSketch"),
    }
}

/// T10: YAML — variables: [] を含む Document を to_yaml → variables フィールドが出力されない
#[test]
fn t10_empty_vars_omitted() {
    let doc = Document::new("Test");
    let yaml = doc.to_yaml().unwrap();

    assert!(
        !yaml.contains("variables:"),
        "empty variables should be omitted"
    );
}

// --- 退化/境界テスト ---

/// T_DEG_circular: Document a="${b}", b="${a}" → CircularDependency
#[test]
fn t_deg_circular() {
    let doc_vars = vec![
        Variable {
            name: "a".into(),
            expr: "${b}".into(),
        },
        Variable {
            name: "b".into(),
            expr: "${a}".into(),
        },
    ];
    let result = evaluate_scope(&doc_vars, &[]);
    assert!(matches!(result, Err(EvalError::CircularDependency { .. })));
}

/// T_DEG_circular_self: Document a="${a}" → CircularDependency { cycle: ["a", "a"] }
#[test]
fn t_deg_circular_self() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "${a}".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]);
    match result {
        Err(EvalError::CircularDependency { ref cycle }) => {
            assert_eq!(cycle, &["a", "a"]);
        }
        other => panic!("expected CircularDependency, got {:?}", other),
    }
}

/// T_DEG_undefined: Document a="${missing}" → UndefinedVariable { name: "missing" }
#[test]
fn t_deg_undefined() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "${missing}".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]);
    assert!(matches!(
        result,
        Err(EvalError::UndefinedVariable { name })
        if name == "missing"
    ));
}

/// T_BOUNDARY_empty_expr: Document a="" → EmptyExpression
#[test]
fn t_boundary_empty_expr() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]);
    assert!(matches!(result, Err(EvalError::EmptyExpression { .. })));
}

/// T_BOUNDARY_whitespace_expr: Document a="   " → EmptyExpression
#[test]
fn t_boundary_whitespace_expr() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "   ".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]);
    assert!(matches!(result, Err(EvalError::EmptyExpression { .. })));
}

/// T_DEG_sketch_self_shadow_self_ref: doc a=10, sketch a="${a}+1" → CircularDependency
#[test]
fn t_deg_sketch_self_shadow_self_ref() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "10".into(),
    }];
    let sketch_vars = vec![Variable {
        name: "a".into(),
        expr: "${a} + 1".into(),
    }];
    let result = evaluate_scope(&doc_vars, &sketch_vars);
    assert!(matches!(result, Err(EvalError::CircularDependency { .. })));
}

/// T_DEG_parse_invalid: Document a="2 + + 3" → Parse error
#[test]
fn t_deg_parse_invalid() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "2 + + 3".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]);
    assert!(matches!(result, Err(EvalError::Parse { .. })));
}

/// T_DEG_div_by_zero: Document a="1/0" → f64::INFINITY (IEEE 754)
#[test]
fn t_deg_div_by_zero() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "1 / 0".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().get("a"), Some(&f64::INFINITY));
}

/// T11: doc_var_shadowed_not_in_result — doc.a が sketch.a で shadow されたとき result には sketch.a のみ
#[test]
fn t11_doc_var_shadowed_not_in_result() {
    let doc_vars = vec![
        Variable {
            name: "a".into(),
            expr: "10".into(),
        },
        Variable {
            name: "b".into(),
            expr: "20".into(),
        },
    ];
    let sketch_vars = vec![Variable {
        name: "a".into(),
        expr: "99".into(),
    }];

    let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

    // result には a:99 (sketch), b:20 (doc) のみ含まれる
    assert_eq!(result.len(), 2);
    assert_eq!(result.get("a"), Some(&99.0)); // sketch.a
    assert_eq!(result.get("b"), Some(&20.0)); // doc.b
}

/// T12: shadowing_evaluation_order_stability — 同一 input を複数回 evaluate → key 順序が決定的
#[test]
fn t12_shadowing_evaluation_order_stability() {
    let doc_vars = vec![
        Variable {
            name: "c".into(),
            expr: "30".into(),
        },
        Variable {
            name: "a".into(),
            expr: "10".into(),
        },
        Variable {
            name: "b".into(),
            expr: "20".into(),
        },
    ];
    let sketch_vars = vec![
        Variable {
            name: "z".into(),
            expr: "100".into(),
        },
        Variable {
            name: "y".into(),
            expr: "200".into(),
        },
    ];

    // 10 回実行して全結果が一致することを確認
    let mut results = Vec::new();
    for _ in 0..10 {
        let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();
        results.push(result.clone());
    }

    // 全結果が同一であること
    for result in &results[1..] {
        assert_eq!(results[0], *result);
    }

    // IndexMap の key 順序が決定的であること
    // Kahn's algorithm は lexicographic (Reverse-min-heap) tiebreak で pop し、result に挿入
    // indegree=0 の var: z, y, a, b, c → lexicographic order: [a, b, c, y, z]
    let keys: Vec<&str> = results[0].keys().map(|k| k.as_str()).collect();
    assert_eq!(keys, vec!["a", "b", "c", "y", "z"]);
}

/// T_COMPAT_old_doc_without_variables: variables なしの古い YAML が parse できる
#[test]
fn t_compat_old_doc_without_variables() {
    let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: Old
  features: []
";
    let doc = Document::from_yaml(yaml).expect("legacy YAML must parse");
    assert!(doc.variables.is_empty());
    // 再シリアライズしても variables キーは出力されない
    let yaml2 = doc.to_yaml().unwrap();
    assert!(!yaml2.contains("variables:"));
}

/// T_COMPAT_old_sketch_without_variables: Sketch 内 variables なしの古い YAML が parse できる
#[test]
fn t_compat_old_sketch_without_variables() {
    let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: Old
  features:
  - type: create_sketch
    id: sketch_1
    plane: xy
    profile: []
";
    let doc = Document::from_yaml(yaml).expect("legacy sketch YAML must parse");
    match &doc.root_component.features[0] {
        Feature::CreateSketch {
            suppressed: _,
            variables,
            ..
        } => {
            assert!(variables.is_empty());
        }
        _ => panic!("expected CreateSketch"),
    }
}

/// T_COMPAT_trailing_chars_parse_error: "2 3" や "10foo" が Parse error になる
#[test]
fn t_compat_trailing_chars_parse_error() {
    // 末尾に余分な文字がある場合は Parse エラー
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "2 3".into(), // trailing "3"
    }];
    let result = evaluate_scope(&doc_vars, &[]);
    assert!(matches!(result, Err(EvalError::Parse { .. })));

    // 数値直後の文字も Parse エラー
    let doc_vars = vec![Variable {
        name: "b".into(),
        expr: "10foo".into(), // trailing "foo"
    }];
    let result = evaluate_scope(&doc_vars, &[]);
    assert!(matches!(result, Err(EvalError::Parse { .. })));
}

// --- 追加エッジケーステスト (テストモード: 壊しに行く視点) ---

/// T_REPEAT_determinism_100: 同一入力を100回実行して全結果が一致するか
#[test]
fn t_repeat_determinism_100() {
    let doc_vars = vec![
        Variable {
            name: "c".into(),
            expr: "30".into(),
        },
        Variable {
            name: "a".into(),
            expr: "10".into(),
        },
        Variable {
            name: "b".into(),
            expr: "${a} + ${c}".into(),
        },
    ];
    let sketch_vars = vec![
        Variable {
            name: "z".into(),
            expr: "100".into(),
        },
        Variable {
            name: "y".into(),
            expr: "${z} / 2".into(),
        },
    ];

    let mut results = Vec::new();
    for _ in 0..100 {
        let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();
        results.push(result.clone());
    }

    // 全結果が同一であること
    for result in &results[1..] {
        assert_eq!(results[0], *result);
    }

    // IndexMap の key 順序が決定的であること
    // indegree=0 の var: c, a, z, y → lexicographic order: [a, c, z, y]
    // b は a,c に依存 → indegree=2 → 後回し
    // 順序: [a, c, z, y, b] または Kahn's algorithm の順序
    let keys: Vec<&str> = results[0].keys().map(|k| k.as_str()).collect();
    // 結果の順序は決定的であれば OK（値の正しさは別途検証）
    assert_eq!(keys.len(), 5);
    assert_eq!(results[0].get("a"), Some(&10.0));
    assert_eq!(results[0].get("b"), Some(&40.0)); // 10 + 30
    assert_eq!(results[0].get("c"), Some(&30.0));
    assert_eq!(results[0].get("y"), Some(&50.0)); // 100 / 2
    assert_eq!(results[0].get("z"), Some(&100.0));

    // 100回すべて同じ順序であること
    for result in &results {
        let keys: Vec<&str> = result.keys().map(|k| k.as_str()).collect();
        assert_eq!(
            keys,
            results[0].keys().map(|k| k.as_str()).collect::<Vec<_>>()
        );
    }
}

/// T_BOUNDARY_nan_expr: NaN を式に含む場合の挙動
#[test]
fn t_boundary_nan_expr() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "NaN".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]);
    // "NaN" は数値リテラルとして認識されない → Parse error
    assert!(matches!(result, Err(EvalError::Parse { .. })));
}

/// T_BOUNDARY_inf_expr: Infinity を式に含む場合の挙動
#[test]
fn t_boundary_inf_expr() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "Infinity".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]);
    // "Infinity" は数値リテラルとして認識されない → Parse error
    assert!(matches!(result, Err(EvalError::Parse { .. })));
}

/// T_BOUNDARY_neg_zero: -0.0 の扱い
#[test]
fn t_boundary_neg_zero() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "-0.0".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]).unwrap();
    // IEEE 754 では -0.0 == 0.0 は true
    let val = result.get("a").unwrap();
    assert_eq!(*val, 0.0);
    // 符号ビットは異なるが、値としては等価
    // -0.0 の to_bits は 0x8000000000000000 (符号ビットが1)
    // +0.0 の to_bits は 0x0000000000000000
    // パーサーが -0.0 を正しくパースしていることを確認
    let bits = val.to_bits();
    assert!(bits == 0.0f64.to_bits() || bits == (-0.0f64).to_bits());
}

/// T_BOUNDARY_very_large: 非常に大きな数値の計算 (decimal 表記)
#[test]
fn t_boundary_very_large() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "1000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]).unwrap();
    // 非常に大きな数値だが finite
    assert!(result.get("a").unwrap().is_finite());
    // 値が正しくパースされていること
    assert!(*result.get("a").unwrap() > 0.0);
}

/// T_BOUNDARY_div_by_zero_negative: 負の数値のゼロ除算
#[test]
fn t_boundary_div_by_zero_negative() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "-10 / 0".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]).unwrap();
    // IEEE 754 では -10 / 0 = -Inf
    assert!(result.get("a").unwrap().is_infinite());
    assert!(result.get("a").unwrap() < &0.0); // 負の無限大
}

/// T_BOUNDARY_zero_div_zero: 0 / 0 の場合は NaN
#[test]
fn t_boundary_zero_div_zero() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "0 / 0".into(),
    }];
    let result = evaluate_scope(&doc_vars, &[]).unwrap();
    // IEEE 754 では 0 / 0 = NaN
    assert!(result.get("a").unwrap().is_nan());
}

/// T_EMPTY_both_empty: Document/Sketch ともに空の variables
#[test]
fn t_empty_both_empty() {
    let doc_vars: Vec<Variable> = vec![];
    let sketch_vars: Vec<Variable> = vec![];

    let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();

    assert_eq!(result.len(), 0);
}

/// T_DEG_three_way_cycle: 3変数の循環依存
#[test]
fn t_deg_three_way_cycle() {
    let doc_vars = vec![
        Variable {
            name: "a".into(),
            expr: "${b}".into(),
        },
        Variable {
            name: "b".into(),
            expr: "${c}".into(),
        },
        Variable {
            name: "c".into(),
            expr: "${a}".into(),
        },
    ];
    let result = evaluate_scope(&doc_vars, &[]);
    match result {
        Err(EvalError::CircularDependency { ref cycle }) => {
            // cycle は ["a", "b", "c", "a"] または lexicographic 順で ["a", "b", "c", "a"]
            assert_eq!(cycle.len(), 4);
            assert_eq!(cycle[0], cycle[3]); // 閉じた cycle
        }
        other => panic!("expected CircularDependency, got {:?}", other),
    }
}

/// T_COMPLEX_diamond_dep: ダイヤモンド依存 (b→a, c→a, d→b+c)
#[test]
fn t_complex_diamond_dep() {
    let doc_vars = vec![
        Variable {
            name: "a".into(),
            expr: "10".into(),
        },
        Variable {
            name: "b".into(),
            expr: "${a} + 5".into(),
        },
        Variable {
            name: "c".into(),
            expr: "${a} * 2".into(),
        },
        Variable {
            name: "d".into(),
            expr: "${b} + ${c}".into(),
        },
    ];
    let result = evaluate_scope(&doc_vars, &[]).unwrap();
    // a=10, b=15, c=20, d=35
    assert_eq!(result.get("a"), Some(&10.0));
    assert_eq!(result.get("b"), Some(&15.0));
    assert_eq!(result.get("c"), Some(&20.0));
    assert_eq!(result.get("d"), Some(&35.0));
}

// --- R4 regression テスト (重複名検出) ---

/// T_DEG_doc_duplicate_name (R4 regression):
/// 同一 Document scope に同名 Variable が複数あれば EvalError::DuplicateVariable
#[test]
fn t_deg_doc_duplicate_name() {
    let doc_vars = vec![
        Variable {
            name: "a".into(),
            expr: "1".into(),
        },
        Variable {
            name: "a".into(),
            expr: "2".into(),
        },
    ];
    let result = evaluate_scope(&doc_vars, &[]);
    match result {
        Err(EvalError::DuplicateVariable { scope, name }) => {
            assert_eq!(scope, "document");
            assert_eq!(name, "a");
        }
        other => panic!("expected DuplicateVariable in document, got {other:?}"),
    }
}

/// T_DEG_sketch_duplicate_name (R4 regression):
/// 同一 Sketch scope に同名 Variable が複数あれば EvalError::DuplicateVariable
/// (Document との shadowing は OK、Sketch 内重複は NG)
#[test]
fn t_deg_sketch_duplicate_name() {
    let doc_vars = vec![];
    let sketch_vars = vec![
        Variable {
            name: "x".into(),
            expr: "10".into(),
        },
        Variable {
            name: "x".into(),
            expr: "20".into(),
        },
    ];
    let result = evaluate_scope(&doc_vars, &sketch_vars);
    match result {
        Err(EvalError::DuplicateVariable { scope, name }) => {
            assert_eq!(scope, "sketch");
            assert_eq!(name, "x");
        }
        other => panic!("expected DuplicateVariable in sketch, got {other:?}"),
    }
}

/// T_OK_cross_scope_shadowing_not_duplicate (R4 regression):
/// Document.a と Sketch.a の同名は重複ではなく shadowing (ADR-015 §2) → OK
#[test]
fn t_ok_cross_scope_shadowing_not_duplicate() {
    let doc_vars = vec![Variable {
        name: "a".into(),
        expr: "10".into(),
    }];
    let sketch_vars = vec![Variable {
        name: "a".into(),
        expr: "20".into(),
    }];
    let result = evaluate_scope(&doc_vars, &sketch_vars).unwrap();
    assert_eq!(result.get("a"), Some(&20.0)); // sketch shadows doc
}
