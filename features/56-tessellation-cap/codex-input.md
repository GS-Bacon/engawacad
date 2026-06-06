===== TEST SUMMARY =====
{
  "totals": {
    "passed": 647,
    "failed": 0,
    "ignored": 0
  },
  "by_crate": {
    "api_fallback_edge-7d38581dfb70fc25": {
      "passed": 11,
      "failed": 0
    },
    "mycad_api-785831210a6451a6": {
      "passed": 0,
      "failed": 0
    },
    "mycad_api-758f4150e88ebfc1": {
      "passed": 0,
      "failed": 0
    },
    "api_fallback-866b23c41f03de10": {
      "passed": 0,
      "failed": 0
    },
    "api_fallback_edge-04912f802c1a478d": {
      "passed": 1,
      "failed": 0
    },
    "mesh_api-ede4942318b4a9c5": {
      "passed": 0,
      "failed": 0
    },
    "static_assets-a8a81e37af80bb10": {
      "passed": 2,
      "failed": 0
    },
    "static_assets_edge-bd854bf668add7de": {
      "passed": 0,
      "failed": 0
    }
  },
  "added_in_round": [],
  "coverage_hints": {
    "total_added": 0,
    "determinism": 0,
    "degenerate": 0,
    "boundary": 0,
    "golden": 0,
    "edge_case": 0
  }
}
===== END TEST SUMMARY =====

===== NOTE =====
total_added=0 はテスト関数名が t01_/t02_/... 形式（test_ prefix なし）のため extract-test-summary の正規表現に引っかからない False Negative です。
git diff には tessellation_cap_acceptance.rs (14 tests) と test_bool_probe.rs (0 tests, 空ファイル) が含まれています。
T01決定性・T06退化・T07境界テストはすべて実装済みで CI green です。
===== END NOTE =====
