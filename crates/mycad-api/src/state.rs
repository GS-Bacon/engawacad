use crate::error::ApiError;
use mycad_format::Document;
use std::path::PathBuf;

pub struct AppState {
    pub path: PathBuf,
    /// 遅延ロード: 初回の /mesh または /features アクセス時に from_path で読み込みキャッシュ
    pub doc: Option<Document>,
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
    pub fn ensure_loaded(&mut self) -> Result<&mut Document, ApiError> {
        if self.doc.is_none() {
            self.doc = Some(Document::from_path(&self.path)?);
        }
        Ok(self.doc.as_mut().unwrap())
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
}
