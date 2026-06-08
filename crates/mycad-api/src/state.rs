use crate::error::ApiError;
use mycad_format::Document;
use std::path::PathBuf;

pub struct AppState {
    pub path: PathBuf,
    /// 遅延ロード: 初回の /mesh または /features アクセス時に from_path で読み込みキャッシュ
    doc: Option<Document>,
}

impl AppState {
    pub fn new(path: PathBuf) -> Self {
        Self { path, doc: None }
    }

    /// doc 未ロードなら from_path で読み込む。読込済みなら参照を返す。
    ///
    /// # キャッシュ戦略
    /// 初回ロード後はメモリ常駐（再読み込みなし）。
    /// ADR-008 §Decision 3「mycad view はシングルユーザーサーバのため競合問題は発生しない」に基づき、
    /// 外部プロセスによる .mycad の並行更新は非サポートシナリオとして明示的に除外する。
    pub fn ensure_loaded(&mut self) -> Result<&Document, ApiError> {
        if self.doc.is_none() {
            self.doc = Some(Document::from_path(&self.path)?);
        }
        Ok(self.doc.as_ref().unwrap())
    }

    /// ロード済み Document の clone を返す（mutation の元にする）。
    pub(crate) fn snapshot(&mut self) -> Result<Document, ApiError> {
        self.ensure_loaded()?;
        Ok(self.doc.as_ref().unwrap().clone())
    }

    /// 検証済み Document でインメモリ状態を更新する。
    /// 呼び出し側が validate を通す責務を持つ（IN01: validate → assemble → write → commit の順序）。
    pub(crate) fn commit(&mut self, doc: Document) {
        self.doc = Some(doc);
    }

    pub fn base_dir(&self) -> PathBuf {
        self.path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 存在する .mycad ファイルのパス（テスト用）
    fn simple_box_path() -> PathBuf {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let p = std::path::Path::new(&dir)
            .join("..")
            .join("..")
            .join("examples")
            .join("simple_box.mycad");
        std::fs::canonicalize(&p).expect("simple_box.mycad fixture must exist")
    }

    #[test]
    fn ensure_loaded_nonexistent_path_returns_error_and_doc_stays_none() {
        let mut state = AppState::new(PathBuf::from("/nonexistent/path/file.mycad"));
        assert!(state.doc.is_none());
        let result = state.ensure_loaded();
        assert!(result.is_err(), "nonexistent path must error");
        assert!(
            state.doc.is_none(),
            "doc must remain None after failed load"
        );
    }

    #[test]
    fn ensure_loaded_valid_path_populates_doc() {
        let mut state = AppState::new(simple_box_path());
        assert!(state.doc.is_none());
        let result = state.ensure_loaded();
        assert!(result.is_ok(), "valid path must succeed");
        assert!(
            state.doc.is_some(),
            "doc must be Some after successful load"
        );
    }

    #[test]
    fn ensure_loaded_second_call_skips_reload() {
        let mut state = AppState::new(simple_box_path());
        let _ = state.ensure_loaded().unwrap();
        let doc_ptr = state.doc.as_ref().unwrap() as *const _;
        // Second call returns the same allocation (no reload)
        let _ = state.ensure_loaded().unwrap();
        let doc_ptr2 = state.doc.as_ref().unwrap() as *const _;
        assert_eq!(
            doc_ptr, doc_ptr2,
            "second ensure_loaded must not replace the doc"
        );
    }

    // --- T01: snapshot returns clone, not alias ---
    #[test]
    fn t01_snapshot_returns_clone_not_alias() {
        let mut state = AppState::new(simple_box_path());
        let candidate = state.snapshot().expect("snapshot must succeed");

        let second = state.snapshot().expect("second snapshot must succeed");
        assert_eq!(
            candidate.root_component.features.len(),
            second.root_component.features.len(),
            "snapshots should reflect the same document state"
        );

        // Different heap allocations (clone, not reference)
        let cand_ptr = &candidate.root_component.features as *const _;
        let sec_ptr = &second.root_component.features as *const _;
        assert_ne!(cand_ptr, sec_ptr, "snapshot must return a clone, not alias");
    }

    // --- T02: commit updates internal doc ---
    #[test]
    fn t02_commit_updates_snapshot() {
        let mut state = AppState::new(simple_box_path());
        let mut candidate = state.snapshot().expect("snapshot must succeed");
        let original_len = candidate.root_component.features.len();

        candidate.root_component.features.pop();
        state.commit(candidate);

        let after = state
            .snapshot()
            .expect("snapshot after commit must succeed");
        assert_eq!(
            after.root_component.features.len(),
            original_len - 1,
            "commit must update internal doc"
        );
    }

    // --- T03: ensure_loaded returns &Document (read-only) ---
    #[test]
    fn t03_ensure_loaded_returns_ref_document() {
        let mut state = AppState::new(simple_box_path());
        // This compiles only because ensure_loaded returns &Document (not &mut Document)
        let _doc: &Document = state.ensure_loaded().expect("ensure_loaded must succeed");
    }

    // --- T01_degen: commit without prior load does not panic ---
    #[test]
    fn t01_degen_commit_without_load_no_panic() {
        let mut state = AppState::new(simple_box_path());

        let loaded = Document::from_path(&simple_box_path()).expect("from_path must succeed");
        state.commit(loaded);

        let after = state
            .snapshot()
            .expect("snapshot after commit must succeed");
        assert!(
            !after.root_component.features.is_empty(),
            "committed doc must be retrievable via snapshot"
        );
    }
}
