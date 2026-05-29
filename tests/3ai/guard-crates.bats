#!/usr/bin/env bats
# guard-crates.sh のユニットテスト

load helpers/common

GUARD="$SCRIPTS_DIR/guard-crates.sh"

setup() {
  setup_common
}

teardown() {
  teardown_common
}

# --- テストケース 6a: crates/** への Edit は block される ---

@test "TC6a: crates/** のパスは block される (exit=2)" {
  INPUT='{"tool_name":"Edit","tool_input":{"file_path":"crates/mycad-kernel/src/lib.rs"}}'
  run bash -c "echo '$INPUT' | bash '$GUARD'"
  [ "$status" -eq 2 ]
  [[ "$output" == *"block"* ]]
}

# --- テストケース 6b: features/ 配下の debug-spec.md は通過 ---

@test "TC6b: features/.../debug-spec.md は block されない (exit=0)" {
  INPUT='{"tool_name":"Write","tool_input":{"file_path":"features/36-escalation/debug-spec.md"}}'
  run bash -c "echo '$INPUT' | bash '$GUARD'"
  [ "$status" -eq 0 ]
}

# --- テストケース 6c: CAD_WORKER=1 のとき crates/** でも通過 ---

@test "TC6c: CAD_WORKER=1 のとき crates/** も通過する (exit=0)" {
  INPUT='{"tool_name":"Edit","tool_input":{"file_path":"crates/mycad-kernel/src/lib.rs"}}'
  run bash -c "export CAD_WORKER=1; echo '$INPUT' | bash '$GUARD'"
  [ "$status" -eq 0 ]
}
