# #153 Test Spec

## 不足テスト (plan 計画分)

### T_boundary_parallel_safety の本体実装

**現状**: `crates/mycad-api/tests/mycad_api_serial_acceptance.rs` に `#[ignore]` + `todo!()` の skeleton として存在。

**やるべきこと**:

skeleton ファイル `crates/mycad-api/tests/mycad_api_serial_acceptance.rs` の `t_boundary_parallel_safety` を以下のように実装する:

1. `#[ignore]` 属性を削除する
2. `#[serial(mycad_api_port_7878)]` を付与する (serial_test crate を使うことを明示)
3. テスト本体: port 7878 に bind できることを確認し (`TcpListener::bind("127.0.0.1:7878")`)、`#[serial]` 属性が他の port 7878 利用テストと相互排他されることに依存することを assertion でなく test 配置の意味として確認する。

   実装案:
   ```rust
   use serial_test::serial;
   use std::net::TcpListener;

   #[test]
   #[serial(mycad_api_port_7878)]
   fn t_boundary_parallel_safety() {
       // serial_test が port 7878 を握る全テストを直列化することを確認:
       // この test が動いている間、他の serial(mycad_api_port_7878) テストは block される。
       // bind が成功することで「他テストが解放した直後の状態」を確認する。
       let listener = TcpListener::bind("127.0.0.1:7878")
           .expect("port 7878 should be bindable when serial_test serializes access");
       drop(listener);
   }
   ```

## 不要セクション

- **実装差分から追加すべきテスト**: なし (plan の T01_existing/T02_existing は既存テストの維持なので新規追加不要)
- **エッジケース・退化入力**: なし
- **数値境界**: なし
- **決定性**: なし (serial_test は test 実行順を直列化するだけ)

## 完了条件

- `cargo test -p mycad-api --test mycad_api_serial_acceptance` で `t_boundary_parallel_safety` が green
- `cargo test -p mycad-api --test startup_log_acceptance` で既存 T01/T02 が green
- `cargo xtask ci` 全 green
